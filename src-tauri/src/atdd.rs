//! Opt-in local acceptance driver. Uses the real recording callbacks and all permission checks.
//! Does not claim to emulate physical keyboard hardware. No network listener or arbitrary input.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

static RUNNING: AtomicBool = AtomicBool::new(false);
struct RunGuard;
impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub(crate) async fn run(app: tauri::AppHandle) -> Result<String, String> {
    RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "已有验收录音在执行".to_string())?;
    let _guard = RunGuard;
    let service = app.state::<crate::AppState>().hotkey_service.clone();
    if !service.is_service_active() || !crate::platform::desktop().status().ready() {
        return Err("需要正常启动服务并授予全部权限".into());
    }
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let fixture = std::env::temp_dir().join(format!("PushToTalk-ATDD-{timestamp}.txt"));
    std::fs::write(&fixture, "PushToTalk ATDD\n\n").map_err(|error| error.to_string())?;
    let setup_path = fixture.clone();
    tokio::task::spawn_blocking(move || crate::platform::atdd_prepare_fixture(&setup_path))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    service
        .atdd_recording(true)
        .map_err(|error| error.to_string())?;
    let target = *app.state::<crate::AppState>().target_window.lock().unwrap();
    let description = crate::platform::atdd_target_description(target);
    tokio::time::sleep(std::time::Duration::from_secs(18)).await;
    service
        .atdd_recording(false)
        .map_err(|error| error.to_string())?;
    // Diagnostic observation only; the real asynchronous pipeline owns ASR and insertion.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let after = crate::platform::atdd_target_description(target);
    Ok(format!(
        "录音已结束；开始目标：{description}。结束后诊断：{after}。验收文档：{}",
        fixture.display()
    ))
}
