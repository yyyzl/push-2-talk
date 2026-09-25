//! Native capability boundary. Business code never interprets a window handle or AX object.
mod contract;
use anyhow::Result;
pub use contract::{prepare_target, InputTarget, TargetAccess};
use serde::Serialize;
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(all(target_os = "macos", feature = "atdd", debug_assertions))]
pub use macos::{atdd_prepare_fixture, atdd_target_description};
#[cfg(target_os = "macos")]
pub use macos::{audio_mute::AudioMuteManager, hotkeys::HotkeyService};
#[cfg(target_os = "windows")]
pub use windows::{audio_mute_manager::AudioMuteManager, hotkey_service::HotkeyService};
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
compile_error!("PushToTalk supports Windows and macOS only");

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Granted,
    NotDetermined,
    Denied,
    Restricted,
}
#[derive(Debug, Clone, Serialize)]
pub struct PlatformStatus {
    pub os: &'static str,
    pub microphone: PermissionState,
    pub accessibility: PermissionState,
    pub input_monitoring: PermissionState,
    pub other_app_mute: bool,
    pub text_observation: bool,
}
impl PlatformStatus {
    pub fn ready(&self) -> bool {
        matches!(self.microphone, PermissionState::Granted)
            && matches!(self.accessibility, PermissionState::Granted)
            && matches!(self.input_monitoring, PermissionState::Granted)
    }
}

/// Semantic operations: Windows owns SendInput/UIA; macOS owns AX/AppKit/Core Graphics.
pub trait DesktopBackend: TargetAccess + Send + Sync {
    fn capture_target(&self) -> Option<InputTarget>;
    fn copy_selection(&self) -> Result<()>;
    fn paste(&self) -> Result<()>;
    fn release_modifiers(&self) -> Result<()>;
    fn read_text(&self, target: InputTarget) -> Result<String>;
    fn status(&self) -> PlatformStatus;
    fn request_permission(&self, permission: &str) -> Result<()>;
}

fn create_backend() -> Box<dyn DesktopBackend> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsDesktop)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacDesktop)
    }
}

pub fn desktop() -> &'static dyn DesktopBackend {
    static BACKEND: OnceLock<Box<dyn DesktopBackend>> = OnceLock::new();
    BACKEND.get_or_init(create_backend).as_ref()
}

/// Fail closed immediately before simulated input (do not reactivate after the clipboard delay).
pub fn verify_insertion_target(target: Option<InputTarget>) -> Result<()> {
    let target = target.ok_or(contract::InteractionError::MissingTarget)?;
    anyhow::ensure!(
        desktop().is_valid(target),
        contract::InteractionError::TargetGone
    );
    desktop().release_modifiers()?;
    anyhow::ensure!(
        desktop().is_focused(target),
        contract::InteractionError::FocusDenied
    );
    Ok(())
}

pub fn configure_windows(app: &tauri::App, start_minimized: bool) {
    #[cfg(target_os = "macos")]
    macos::configure_windows(app, start_minimized);
    #[cfg(target_os = "windows")]
    let _ = (app, start_minimized);
}

pub fn handle_run_event(app: &tauri::AppHandle, event: &tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    macos::handle_run_event(app, event);
    #[cfg(target_os = "windows")]
    let _ = (app, event);
}
