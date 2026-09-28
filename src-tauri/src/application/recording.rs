//! One owner for startup, processing and cleanup. Native resources stay in the adapter.
use futures_util::{future::BoxFuture, FutureExt};
use std::{
    future::Future,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
};
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct RecordingSession {
    active: Mutex<Option<Active>>,
}

struct Active {
    cancel: CancellationToken,
    done: CancellationToken,
    #[allow(dead_code)] // readiness observation for the native acceptance harness
    startup: watch::Receiver<Option<Result<(), String>>>,
    finish: Option<oneshot::Sender<BoxFuture<'static, ()>>>,
}

#[derive(Clone)]
pub(crate) struct Completion(CancellationToken);
impl Completion {
    pub async fn wait(self) {
        self.0.cancelled().await;
    }
}

impl RecordingSession {
    #[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
    pub fn is_active(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }

    /// Prepare is called only after exclusive ownership is acquired. It may capture the input target.
    pub fn start<F, C>(self: &Arc<Self>, prepare: impl FnOnce() -> (F, C)) -> bool
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        C: Future<Output = ()> + Send + 'static,
    {
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return false;
        }
        let (startup, cleanup) = prepare();
        let cancel = CancellationToken::new();
        let done = CancellationToken::new();
        let (finish, work) = oneshot::channel::<BoxFuture<'static, ()>>();
        let (ready, status) = watch::channel(None);
        *active = Some(Active {
            cancel: cancel.clone(),
            done: done.clone(),
            startup: status,
            finish: Some(finish),
        });
        let owner = self.clone();
        tauri::async_runtime::spawn(async move {
            let run = async {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {},
                    result = async {
                        let result = startup.await;
                        ready.send_replace(Some(result.clone()));
                        result?;
                        if let Ok(work) = work.await { work.await; }
                        Ok::<(), String>(())
                    } => {
                        if let Err(error) = result { tracing::warn!("录音会话启动失败: {error}"); }
                    }
                }
            };
            if AssertUnwindSafe(run).catch_unwind().await.is_err() {
                tracing::error!("录音会话异常，执行资源收尾");
            }
            // startup/processing futures have been dropped; no late work can write into a new session.
            ready.send_if_modified(|status| {
                if status.is_none() {
                    *status = Some(Err("录音启动已取消".into()));
                    true
                } else {
                    false
                }
            });
            if AssertUnwindSafe(cleanup).catch_unwind().await.is_err() {
                tracing::error!("录音资源收尾异常");
            }
            // Keep the slot occupied through cleanup. A later session cannot share these resources.
            *owner.active.lock().unwrap() = None;
            done.cancel();
        });
        true
    }

    /// At most one stop is accepted. Its work runs after startup, under the same cancellation scope.
    pub fn finish(&self, work: impl Future<Output = ()> + Send + 'static) -> Option<Completion> {
        let mut active = self.active.lock().unwrap();
        let active = active.as_mut()?;
        if active.cancel.is_cancelled() {
            return None;
        }
        active.finish.take()?.send(work.boxed()).ok()?;
        Some(Completion(active.done.clone()))
    }

    pub async fn cancel(&self) {
        let done = {
            let active = self.active.lock().unwrap();
            active.as_ref().map(|active| {
                active.cancel.cancel();
                active.done.clone()
            })
        };
        if let Some(done) = done {
            done.cancelled().await;
        }
    }

    #[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
    pub async fn wait_started(&self) -> Result<(), String> {
        let mut status = self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("没有录音会话")?
            .startup
            .clone();
        loop {
            if let Some(result) = status.borrow().clone() {
                return result;
            }
            status.changed().await.map_err(|_| "录音会话已结束")?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn new_start_is_rejected_until_previous_cleanup_has_finished() {
        let runtime = Arc::new(RecordingSession::default());
        let (entered, cleaning) = oneshot::channel();
        let (release, wait) = oneshot::channel();
        assert!(runtime.start(|| (async { Ok(()) }, async {
            entered.send(()).unwrap();
            wait.await.unwrap();
        })));
        let done = runtime.finish(async {}).unwrap();
        cleaning.await.unwrap();
        assert!(!runtime.start(|| (async { Ok(()) }, async {})));
        release.send(()).unwrap();
        done.wait().await;
        assert!(runtime.start(|| (async { Ok(()) }, async {})));
        runtime.cancel().await;
    }

    #[tokio::test]
    async fn immediate_stop_waits_for_startup_and_duplicate_stop_is_ignored() {
        let runtime = Arc::new(RecordingSession::default());
        let log = Arc::new(Mutex::new(Vec::new()));
        let (ready, wait) = oneshot::channel();
        let started = log.clone();
        let cleaned = log.clone();
        assert!(runtime.start(|| (
            async move {
                wait.await.unwrap();
                started.lock().unwrap().push("start");
                Ok(())
            },
            async move {
                cleaned.lock().unwrap().push("cleanup");
            }
        )));
        let finished = log.clone();
        let done = runtime
            .finish(async move {
                finished.lock().unwrap().push("stop");
            })
            .unwrap();
        assert!(runtime.finish(async { panic!("duplicate") }).is_none());
        assert!(log.lock().unwrap().is_empty());
        ready.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), done.wait())
            .await
            .unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["start", "stop", "cleanup"]);
    }

    #[tokio::test]
    async fn cancellation_joins_startup_before_cleanup_and_allows_next_session() {
        let runtime = Arc::new(RecordingSession::default());
        let late = Arc::new(AtomicUsize::new(0));
        let cleaned = Arc::new(AtomicUsize::new(0));
        let (ready, wait) = oneshot::channel::<()>();
        let late_task = late.clone();
        let cleanup_count = cleaned.clone();
        assert!(runtime.start(|| (
            async move {
                let _ = wait.await;
                late_task.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            async move {
                cleanup_count.fetch_add(1, Ordering::SeqCst);
            }
        )));
        runtime.cancel().await;
        let _ = ready.send(());
        assert_eq!(late.load(Ordering::SeqCst), 0);
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
        assert!(runtime.start(|| (async { Ok(()) }, async {})));
        runtime.cancel().await;
        runtime.cancel().await;
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn busy_start_does_not_capture_or_overwrite_the_current_target() {
        let runtime = Arc::new(RecordingSession::default());
        assert!(runtime.start(|| (std::future::pending::<Result<(), String>>(), async {})));
        assert!(!runtime.start(|| -> (std::future::Ready<Result<(), String>>, std::future::Ready<()>) { panic!("must not prepare another target"); }));
        runtime.cancel().await;
    }

    #[tokio::test]
    async fn cancelling_processing_drops_late_result_before_the_next_session() {
        let runtime = Arc::new(RecordingSession::default());
        let inserted = Arc::new(AtomicUsize::new(0));
        assert!(runtime.start(|| (async { Ok(()) }, async {})));
        let (entered, waiting) = oneshot::channel();
        let (response, wait) = oneshot::channel::<()>();
        let insertion = inserted.clone();
        runtime
            .finish(async move {
                entered.send(()).unwrap();
                let _ = wait.await;
                insertion.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        waiting.await.unwrap();
        runtime.cancel().await;
        let _ = response.send(());
        assert!(runtime.start(|| (async { Ok(()) }, async {})));
        runtime.finish(async {}).unwrap().wait().await;
        assert_eq!(inserted.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failed_startup_skips_processing_but_still_releases_resources() {
        let runtime = Arc::new(RecordingSession::default());
        let cleaned = Arc::new(AtomicUsize::new(0));
        let cleanup_count = cleaned.clone();
        let (fail, wait) = oneshot::channel();
        assert!(runtime.start(|| (
            async {
                wait.await.unwrap();
                Err("microphone unavailable".into())
            },
            async move {
                cleanup_count.fetch_add(1, Ordering::SeqCst);
            }
        )));
        let done = runtime
            .finish(async { panic!("must not transcribe failed startup") })
            .unwrap();
        fail.send(()).unwrap();
        done.wait().await;
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
        assert!(runtime.start(|| (async { Ok(()) }, async {})));
        runtime.cancel().await;
    }
}
