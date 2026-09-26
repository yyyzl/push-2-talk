use crate::{clipboard_manager, platform::InputTarget};
use anyhow::Result;

pub struct TextInserter;

impl TextInserter {
    pub fn new() -> Result<Self> {
        // Preserve startup availability checking; snapshot only at insertion time.
        arboard::Clipboard::new()?;
        Ok(Self)
    }

    pub fn insert_text(&mut self, text: &str, target: Option<InputTarget>) -> Result<()> {
        clipboard_manager::insert_text_with_context(text, false, None, target)
    }
}
