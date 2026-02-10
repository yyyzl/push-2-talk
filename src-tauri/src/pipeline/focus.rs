// 焦点管理模块
//
// 提供悬浮窗隐藏和目标窗口焦点恢复功能
// 确保文本能正确粘贴到用户原本操作的窗口

use crate::win32_input;
use tauri::{AppHandle, Manager};

/// 隐藏悬浮窗并恢复目标窗口焦点
///
/// 这是文本插入前的关键步骤，确保：
/// 1. 悬浮窗被隐藏
/// 2. 焦点正确恢复到用户触发热键时的窗口
///
/// # 参数
/// * `app` - Tauri 应用句柄
/// * `target_hwnd` - 目标窗口句柄（热键按下时保存的）
///
/// # 流程
/// 1. 隐藏悬浮窗
/// 2. 等待 50ms 让 Windows 处理窗口消息
/// 3. 主动恢复目标窗口焦点（带验证，最多重试 3 次）
/// 4. 等待 100ms 焦点稳定
pub async fn hide_overlay_and_restore_focus(app: &AppHandle, target_hwnd: Option<isize>) {
    // 1. 隐藏悬浮窗
    if let Some(overlay) = app.get_webview_window("overlay") {
        if overlay.is_visible().unwrap_or(false) {
            tracing::info!("Pipeline: 隐藏悬浮窗...");
            let _ = overlay.hide();
        }
    }

    // 2. 等待 Windows 处理窗口消息
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 3. 主动恢复目标窗口焦点
    if let Some(hwnd) = target_hwnd {
        tracing::info!("Pipeline: 恢复目标窗口焦点 (0x{:X})...", hwnd);

        // 检查窗口是否仍然有效
        if !win32_input::is_window_valid(hwnd) {
            tracing::warn!("Pipeline: 目标窗口已无效，跳过焦点恢复");
        } else {
            // 尝试恢复焦点（最多重试 3 次）
            let success = win32_input::restore_focus_with_verify(hwnd, 3);

            if success {
                tracing::info!("Pipeline: 焦点恢复成功");
            } else {
                tracing::warn!("Pipeline: 焦点恢复失败，粘贴可能不会生效到目标窗口");
            }
        }
    } else {
        tracing::warn!("Pipeline: 没有保存目标窗口句柄，跳过焦点恢复");
    }

    // 4. 等待焦点稳定
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

/// 验证当前焦点是否在目标窗口
///
/// 用于在粘贴前进行最后检查
#[allow(dead_code)]
pub fn verify_focus(target_hwnd: Option<isize>) -> bool {
    match target_hwnd {
        Some(hwnd) => win32_input::verify_foreground_window(hwnd),
        None => false,
    }
}
