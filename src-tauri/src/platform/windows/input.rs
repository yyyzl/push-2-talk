use anyhow::Result;

use crate::platform::types::WindowId;

pub fn send_copy() -> Result<()> {
    crate::win32_input::send_ctrl_c()
}

pub fn send_paste() -> Result<()> {
    crate::win32_input::send_ctrl_v()
}

pub fn release_all_modifiers() -> Result<()> {
    crate::win32_input::release_all_modifiers()
}

pub fn get_foreground_window() -> Option<WindowId> {
    crate::win32_input::get_foreground_window()
}

pub fn is_window_valid(id: WindowId) -> bool {
    crate::win32_input::is_window_valid(id)
}

#[allow(dead_code)]
pub fn force_foreground_window(id: WindowId) -> Result<()> {
    crate::win32_input::force_foreground_window(id)
}

pub fn restore_focus_with_verify(id: WindowId, max_retries: u32) -> bool {
    crate::win32_input::restore_focus_with_verify(id, max_retries)
}

pub fn verify_foreground_window(id: WindowId) -> bool {
    crate::win32_input::verify_foreground_window(id)
}
