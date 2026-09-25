use super::*;
pub mod audio_mute_manager;
pub mod hotkey_service;
mod uia_text_reader;
mod win32_input;

pub struct WindowsDesktop;
impl TargetAccess for WindowsDesktop {
    fn is_valid(&self, target: InputTarget) -> bool {
        win32_input::is_window_valid(target.0 as isize)
    }
    fn is_focused(&self, target: InputTarget) -> bool {
        win32_input::verify_foreground_window(target.0 as isize)
    }
    fn restore_focus(&self, target: InputTarget) -> bool {
        win32_input::restore_focus_with_verify(target.0 as isize, 3)
    }
}
impl DesktopBackend for WindowsDesktop {
    fn capture_target(&self) -> Option<InputTarget> {
        win32_input::get_foreground_window().map(|v| InputTarget(v as u64))
    }
    fn copy_selection(&self) -> Result<()> {
        win32_input::send_ctrl_c()
    }
    fn paste(&self) -> Result<()> {
        win32_input::send_ctrl_v()
    }
    fn release_modifiers(&self) -> Result<()> {
        win32_input::release_all_modifiers()
    }
    fn read_text(&self, target: InputTarget) -> Result<String> {
        uia_text_reader::get_focused_window_text(target.0 as isize)
    }
    fn status(&self) -> PlatformStatus {
        PlatformStatus {
            os: "windows",
            microphone: PermissionState::Granted,
            accessibility: PermissionState::Granted,
            input_monitoring: PermissionState::Granted,
            other_app_mute: true,
            text_observation: true,
        }
    }
    fn request_permission(&self, _: &str) -> Result<()> {
        Ok(())
    }
}
