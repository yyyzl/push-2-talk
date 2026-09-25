#[path = "../src/atdd/lifecycle.rs"]
mod lifecycle;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::watch;

#[tokio::test]
async fn cancel_before_preparation_prevents_start() {
    let (_tx, mut rx) = watch::channel(true);
    assert!(!lifecycle::wait_delay(Duration::from_secs(5), &mut rx).await);
}

#[tokio::test]
async fn cancelled_initialization_cannot_start_recording_later() {
    let (tx, mut rx) = watch::channel(false);
    let recorded = Arc::new(AtomicBool::new(false));
    let probe = recorded.clone();
    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        probe.store(true, Ordering::SeqCst);
    });
    tx.send(true).unwrap();
    assert!(
        !lifecycle::await_startup(task, &mut rx, Duration::from_secs(1))
            .await
            .unwrap()
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!recorded.load(Ordering::SeqCst));
}

#[tokio::test]
async fn timed_out_initialization_is_aborted_and_joined() {
    let (_tx, mut rx) = watch::channel(false);
    let recorded = Arc::new(AtomicBool::new(false));
    let probe = recorded.clone();
    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        probe.store(true, Ordering::SeqCst);
    });
    assert!(
        lifecycle::await_startup(task, &mut rx, Duration::from_millis(5))
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!recorded.load(Ordering::SeqCst));
}

#[tokio::test]
async fn initialized_recording_continues_normally() {
    let (_tx, mut rx) = watch::channel(false);
    let task = tauri::async_runtime::spawn(async {});
    assert!(
        lifecycle::await_startup(task, &mut rx, Duration::from_secs(1))
            .await
            .unwrap()
    );
    assert!(lifecycle::wait_delay(Duration::from_millis(1), &mut rx).await);
}
