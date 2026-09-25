//! Explicitly unavailable until Core Audio process taps are validated. Never alter master volume.
pub struct AudioMuteManager;
impl AudioMuteManager {
    pub fn new(enabled: bool) -> Self {
        let manager = Self;
        manager.set_enabled(enabled);
        manager
    }
    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            tracing::warn!("macOS 暂不支持静音其他应用");
        }
    }
    pub fn is_enabled(&self) -> bool {
        false
    }
    pub fn begin_session(&self) {}
    pub fn end_session(&self) {}
    pub fn mute_other_apps(&self) -> Result<usize, String> {
        Err("macOS 暂不支持静音其他应用".into())
    }
    pub fn restore_volumes(&self) -> Result<usize, String> {
        Ok(0)
    }
}
