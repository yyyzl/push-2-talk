// 文本插入模块
// 使用 Win32 SendInput API 替代 enigo 实现更低延迟的键盘模拟
use arboard::Clipboard;
use std::thread;
use std::time::Duration;
use anyhow::Result;

use crate::win32_input;

pub struct TextInserter {
    clipboard: Clipboard,
}

impl TextInserter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            clipboard: Clipboard::new()?,
        })
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        tracing::info!("准备插入文本: {}", text);

        // 1. 保存当前剪贴板内容
        let original_clipboard = self.clipboard.get_text().ok();

        // 2. 将文本复制到剪贴板
        self.clipboard.set_text(text)?;

        // 3. 等待剪贴板更新
        thread::sleep(Duration::from_millis(50));

        // 4. 使用 Win32 SendInput 模拟 Ctrl+V 粘贴
        win32_input::send_ctrl_v()?;

        // 5. 等待粘贴完成
        thread::sleep(Duration::from_millis(150));

        // 6. 恢复原剪贴板内容
        if let Some(original) = original_clipboard {
            self.clipboard.set_text(original)?;
        }

        tracing::info!("文本插入完成");
        Ok(())
    }
}
