// 文本插入模块
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;
use anyhow::Result;

pub struct TextInserter {
    clipboard: Clipboard,
    enigo: Enigo,
}

impl TextInserter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            clipboard: Clipboard::new()?,
            enigo: Enigo::new(&Settings::default())?,
        })
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        tracing::info!("准备插入文本: {}", text);

        // 1. 保存当前剪贴板内容
        let original_clipboard = self.clipboard.get_text().ok();

        // 2. 将文本复制到剪贴板
        self.clipboard.set_text(text)?;

        // 3. 等待一小段时间确保剪贴板已更新
        thread::sleep(Duration::from_millis(50));

        // 4. 模拟 Ctrl+V 粘贴
        self.enigo.key(Key::Control, Direction::Press)?;
        let result = (|| -> Result<()> {
            thread::sleep(Duration::from_millis(10));
            self.enigo.key(Key::Unicode('v'), Direction::Click)?;
            thread::sleep(Duration::from_millis(10));
            Ok(())
        })();
        // 最大努力保证 Ctrl 不会遗留为按下状态
        let _ = self.enigo.key(Key::Control, Direction::Release);
        result?;

        // 5. 等待粘贴完成
        thread::sleep(Duration::from_millis(100));

        // 6. 恢复原剪贴板内容
        if let Some(original) = original_clipboard {
            self.clipboard.set_text(original)?;
        }

        tracing::info!("文本插入完成");
        Ok(())
    }
}
