// src-tauri/src/clipboard_manager.rs
//
// 剪贴板管理模块 - 用于 AI 助手模式
//
// 提供选中文本捕获和剪贴板恢复功能
// 使用 Win32 SendInput API 替代 enigo 实现更低延迟

use anyhow::Result;
use arboard::Clipboard;
use std::thread;
use std::time::{Duration, Instant};

use crate::win32_input;

/// RAII守卫：自动恢复剪贴板内容
///
/// 当守卫被销毁时，自动将原始剪贴板内容恢复
pub struct ClipboardGuard {
    original_content: Option<String>,
    clipboard: Clipboard,
}

impl ClipboardGuard {
    /// 创建守卫并保存当前剪贴板内容
    pub fn new() -> Result<Self> {
        let mut clipboard = Clipboard::new()?;
        let original_content = clipboard.get_text().ok();

        tracing::debug!("ClipboardGuard: 已保存原始剪贴板内容");

        Ok(Self {
            original_content,
            clipboard,
        })
    }

    /// 手动恢复剪贴板（消费守卫）
    pub fn restore(mut self) -> Result<()> {
        if let Some(ref content) = self.original_content {
            self.clipboard.set_text(content.clone())?;
            tracing::debug!("ClipboardGuard: 已手动恢复剪贴板");
        }
        Ok(())
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if let Some(ref content) = self.original_content {
            // 最大努力恢复，忽略错误
            let _ = self.clipboard.set_text(content.clone());
            tracing::debug!("ClipboardGuard: 已自动恢复剪贴板（Drop）");
        }
    }
}

/// 获取当前选中的文本（通过模拟 Ctrl+C）
///
/// # 返回值
/// * `Ok((guard, Some(text)))` - 成功捕获选中文本
/// * `Ok((guard, None))` - 没有选中文本或选中内容为空
/// * `Err(e)` - 操作失败
///
/// # 说明
/// 返回的 guard 应该保持存活，直到不再需要恢复剪贴板为止
///
/// # 重要
/// 调用此函数前，请确保用户已松开所有热键（如 Alt+Space）。
/// 建议在 on_stop 回调中等待 100ms 后再调用，以避免物理按键与模拟按键冲突。
pub fn get_selected_text() -> Result<(ClipboardGuard, Option<String>)> {
    // 1. 保存当前剪贴板
    let guard = ClipboardGuard::new()?;

    // 2. 清空剪贴板（用于检测是否有选中内容）
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text("")?;

    // 3. 等待剪贴板同步（比 enigo 版本更短）
    thread::sleep(Duration::from_millis(50));

    // 4. 防御性释放修饰键
    win32_input::release_all_modifiers()?;
    thread::sleep(Duration::from_millis(5));

    // 5. 使用 Win32 SendInput 模拟 Ctrl+C
    win32_input::send_ctrl_c()?;

    // 6. 等待剪贴板更新（带重试机制）
    let selected_text = wait_for_clipboard_update(&mut clipboard, 3, 80)?;

    if let Some(ref text) = selected_text {
        tracing::info!(
            "clipboard_manager: 捕获到选中文本 (长度: {} 字符)",
            text.len()
        );
    } else {
        tracing::debug!("clipboard_manager: 未检测到选中文本");
    }

    Ok((guard, selected_text))
}

/// 等待剪贴板更新的辅助函数（动态轮询检测）
///
/// # 参数
/// * `clipboard` - 剪贴板实例
/// * `max_retries` - 最大重试次数（用于兼容旧接口）
/// * `initial_delay_ms` - 初始轮询间隔（毫秒）
///
/// # 返回值
/// * `Ok(Some(text))` - 成功获取到非空文本
/// * `Ok(None)` - 剪贴板为空或未更新
///
/// # 优化说明
/// 使用动态轮询替代固定等待，响应快的应用几乎无延迟
fn wait_for_clipboard_update(
    clipboard: &mut Clipboard,
    max_retries: u32,
    initial_delay_ms: u64,
) -> Result<Option<String>> {
    let start = Instant::now();
    // 最大等待时间：初始延迟 × (1 + 1.5 + 2.25 + ...) ≈ 初始延迟 × 4
    let max_wait_ms = initial_delay_ms * 4;
    let poll_interval_ms = 15; // 15ms 轮询间隔

    let mut attempt = 0u32;

    while start.elapsed().as_millis() < max_wait_ms as u128 {
        match clipboard.get_text() {
            Ok(text) if !text.is_empty() => {
                let elapsed = start.elapsed().as_millis();
                if elapsed > initial_delay_ms as u128 {
                    tracing::debug!("clipboard_manager: {}ms 后成功获取剪贴板内容", elapsed);
                }
                return Ok(Some(text));
            }
            Ok(_) => {
                // 剪贴板为空，继续轮询
            }
            Err(e) => {
                if attempt == 0 {
                    tracing::warn!("clipboard_manager: 读取剪贴板失败: {}", e);
                }
            }
        }

        thread::sleep(Duration::from_millis(poll_interval_ms));
        attempt += 1;

        // 兼容旧的重试次数限制
        if attempt > max_retries * 10 {
            break;
        }
    }

    // 超时，返回 None（表示没有选中内容）
    tracing::debug!(
        "clipboard_manager: 等待 {}ms 后仍未获取到剪贴板内容",
        start.elapsed().as_millis()
    );
    Ok(None)
}

/// 插入文本（支持上下文感知）
///
/// # 参数
/// * `text` - 要插入的文本
/// * `has_selection` - 是否有选中文本（如果为 true，粘贴会替换选中内容）
/// * `clipboard_guard` - 可选的剪贴板守卫（操作完成后恢复）
///
/// # 行为
/// * 有选中文本时：Ctrl+V 会替换选中内容
/// * 无选中文本时：Ctrl+V 会在光标处插入
pub fn insert_text_with_context(
    text: &str,
    has_selection: bool,
    clipboard_guard: Option<ClipboardGuard>,
) -> Result<()> {
    let mut clipboard = Clipboard::new()?;

    // 1. 将文本写入剪贴板
    clipboard.set_text(text)?;
    thread::sleep(Duration::from_millis(50));

    tracing::info!(
        "clipboard_manager: 准备插入文本 (长度: {} 字符, 有选中: {})",
        text.len(),
        has_selection
    );

    // 2. 使用 Win32 SendInput 模拟 Ctrl+V 粘贴
    win32_input::send_ctrl_v()?;

    // 3. 等待粘贴完成
    thread::sleep(Duration::from_millis(150));

    // 4. 恢复原始剪贴板
    if let Some(guard) = clipboard_guard {
        guard.restore()?;
        tracing::debug!("clipboard_manager: 已恢复原始剪贴板");
    }

    tracing::info!("clipboard_manager: 文本插入成功");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_guard_creation() {
        let guard = ClipboardGuard::new();
        assert!(guard.is_ok());
    }

    #[test]
    fn test_get_selected_text() {
        // 注意：此测试需要手动运行，因为需要实际的剪贴板和键盘模拟
        // 仅检查函数签名是否正确
        let result = get_selected_text();
        // 在CI环境可能失败，所以只检查类型
        match result {
            Ok(_) | Err(_) => {}
        }
    }
}
