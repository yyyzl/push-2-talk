use std::sync::atomic::{AtomicBool, Ordering};

pub struct AudioMuteManager {
    enabled: AtomicBool,
}

impl Default for AudioMuteManager {
    fn default() -> Self {
        Self::new(false)
    }
}

impl AudioMuteManager {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
        }
    }

    // macOS 精简版不执行实际静音，但仍保持开关状态一致，避免跨平台行为漂移。
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn begin_session(&self) {}

    pub fn end_session(&self) {}

    pub fn mute_other_apps(&self) -> Result<usize, String> {
        Ok(0)
    }

    pub fn restore_volumes(&self) -> Result<usize, String> {
        Ok(0)
    }
}
