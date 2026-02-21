use tauri::{AppHandle, Manager};

use crate::AppState;

#[tauri::command]
pub async fn hide_to_tray(app_handle: AppHandle) -> Result<String, String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        return Ok("已隐藏到菜单栏".to_string());
    }

    #[cfg(not(target_os = "macos"))]
    {
        return Ok("已最小化到托盘".to_string());
    }
}

#[tauri::command]
pub async fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    // 先停止服务
    let state = app_handle.state::<AppState>();
    {
        let mut is_running = state.is_running.lock().unwrap();
        if *is_running {
            state.hotkey_service.deactivate();
            *state.audio_recorder.lock().unwrap() = None;
            *state.streaming_recorder.lock().unwrap() = None;
            *state.text_inserter.lock().unwrap() = None;
            *state.post_processor.lock().unwrap() = None;
            *state.assistant_processor.lock().unwrap() = None;
            *state.qwen_client.lock().unwrap() = None;
            *state.sensevoice_client.lock().unwrap() = None;
            *state.doubao_client.lock().unwrap() = None;
            *is_running = false;
        }
    }
    app_handle.exit(0);
    Ok(())
}

/// 显示录音悬浮窗
#[tauri::command]
pub async fn show_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        overlay.show().map_err(|e| e.to_string())?;
        // 注意：不调用 set_focus()，避免抢夺用户当前窗口的焦点
    }
    Ok(())
}

/// 隐藏录音悬浮窗（带重试机制）
#[tauri::command]
pub async fn hide_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        // 第一次尝试
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败，准备重试: {}", e);
            // 延迟 50ms 重试
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            overlay.hide().map_err(|e| {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
                e.to_string()
            })?;
        }
    }
    Ok(())
}
