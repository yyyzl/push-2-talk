//! Desktop window presentation; no speech or configuration decisions.
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn find_monitor_at_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let cursor = window.cursor_position().ok()?;
    let (cursor_x, cursor_y) = (cursor.x as i32, cursor.y as i32);
    let monitors = window.available_monitors().ok()?;

    for monitor in monitors {
        let pos = monitor.position();
        let size = monitor.size();
        if cursor_x >= pos.x
            && cursor_x < pos.x + size.width as i32
            && cursor_y >= pos.y
            && cursor_y < pos.y + size.height as i32
        {
            return Some(monitor);
        }
    }
    window.primary_monitor().ok().flatten()
}

pub(crate) fn emit_error_and_hide_overlay(app: &AppHandle, error_msg: String) {
    tracing::error!("发送错误并隐藏悬浮窗: {}", error_msg);
    let _ = app.emit("error", error_msg);

    // 隐藏悬浮窗，带重试机制
    hide_overlay_silently(app);
}

pub(crate) fn hide_overlay_silently(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败: {}", e);
            // 延迟 50ms 重试一次
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Err(e) = overlay.hide() {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }
}

pub(crate) async fn hide_overlay_window(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败: {}", e);
            // 延迟 50ms 重试一次
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Err(e) = overlay.hide() {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }
}

pub(crate) async fn show_result_panel_window(app: &AppHandle) {
    tracing::info!("[ResultPanel] show_result_panel_window 被调用");
    if let Some(panel) = app.get_webview_window("result_panel") {
        // 使用 overlay 或 main 窗口获取显示器列表（result_panel 首次显示前可能未初始化）
        let reference_window = app
            .get_webview_window("overlay")
            .or_else(|| app.get_webview_window("main"));

        if let Some(ref_win) = reference_window {
            if let Some(monitor) = find_monitor_at_cursor(&ref_win) {
                let monitor_pos = monitor.position();
                let screen_size = monitor.size();
                let scale_factor = monitor.scale_factor();

                // 结果面板逻辑尺寸（与 tauri.conf.json 一致）
                let window_width = (520.0 * scale_factor) as i32;
                let window_height = (620.0 * scale_factor) as i32;

                // 屏幕居中
                let x = monitor_pos.x + (screen_size.width as i32 - window_width) / 2;
                let y = monitor_pos.y + (screen_size.height as i32 - window_height) / 2;

                if let Err(e) = panel.set_position(tauri::PhysicalPosition::new(x, y)) {
                    tracing::warn!("设置结果面板窗口位置失败: {}", e);
                }
            }
        }

        match panel.show() {
            Ok(()) => tracing::info!("[ResultPanel] panel.show() 成功"),
            Err(e) => tracing::error!("[ResultPanel] panel.show() 失败: {}", e),
        }
        match panel.set_focus() {
            Ok(()) => tracing::info!("[ResultPanel] panel.set_focus() 成功"),
            Err(e) => tracing::warn!("[ResultPanel] panel.set_focus() 失败: {}", e),
        }
    } else {
        tracing::error!("[ResultPanel] 结果面板窗口不存在 (get_webview_window 返回 None)");
    }
}

pub(crate) async fn hide_result_panel_window(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window("result_panel") {
        if let Err(e) = panel.hide() {
            tracing::error!("隐藏结果面板窗口失败: {}", e);
        }
    }
}
