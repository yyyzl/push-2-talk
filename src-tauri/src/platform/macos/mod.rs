use super::*;
use std::{
    ffi::{c_char, c_void, CStr},
    thread,
    time::Duration,
};
#[cfg(all(feature = "atdd", debug_assertions))]
mod atdd_control;
pub mod audio_mute;
mod hotkey_state;
pub mod hotkeys;
mod key_snapshot;
mod keys;
extern "C" {
    fn ptt_capture_target() -> u64;
    fn ptt_target_valid(token: u64) -> bool;
    fn ptt_target_focused(token: u64) -> bool;
    fn ptt_restore_target(token: u64) -> bool;
    fn ptt_read_text(token: u64) -> *mut c_char;
    fn ptt_free_string(value: *mut c_char);
    fn ptt_key_down(code: u16) -> bool;
    fn ptt_hotkeys_start() -> bool;
    fn ptt_hotkeys_next(down: *mut u8) -> i32;
    fn ptt_accessibility() -> bool;
    fn ptt_input_monitoring() -> bool;
    fn ptt_microphone() -> i32;
    fn ptt_request_permission(kind: i32);
    fn ptt_send_shortcut(code: u16) -> bool;
    fn ptt_configure_overlay(window: *mut c_void);
}
pub struct MacDesktop;

#[cfg(all(feature = "atdd", debug_assertions))]
pub fn atdd_prepare_fixture(
    path: &std::path::Path,
    expected: &str,
    start: u64,
    length: u64,
) -> Result<InputTarget> {
    extern "C" {
        fn ptt_atdd_open_fixture(path: *const c_char) -> bool;
        fn ptt_atdd_fixture_focused(token: u64, path: *const c_char) -> bool;
        fn ptt_atdd_select_fixture(
            token: u64,
            path: *const c_char,
            expected: *const c_char,
            start: u64,
            length: u64,
        ) -> bool;
    }
    let path = std::ffi::CString::new(path.to_string_lossy().as_bytes())?;
    let expected = std::ffi::CString::new(expected)?;
    anyhow::ensure!(
        unsafe { ptt_atdd_open_fixture(path.as_ptr()) },
        "无法打开验收文档，请确认所选测试应用已安装"
    );
    let mut observed = None;
    for _ in 0..30 {
        observed = desktop().capture_target();
        if let Some(target) = observed {
            if unsafe { ptt_atdd_fixture_focused(target.0, path.as_ptr()) } {
                anyhow::ensure!(
                    unsafe {
                        ptt_atdd_select_fixture(
                            target.0,
                            path.as_ptr(),
                            expected.as_ptr(),
                            start,
                            length,
                        )
                    },
                    "验收文档内容或选区不匹配，已停止本轮验收；{}",
                    atdd_target_description(Some(target))
                );
                return Ok(target);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!(
        "验收文档未获得系统输入焦点，已停止本轮验收；当前目标：{}",
        atdd_target_description(observed)
    )
}

#[cfg(all(feature = "atdd", debug_assertions))]
pub fn atdd_target_description(target: Option<InputTarget>) -> String {
    extern "C" {
        fn ptt_atdd_target_description(token: u64) -> *mut c_char;
    }
    let pointer = unsafe { ptt_atdd_target_description(target.map_or(0, |t| t.0)) };
    if pointer.is_null() {
        return "目标诊断不可用".into();
    }
    let description = unsafe { CStr::from_ptr(pointer).to_string_lossy().into_owned() };
    unsafe { ptt_free_string(pointer) };
    description
}
impl TargetAccess for MacDesktop {
    fn is_valid(&self, target: InputTarget) -> bool {
        unsafe { ptt_target_valid(target.0) }
    }
    fn is_focused(&self, target: InputTarget) -> bool {
        unsafe { ptt_target_focused(target.0) }
    }
    fn restore_focus(&self, target: InputTarget) -> bool {
        if !unsafe { ptt_restore_target(target.0) } {
            return false;
        }
        let deadline = std::time::Instant::now() + Duration::from_millis(1500);
        while std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(30));
            if self.is_focused(target) {
                return true;
            }
        }
        false
    }
}
impl DesktopBackend for MacDesktop {
    fn capture_target(&self) -> Option<InputTarget> {
        let token = unsafe { ptt_capture_target() };
        (token != 0).then_some(InputTarget(token))
    }
    fn copy_selection(&self) -> Result<()> {
        send_shortcut(8)
    }
    fn paste(&self) -> Result<()> {
        send_shortcut(9)
    }
    fn release_modifiers(&self) -> Result<()> {
        // Do not synthesize releases for physically held keys on macOS. Wait, then fail closed.
        for _ in 0..50 {
            if !keys::MODIFIER_CODES
                .iter()
                .any(|code| unsafe { ptt_key_down(*code) })
            {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        anyhow::bail!("请松开快捷键后重试")
    }
    fn read_text(&self, target: InputTarget) -> Result<String> {
        let pointer = unsafe { ptt_read_text(target.0) };
        anyhow::ensure!(!pointer.is_null(), "此输入控件不支持文本读取，或已失去焦点");
        let text = unsafe { CStr::from_ptr(pointer).to_string_lossy().into_owned() };
        unsafe { ptt_free_string(pointer) };
        Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
    }
    fn status(&self) -> PlatformStatus {
        PlatformStatus {
            os: "macos",
            microphone: match unsafe { ptt_microphone() } {
                3 => PermissionState::Granted,
                0 => PermissionState::NotDetermined,
                1 => PermissionState::Restricted,
                _ => PermissionState::Denied,
            },
            accessibility: if unsafe { ptt_accessibility() } {
                PermissionState::Granted
            } else {
                PermissionState::Denied
            },
            input_monitoring: if unsafe { ptt_input_monitoring() } {
                PermissionState::Granted
            } else {
                PermissionState::Denied
            },
            other_app_mute: false,
            text_observation: true,
        }
    }
    fn request_permission(&self, permission: &str) -> Result<()> {
        let kind = match permission {
            "microphone" => 0,
            "accessibility" => 1,
            "input_monitoring" => 2,
            _ => anyhow::bail!("未知权限"),
        };
        unsafe { ptt_request_permission(kind) };
        Ok(())
    }
}
fn send_shortcut(code: u16) -> Result<()> {
    anyhow::ensure!(
        unsafe { ptt_send_shortcut(code) },
        "请在系统设置中允许 PushToTalk 使用辅助功能"
    );
    Ok(())
}

pub fn configure_windows(app: &tauri::App, start_minimized: bool) {
    use tauri::Manager;
    for label in ["overlay", "notification"] {
        if let Some(window) = app.get_webview_window(label) {
            if let Ok(pointer) = window.ns_window() {
                unsafe { ptt_configure_overlay(pointer) };
            }
        }
    }
    if !start_minimized {
        show_main_window(app.handle());
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn handle_run_event(app: &tauri::AppHandle, event: &tauri::RunEvent) {
    if let tauri::RunEvent::Reopen { .. } = event {
        show_main_window(app);
    }
}
