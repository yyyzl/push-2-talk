use anyhow::{ensure, Result};
use arboard::Clipboard;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
}
fn revision() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Retains the Windows text clipboard behavior, with revision/one-shot protection.
/// Rich-format snapshots are currently a macOS capability.
pub struct ClipboardSession {
    clipboard: Clipboard,
    original: Option<String>,
    revision: u32,
    modified: bool,
    finished: bool,
}
impl ClipboardSession {
    pub fn new() -> Result<Self> {
        let before = revision();
        let mut clipboard = Clipboard::new()?;
        let original = clipboard.get_text().ok();
        ensure!(revision() == before, "剪贴板正在变化，已取消操作");
        Ok(Self {
            clipboard,
            original,
            revision: before,
            modified: false,
            finished: false,
        })
    }
    pub fn write_text(&mut self, text: &str) -> Result<()> {
        self.ensure_owned()?;
        self.clipboard.set_text(text)?;
        self.revision = revision();
        self.modified = true;
        Ok(())
    }
    pub fn ensure_owned(&self) -> Result<()> {
        ensure!(
            !self.finished && revision() == self.revision,
            "剪贴板已更新，已取消本次剪贴板操作"
        );
        Ok(())
    }
    pub fn claim_copy_result(&mut self, text: &str) -> Result<()> {
        let before = revision();
        ensure!(
            !self.finished
                && self.modified
                && !text.is_empty()
                && before != self.revision
                && self.clipboard.get_text()? == text
                && revision() == before,
            "剪贴板已更新，已取消本次剪贴板操作"
        );
        self.revision = before;
        Ok(())
    }
    pub fn restore(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        let restore = self.modified && revision() == self.revision;
        self.finished = true;
        if restore {
            if let Some(text) = &self.original {
                self.clipboard.set_text(text.as_str())?;
            }
        }
        Ok(())
    }
}
