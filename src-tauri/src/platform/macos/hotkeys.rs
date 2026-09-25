use super::hotkey_state::{Action, Machine, Mode, Snapshot};
use crate::config::{DualHotkeyConfig, HotkeyMode, TriggerMode};
use anyhow::Result;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{Duration, Instant},
};
type Callback = Arc<dyn Fn(TriggerMode, bool) + Send + Sync>;
struct Inner {
    active: AtomicBool,
    started: AtomicBool,
    config: RwLock<DualHotkeyConfig>,
    machine: Mutex<Machine>,
    #[cfg(all(feature = "atdd", debug_assertions))]
    atdd: Mutex<super::atdd_control::AtddControl>,
    on_start: RwLock<Option<Callback>>,
    on_stop: RwLock<Option<Callback>>,
}
pub struct HotkeyService {
    inner: Arc<Inner>,
}
impl HotkeyService {
    #[cfg(all(feature = "atdd", debug_assertions))]
    pub fn atdd_recording(&self, mode: TriggerMode) -> Result<u64> {
        anyhow::ensure!(
            self.is_service_active() && crate::platform::desktop().status().ready(),
            "录音服务或系统权限不可用"
        );
        let callback = self
            .inner
            .on_start
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("录音回调尚未初始化"))?;
        let mode = if mode == TriggerMode::AiAssistant {
            Mode::Assistant
        } else {
            Mode::Release
        };
        let (id, _) = {
            let mut machine = self.inner.machine.lock().unwrap();
            self.inner
                .atdd
                .lock()
                .unwrap()
                .begin(&mut machine, mode)
                .map_err(anyhow::Error::msg)?
        };
        callback(
            if mode == Mode::Assistant {
                TriggerMode::AiAssistant
            } else {
                TriggerMode::Dictation
            },
            mode == Mode::Release,
        );
        Ok(id)
    }
    #[cfg(all(feature = "atdd", debug_assertions))]
    pub fn atdd_owns(&self, id: u64) -> bool {
        self.inner.atdd.lock().unwrap().owns(id)
    }
    #[cfg(all(feature = "atdd", debug_assertions))]
    pub fn atdd_abort(&self, id: u64) {
        let mut machine = self.inner.machine.lock().unwrap();
        let mut driver = self.inner.atdd.lock().unwrap();
        if driver.owns(id) {
            driver.reset();
            machine.reset();
        }
    }
    #[cfg(all(feature = "atdd", debug_assertions))]
    pub fn atdd_finish(&self, id: u64) {
        let action = {
            let mut machine = self.inner.machine.lock().unwrap();
            self.inner.atdd.lock().unwrap().finish(&mut machine, id)
        };
        if let Some(Action::Stop(mode)) = action {
            if let Some(callback) = self.inner.on_stop.read().unwrap().clone() {
                callback(
                    if mode == Mode::Assistant {
                        TriggerMode::AiAssistant
                    } else {
                        TriggerMode::Dictation
                    },
                    mode == Mode::Release,
                );
            }
        }
    }
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                active: AtomicBool::new(false),
                started: AtomicBool::new(false),
                config: RwLock::new(DualHotkeyConfig::default()),
                machine: Mutex::new(Machine::default()),
                #[cfg(all(feature = "atdd", debug_assertions))]
                atdd: Mutex::new(super::atdd_control::AtddControl::default()),
                on_start: RwLock::new(None),
                on_stop: RwLock::new(None),
            }),
        }
    }
    pub fn is_service_active(&self) -> bool {
        self.inner.active.load(Ordering::SeqCst)
    }
    pub fn resume(&self) {
        self.reset_state();
        self.inner.active.store(true, Ordering::SeqCst);
    }
    pub fn deactivate(&self) {
        self.inner.active.store(false, Ordering::SeqCst);
        self.reset_state();
    }
    pub fn reset_state(&self) {
        let mut machine = self.inner.machine.lock().unwrap();
        machine.reset();
        #[cfg(all(feature = "atdd", debug_assertions))]
        self.inner.atdd.lock().unwrap().reset();
    }
    pub fn get_debug_info(&self) -> String {
        format!(
            "macOS active={}, {:?}",
            self.is_service_active(),
            self.inner.machine.lock().unwrap()
        )
    }
    pub fn activate_dual<F1, F2>(
        &self,
        config: DualHotkeyConfig,
        on_start: F1,
        on_stop: F2,
    ) -> Result<()>
    where
        F1: Fn(TriggerMode, bool) + Send + Sync + 'static,
        F2: Fn(TriggerMode, bool) + Send + Sync + 'static,
    {
        config.validate()?;
        anyhow::ensure!(
            crate::platform::desktop().status().ready(),
            "请到偏好设置授权麦克风、辅助功能和输入监控，然后启动服务"
        );
        *self.inner.config.write().unwrap() = config;
        *self.inner.on_start.write().unwrap() = Some(Arc::new(on_start));
        *self.inner.on_stop.write().unwrap() = Some(Arc::new(on_stop));
        self.reset_state();
        self.init_listener()?;
        self.inner.active.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn init_listener(&self) -> Result<()> {
        if self.inner.started.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let inner = self.inner.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        if let Err(error) = thread::Builder::new()
            .name("macos-hotkeys".into())
            .spawn(move || {
                if !unsafe { super::ptt_hotkeys_start() } {
                    let _ = ready_tx.send(false);
                    return;
                }
                let _ = ready_tx.send(true);
                let mut permission_check = Instant::now();
                let mut allowed = crate::platform::desktop().status().ready();
                let mut first = true;
                loop {
                    let mut down = [0u8; 128];
                    let sample = unsafe { super::ptt_hotkeys_next(down.as_mut_ptr()) };
                    if sample < 0 {
                        tracing::error!("macOS keyboard event listener stopped");
                        inner.active.store(false, Ordering::SeqCst);
                    }
                    if permission_check.elapsed() >= Duration::from_millis(500) {
                        allowed = crate::platform::desktop().status().ready();
                        permission_check = Instant::now();
                    }
                    let config = inner.config.read().unwrap().clone();
                    let snapshot = Snapshot {
                        dictation: super::keys::strictly_pressed(&down, &config.dictation.keys),
                        assistant: super::keys::strictly_pressed(&down, &config.assistant.keys),
                        release: config
                            .dictation
                            .release_mode_keys
                            .as_deref()
                            .map(|keys| super::keys::strictly_pressed(&down, keys))
                            .unwrap_or(false),
                    };
                    let active =
                        inner.active.load(Ordering::SeqCst) && allowed && !first && sample != 2;
                    first = false;
                    let action = {
                        let mut machine = inner.machine.lock().unwrap();
                        #[cfg(all(feature = "atdd", debug_assertions))]
                        let snapshot = inner.atdd.lock().unwrap().snapshot(snapshot, active);
                        machine.tick(
                            snapshot,
                            active,
                            matches!(config.dictation.mode, HotkeyMode::Toggle),
                            matches!(config.assistant.mode, HotkeyMode::Toggle),
                        )
                    };
                    let Some(action) = action else {
                        continue;
                    };
                    let (mode, callback) = match action {
                        Action::Start(mode) => (mode, inner.on_start.read().unwrap().clone()),
                        Action::Stop(mode) => (mode, inner.on_stop.read().unwrap().clone()),
                    };
                    if let Some(callback) = callback {
                        callback(
                            if mode == Mode::Assistant {
                                TriggerMode::AiAssistant
                            } else {
                                TriggerMode::Dictation
                            },
                            mode == Mode::Release,
                        );
                    }
                }
            })
        {
            self.inner.started.store(false, Ordering::SeqCst);
            return Err(error.into());
        }
        if !ready_rx.recv().unwrap_or(false) {
            self.inner.started.store(false, Ordering::SeqCst);
            anyhow::bail!("无法创建 macOS 快捷键监听，请检查输入监控权限并重启应用");
        }
        Ok(())
    }
}
