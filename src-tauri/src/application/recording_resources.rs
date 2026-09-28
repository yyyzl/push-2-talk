//! Native recording resources shared with provider adapters, released by RecordingSession.
use crate::{
    asr::{DoubaoImeRealtimeSession, DoubaoRealtimeSession, RealtimeSession},
    audio_recorder::AudioRecorder,
    config::TriggerMode,
    platform::{AudioMuteManager, InputTarget},
    streaming_recorder::StreamingRecorder,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Default)]
pub(crate) struct RecordingResources {
    pub settings: Arc<Mutex<Option<crate::config::AppConfig>>>,
    pub audio_recorder: Arc<Mutex<Option<AudioRecorder>>>,
    pub streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    pub active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    pub doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    pub doubao_ime_session: Arc<tokio::sync::Mutex<Option<DoubaoImeRealtimeSession>>>,
    pub audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub audio_mute_manager: Arc<Mutex<Option<AudioMuteManager>>>,
    pub current_trigger_mode: Arc<Mutex<Option<TriggerMode>>>,
    pub is_recording_locked: Arc<AtomicBool>,
    pub target_window: Arc<Mutex<Option<InputTarget>>>,
    pub recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
    mute_session_active: Arc<AtomicBool>,
}

impl RecordingResources {
    pub fn begin_audio(&self) {
        if let Some(manager) = self.audio_mute_manager.lock().unwrap().as_ref() {
            self.mute_session_active.store(true, Ordering::SeqCst);
            manager.begin_session();
            if let Err(error) = manager.mute_other_apps() {
                tracing::debug!("静音其他应用: {error}");
            }
        }
    }

    pub fn restore_audio(&self) {
        if self.mute_session_active.swap(false, Ordering::SeqCst) {
            if let Some(manager) = self.audio_mute_manager.lock().unwrap().as_ref() {
                manager.end_session();
                if let Err(error) = manager.restore_volumes() {
                    tracing::warn!("恢复音量失败: {error}");
                }
            }
        }
    }

    pub fn is_recording(&self, realtime: bool) -> bool {
        if realtime {
            self.streaming_recorder
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|r| r.is_recording())
        } else {
            self.audio_recorder
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|r| r.is_recording())
        }
    }

    pub async fn cleanup(&self) {
        // Stop producers first, including partially initialized microphone streams.
        if let Some(recorder) = self.streaming_recorder.lock().unwrap().as_mut() {
            recorder.discard();
        }
        if let Some(recorder) = self.audio_recorder.lock().unwrap().as_mut() {
            recorder.discard();
        }
        self.restore_audio();
        self.settings.lock().unwrap().take();
        let sender = self.audio_sender_handle.lock().unwrap().take();
        if let Some(sender) = sender {
            sender.abort();
            let _ = tokio::time::timeout(Duration::from_secs(2), sender).await;
        }
        // Remove each socket from shared state before awaiting bounded network shutdown.
        let qwen = self.active_session.lock().await.take();
        let doubao = self.doubao_session.lock().await.take();
        let ime = self.doubao_ime_session.lock().await.take();
        tokio::join!(
            async {
                if let Some(session) = qwen {
                    let _ = tokio::time::timeout(Duration::from_secs(2), session.close()).await;
                }
            },
            async {
                if let Some(mut session) = doubao {
                    let _ =
                        tokio::time::timeout(Duration::from_secs(2), session.finish_audio()).await;
                }
            },
            async {
                if let Some(mut session) = ime {
                    let _ =
                        tokio::time::timeout(Duration::from_secs(2), session.finish_audio()).await;
                }
            },
        );
        self.is_recording_locked.store(false, Ordering::SeqCst);
        self.current_trigger_mode.lock().unwrap().take();
        self.recording_start_instant.lock().unwrap().take();
    }
}

pub(crate) fn join_audio_sender(
    task: tokio::task::JoinHandle<()>,
) -> impl std::future::Future<Output = ()> {
    // Construct the guard before polling, so even a never-polled drain owns cancellation.
    struct AbortOnDrop(tokio::task::AbortHandle);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let abort = AbortOnDrop(task.abort_handle());
    async move {
        let _guard = abort;
        let _ = task.await;
    }
}

#[cfg(test)]
mod sender_tests {
    #[tokio::test]
    async fn cancelling_a_drain_also_cancels_the_sender_it_owns() {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let (late, result) = tokio::sync::oneshot::channel();
        let sender = tokio::spawn(async {
            started.send(()).unwrap();
            let _ = wait.await;
            let _ = late.send(());
        });
        let drain = tokio::spawn(super::join_audio_sender(sender));
        ready.await.unwrap();
        drain.abort();
        let _ = drain.await;
        let _ = release.send(());
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(2), result)
                .await
                .unwrap()
                .is_err()
        );
    }
}
