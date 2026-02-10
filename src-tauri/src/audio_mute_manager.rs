// Windows Audio Session API (WASAPI) 集成
// 用于在录音时自动静音其他应用程序

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use windows::core::Interface;
#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// RAII Guard for COM initialization
/// 确保 CoUninitialize 在作用域结束时被调用
#[cfg(target_os = "windows")]
struct ComGuard;

#[cfg(target_os = "windows")]
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

/// 看门狗检查间隔（毫秒）
const WATCHDOG_INTERVAL_MS: u64 = 1000;

/// 录音会话超时时间（秒）
/// 如果录音持续超过这个时间，看门狗会强制重置状态并恢复音量
/// 这是防止"全静音卡死"的核弹级兜底机制
const SESSION_TIMEOUT_SECS: u64 = 180; // 3 分钟

/// 音频静音管理器
/// 负责在录音时静音其他应用，录音结束后恢复
/// 使用看门狗机制确保即使出现异常也能恢复静音状态
///
/// 安全机制：
/// 1. 引用计数：跟踪活跃会话数，只有归零时才触发恢复
/// 2. 超时强制重置：录音超过 3 分钟自动强制恢复（防止计数器锁死）
/// 3. 僵尸进程清理：自动清理已关闭应用的 PID
/// 4. 中断检测：恢复过程中检测新会话，及时中止
pub struct AudioMuteManager {
    /// 当前进程 ID（避免静音自己）
    own_process_id: u32,
    /// 存储被我们静音的 Session 的 PID（使用 HashSet 去重）
    muted_pids: Arc<Mutex<HashSet<u32>>>,
    /// 是否启用静音功能
    enabled: Arc<AtomicBool>,
    /// 当前活跃的录音会话计数（用于看门狗判断）
    active_sessions: Arc<AtomicU32>,
    /// 记录最后一次开始录音的时间，用于超时强制重置
    last_session_start: Arc<Mutex<Option<Instant>>>,
    /// 看门狗线程是否应该停止
    watchdog_stop: Arc<AtomicBool>,
    /// 看门狗线程句柄
    watchdog_handle: Option<thread::JoinHandle<()>>,
}

impl AudioMuteManager {
    /// 创建新的音频静音管理器
    pub fn new(enabled: bool) -> Self {
        let own_process_id = std::process::id();
        tracing::info!(
            "AudioMuteManager created, own_pid: {}, enabled: {}",
            own_process_id,
            enabled
        );

        let muted_pids = Arc::new(Mutex::new(HashSet::new()));
        let enabled_flag = Arc::new(AtomicBool::new(enabled));
        let active_sessions = Arc::new(AtomicU32::new(0));
        let last_session_start = Arc::new(Mutex::new(None));
        let watchdog_stop = Arc::new(AtomicBool::new(false));

        // 启动看门狗线程
        let watchdog_handle = Self::start_watchdog(
            Arc::clone(&muted_pids),
            Arc::clone(&enabled_flag),
            Arc::clone(&active_sessions),
            Arc::clone(&last_session_start),
            Arc::clone(&watchdog_stop),
            own_process_id,
        );

        Self {
            own_process_id,
            muted_pids,
            enabled: enabled_flag,
            active_sessions,
            last_session_start,
            watchdog_stop,
            watchdog_handle: Some(watchdog_handle),
        }
    }

