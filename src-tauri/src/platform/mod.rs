// 平台抽象层入口
pub mod types;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::{audio_mute, cursor, input};
#[cfg(target_os = "windows")]
pub use windows::{audio_mute, cursor, input};
