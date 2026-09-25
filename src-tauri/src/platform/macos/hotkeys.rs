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
    on_start: RwLock<Option<Callback>>,
    on_stop: RwLock<Option<Callback>>,
}
pub struct HotkeyService {
    inner: Arc<Inner>,
}
impl HotkeyService {
    #[cfg(all(feature = "atdd", debug_assertions))]
    pub fn atdd_recording(&self, start: bool) -> Result<()> {
        anyhow::ensure!(
            self.is_service_active() && crate::platform::desktop().status().ready(),
            "录音服务或系统权限不可用"
        );
        let callback = {
            let mut machine = self.inner.machine.lock().unwrap();
            if start {
                anyhow::ensure!(machine.recording.is_none(), "已有录音进行中");
                machine.tick(Snapshot::default(), true, false, false);
                machine.tick(
                    Snapshot {
                        release: true,
                        ..Snapshot::default()
                    },
                    true,
                    false,
                    false,
                );
                machine.tick(Snapshot::default(), true, false, false);
                self.inner.on_start.read().unwrap().clone()
            } else {
                // A user cancellation/reset must not accidentally start another recording.
                if machine.recording != Some(Mode::Release) {
                    return Ok(());
                }
                machine.reset();
                self.inner.on_stop.read().unwrap().clone()
            }
        };
        callback.ok_or_else(|| anyhow::anyhow!("录音回调尚未初始化"))?(
            TriggerMode::Dictation,
            true,
        );
        Ok(())
    }
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                active: AtomicBool::new(false),
                started: AtomicBool::new(false),
                config: RwLock::new(DualHotkeyConfig::default()),
                machine: Mutex::new(Machine::default()),
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
        self.inner.machine.lock().unwrap().reset();
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
                    let action = inner.machine.lock().unwrap().tick(
                        snapshot,
                        active,
                        matches!(config.dictation.mode, HotkeyMode::Toggle),
                        matches!(config.assistant.mode, HotkeyMode::Toggle),
                    );
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