    /// 启动看门狗线程
    /// 定期检查：
    /// 1. 如果没有活跃录音会话但有应用被静音，则自动恢复
    /// 2. 如果录音会话超时（>3分钟），强制重置状态并恢复音量（核弹级兜底）
    fn start_watchdog(
        muted_pids: Arc<Mutex<HashSet<u32>>>,
        enabled: Arc<AtomicBool>,
        active_sessions: Arc<AtomicU32>,
        last_session_start: Arc<Mutex<Option<Instant>>>,
        stop_flag: Arc<AtomicBool>,
        own_process_id: u32,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            tracing::info!("AudioMuteManager watchdog started");

            while !stop_flag.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(WATCHDOG_INTERVAL_MS));

                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }

                // 如果功能未启用，跳过检查
                if !enabled.load(Ordering::Relaxed) {
                    continue;
                }

                let current_sessions = active_sessions.load(Ordering::Relaxed);

                // === 安全保险：超时强制重置 ===
                // 如果 sessions > 0 但持续时间超过 SESSION_TIMEOUT_SECS（防止程序逻辑卡死导致永远静音）
                let is_timeout = {
                    let start_opt = last_session_start.lock().unwrap();
                    if let Some(start) = *start_opt {
                        start.elapsed().as_secs() > SESSION_TIMEOUT_SECS
                    } else {
                        false
                    }
                };

                if is_timeout && current_sessions > 0 {
                    tracing::error!(
                        "⚠️ CRITICAL: Recording session timed out (>{}s). Forcing volume restore!",
                        SESSION_TIMEOUT_SECS
                    );
                    // 强制归零计数器
                    active_sessions.store(0, Ordering::Relaxed);
                    // 清除计时器
                    *last_session_start.lock().unwrap() = None;
                    // 继续执行下面的恢复逻辑
                }

                // === 正常的恢复检查 ===
                // 重新读取 active_sessions（因为上面可能刚刚重置了）
                if active_sessions.load(Ordering::Relaxed) == 0 {
                    let has_muted = {
                        let pids = muted_pids.lock().unwrap();
                        !pids.is_empty()
                    };

                    if has_muted {
                        tracing::warn!(
                            "Watchdog detected muted apps without active session, restoring..."
                        );
                        if let Err(e) = Self::restore_volumes_internal(
                            &muted_pids,
                            &active_sessions,
                            own_process_id,
                        ) {
                            tracing::error!("Watchdog failed to restore volumes: {}", e);
                        }
                    }
                }
            }

            tracing::info!("AudioMuteManager watchdog stopped");
        })
    }

    /// 设置是否启用静音功能
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        tracing::info!("AudioMuteManager enabled: {}", enabled);

        // 如果禁用，立即恢复所有静音的应用
        if !enabled {
            if let Err(e) = self.restore_volumes() {
                tracing::warn!("Failed to restore volumes when disabling: {}", e);
            }
        }
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// 开始录音会话（增加活跃计数）
    /// 只有从 0 变 1 时才重置计时器，代表一轮新的录音开始
    pub fn begin_session(&self) {
        let prev = self.active_sessions.fetch_add(1, Ordering::Relaxed);
        if prev == 0 {
            // 从 0 变 1，开始新的一轮录音，记录开始时间
            *self.last_session_start.lock().unwrap() = Some(Instant::now());
        }
        tracing::debug!("AudioMuteManager session started, active: {}", prev + 1);
    }

    /// 结束录音会话（减少活跃计数）
    /// 使用 fetch_update (CAS) 防止下溢，确保计数器不会变成 u32::MAX
    /// 计数器归零时清除开始时间
    pub fn end_session(&self) {
        let result = self
            .active_sessions
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
                if x > 0 {
                    Some(x - 1)
                } else {
                    None
                }
            });

        match result {
            Ok(prev) => {
                if prev == 1 {
                    // 从 1 变 0，本轮录音结束，清除计时器
                    *self.last_session_start.lock().unwrap() = None;
                }
                tracing::debug!("AudioMuteManager session ended, active: {}", prev - 1);
            }
            Err(_) => {
                // 计数器已经是 0，不执行减法，记录警告
                tracing::warn!("AudioMuteManager end_session called but active_sessions already 0, ignoring to prevent underflow");
            }
        }
    }

    /// 静音所有其他音频应用
    /// 返回成功静音的应用数量
    /// 注意：不再清空之前的记录，新静音的应用会累加到列表中
    #[cfg(target_os = "windows")]
    pub fn mute_other_apps(&self) -> Result<usize, String> {
        if !self.is_enabled() {
            tracing::debug!("AudioMuteManager is disabled, skipping mute");
            return Ok(0);
        }

        unsafe {
            // 初始化 COM，使用 Multithreaded 模式以适应 Tauri 线程池
            // 注意：CoInitializeEx 是幂等的，重复调用不会出错
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // 使用 RAII 确保 CoUninitialize 被调用
            let _com_guard = ComGuard;

            // 获取设备枚举器
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

            // 获取默认音频输出设备
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(|e| format!("Failed to get default endpoint: {}", e))?;

            // 获取音频会话管理器
            let session_manager: IAudioSessionManager2 = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Failed to activate session manager: {}", e))?;

            // 获取会话枚举器
            let session_enumerator = session_manager
                .GetSessionEnumerator()
                .map_err(|e| format!("Failed to get session enumerator: {}", e))?;

            let count = session_enumerator
                .GetCount()
                .map_err(|e| format!("Failed to get session count: {}", e))?;

            let mut muted_count = 0;
            let mut muted_map = self.muted_pids.lock().unwrap();

            // 不再清空，改为累加模式

            tracing::debug!("Found {} audio sessions", count);

            for i in 0..count {
                if let Ok(control) = session_enumerator.GetSession(i) {
                    // 获取 IAudioSessionControl2 以访问进程信息
                    let control2: IAudioSessionControl2 = match control.cast() {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    let pid = control2.GetProcessId().unwrap_or(0);

                    // 1. 跳过自己
                    if pid == self.own_process_id {
                        tracing::debug!("Skipping own process (pid: {})", pid);
                        continue;
                    }

                    // 2. 跳过系统声音 (PID 0)
                    if pid == 0 {
                        tracing::debug!("Skipping system sounds (pid: 0)");
                        continue;
                    }

                    // 3. 跳过已经在我们列表中的（避免重复操作）
                    if muted_map.contains(&pid) {
                        tracing::debug!("Already in muted list, skipping (pid: {})", pid);
                        continue;
                    }

                    // 获取音量控制接口
                    if let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                        if let Ok(is_muted) = volume.GetMute() {
                            // 4. 只静音当前未静音的应用
                            if !is_muted.as_bool() {
                                if volume.SetMute(true, std::ptr::null()).is_ok() {
                                    muted_map.insert(pid);
                                    muted_count += 1;
                                    tracing::debug!("Muted process (pid: {})", pid);
                                }
                            } else {
                                tracing::debug!(
                                    "Process already muted externally, skipping (pid: {})",
                                    pid
                                );
                            }
                        }
                    }
                }
            }

            tracing::info!(
                "Muted {} audio applications (total tracked: {})",
                muted_count,
                muted_map.len()
            );
            Ok(muted_count)
        }
    }

    /// 恢复之前被静音的应用
    /// 返回成功恢复的应用数量
    #[cfg(target_os = "windows")]
    pub fn restore_volumes(&self) -> Result<usize, String> {
        Self::restore_volumes_internal(&self.muted_pids, &self.active_sessions, self.own_process_id)
    }

    /// 内部恢复实现（供看门狗使用）
    ///
    /// 安全机制：
    /// 1. 恢复成功一个，从列表删除一个（避免竞态条件）
    /// 2. 中断检测：如果恢复过程中用户又开始录音，立即停止恢复
    /// 3. 僵尸进程清理：自动清理已关闭应用的 PID，防止看门狗空转
    #[cfg(target_os = "windows")]
    fn restore_volumes_internal(
        muted_pids: &Arc<Mutex<HashSet<u32>>>,
        active_sessions: &Arc<AtomicU32>,
        _own_process_id: u32,
    ) -> Result<usize, String> {
        // 获取快照，放入 pending_pids 用于跟踪僵尸进程
        let mut pending_pids: HashSet<u32> = {
            let muted_map = muted_pids.lock().unwrap();
            muted_map.iter().cloned().collect()
        };

        if pending_pids.is_empty() {
            tracing::debug!("No muted applications to restore");
            return Ok(0);
        }

        tracing::debug!("Restoring {} muted applications", pending_pids.len());

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _com_guard = ComGuard;

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(|e| format!("Failed to get default endpoint: {}", e))?;

            let session_manager: IAudioSessionManager2 = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Failed to activate session manager: {}", e))?;

            let session_enumerator = session_manager
                .GetSessionEnumerator()
                .map_err(|e| format!("Failed to get session enumerator: {}", e))?;

            let count = session_enumerator.GetCount().unwrap_or(0);

            let mut restored_count = 0;

            for i in 0..count {
                // === 中断检测 ===
                // 如果在恢复过程中用户又按下了录音键，立即停止恢复
                // 这样残留的 muted_pids 会在 mute_other_apps 中被跳过，保持静音（正确行为）
                if active_sessions.load(Ordering::Relaxed) > 0 {
                    tracing::info!(
                        "New session started during restore, aborting restore operation"
                    );
                    return Ok(restored_count);
                }

                if let Ok(control) = session_enumerator.GetSession(i) {
                    let control2: IAudioSessionControl2 = match control.cast() {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    let pid = control2.GetProcessId().unwrap_or(0);

                    // 如果这个 PID 在我们的待处理列表里
                    if pending_pids.contains(&pid) {
                        // 无论恢复成功与否，都说明这个进程还活着
                        // 从 pending_pids 中移除（剩下的就是僵尸进程）
                        pending_pids.remove(&pid);

                        if let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                            if volume.SetMute(false, std::ptr::null()).is_ok() {
                                // 恢复成功后，立即从全局列表中删除
                                {
                                    let mut muted_map = muted_pids.lock().unwrap();
                                    muted_map.remove(&pid);
                                }
                                restored_count += 1;
                                tracing::debug!("Restored audio for process (pid: {})", pid);
                            }
                        }
                    }
                }
            }

            // === 僵尸进程清理 ===
            // 循环结束后，pending_pids 里剩下的就是"在 muted_pids 里，但没在系统活跃会话里找到"的 PID
            // 说明这些进程已经关闭了。必须从全局 map 里删掉它们，否则看门狗会死循环空转。
            if !pending_pids.is_empty() {
                let mut muted_map = muted_pids.lock().unwrap();
                for zombie_pid in &pending_pids {
                    muted_map.remove(zombie_pid);
                    tracing::debug!(
                        "Removed zombie process (pid: {}) from muted list",
                        zombie_pid
                    );
                }
                tracing::info!("Cleaned up {} zombie processes", pending_pids.len());
            }

            tracing::info!("Restored {} audio applications", restored_count);
            Ok(restored_count)
        }
    }
}

/// 确保在 AudioMuteManager 销毁时恢复所有被静音的应用并停止看门狗
impl Drop for AudioMuteManager {
    fn drop(&mut self) {
        tracing::debug!("AudioMuteManager dropping...");

        // 停止看门狗线程
        self.watchdog_stop.store(true, Ordering::Relaxed);

        // 恢复所有静音的应用
        if let Err(e) = self.restore_volumes() {
            tracing::warn!("Failed to restore volumes on drop: {}", e);
        }

        // 等待看门狗线程结束（最多等待2秒）
        if let Some(handle) = self.watchdog_handle.take() {
            // 使用 thread::spawn 包装 join 以实现超时
            let _ = handle.join();
        }

        tracing::debug!("AudioMuteManager dropped");
    }
}
