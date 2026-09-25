use std::time::Duration;
use tokio::sync::watch;

async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    if *cancel.borrow() {
        return;
    }
    while cancel.changed().await.is_ok() {
        if *cancel.borrow() {
            return;
        }
    }
}

pub async fn wait_delay(duration: Duration, cancel: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        biased;
        _ = cancelled(cancel) => false,
        _ = tokio::time::sleep(duration) => true,
    }
}

pub async fn await_startup(
    mut task: tauri::async_runtime::JoinHandle<()>,
    cancel: &mut watch::Receiver<bool>,
    timeout: Duration,
) -> Result<bool, String> {
    let result = tokio::select! {
        biased;
        _ = cancelled(cancel) => Ok(false),
        result = tokio::time::timeout(timeout, &mut task) => match result {
            Ok(result) => return result.map(|_| true).map_err(|e| e.to_string()),
            Err(_) => Err("录音初始化超时，已取消本轮验收".into()),
        },
    };
    // Join the aborted initializer before cleaning up: it cannot open a microphone later.
    task.abort();
    let _ = task.await;
    result
}
