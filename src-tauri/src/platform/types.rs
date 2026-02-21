#[cfg(target_os = "windows")]
pub type WindowId = isize;

#[cfg(target_os = "macos")]
pub type WindowId = i32;
