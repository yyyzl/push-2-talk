//! Window visibility is shared; native target restoration belongs to the platform backend.
use crate::platform::{self, InputTarget};
use tauri::{AppHandle, Manager};

pub async fn hide_overlay_and_restore_focus(app: &AppHandle, target: Option<InputTarget>) -> bool {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // Keep focus restoration inside the recording task. A detached blocking task could
    // move focus after cancellation and after the next recording has already begun.
    let restored = platform::prepare_target(platform::desktop(), target);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    match restored {
        Ok(_) => true,
        error => {
            tracing::warn!("目标焦点恢复失败: {:?}", error);
            false
        }
    }
}

#[allow(dead_code)]
pub fn verify_focus(target: Option<InputTarget>) -> bool {
    target
        .map(|target| platform::desktop().is_focused(target))
        .unwrap_or(false)
}
