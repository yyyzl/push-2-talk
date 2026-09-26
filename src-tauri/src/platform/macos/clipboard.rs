use anyhow::{bail, Context, Result};
use std::{ffi::c_void, ptr::NonNull};

unsafe extern "C" {
    fn ptt_clipboard_begin() -> *mut c_void;
    fn ptt_clipboard_set_text(handle: *mut c_void, bytes: *const u8, length: usize) -> i32;
    fn ptt_clipboard_claim_copy(handle: *mut c_void, bytes: *const u8, length: usize) -> i32;
    fn ptt_clipboard_owned(handle: *mut c_void) -> i32;
    fn ptt_clipboard_restore(handle: *mut c_void) -> i32;
    fn ptt_clipboard_dispose(handle: *mut c_void);
}

/// Owns the retained native snapshot. Kept inside a single synchronous transaction.
pub struct ClipboardSession(NonNull<c_void>);

fn check(status: i32) -> Result<()> {
    match status {
        0 => Ok(()),
        1 => bail!("剪贴板已更新，已取消本次剪贴板操作"),
        _ => bail!("macOS 剪贴板操作失败"),
    }
}

impl ClipboardSession {
    pub fn new() -> Result<Self> {
        NonNull::new(unsafe { ptt_clipboard_begin() })
            .map(Self)
            .context("无法完整保存剪贴板（内容不可读取、超过 64 MiB 或正在变化），已取消操作")
    }
    pub fn write_text(&mut self, text: &str) -> Result<()> {
        check(unsafe { ptt_clipboard_set_text(self.0.as_ptr(), text.as_ptr(), text.len()) })
    }
    pub fn claim_copy_result(&mut self, text: &str) -> Result<()> {
        check(unsafe { ptt_clipboard_claim_copy(self.0.as_ptr(), text.as_ptr(), text.len()) })
    }
    pub fn ensure_owned(&self) -> Result<()> {
        check(unsafe { ptt_clipboard_owned(self.0.as_ptr()) })
    }
    pub fn restore(&mut self) -> Result<()> {
        check(unsafe { ptt_clipboard_restore(self.0.as_ptr()) })
    }
}

impl Drop for ClipboardSession {
    fn drop(&mut self) {
        // Guard normally restores first; native restoration is idempotent.
        unsafe {
            ptt_clipboard_restore(self.0.as_ptr());
            ptt_clipboard_dispose(self.0.as_ptr());
        }
    }
}
