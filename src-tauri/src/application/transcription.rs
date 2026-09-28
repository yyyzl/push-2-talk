//! ASR finalization and the production dictation workflow.
use super::runtime::AppState;
use crate::shell::windows::{
    emit_error_and_hide_overlay, hide_overlay_silently, hide_overlay_window,
};
use crate::{
    asr::{
        self, DoubaoASRClient, DoubaoImeRealtimeSession, DoubaoRealtimeSession, QwenASRClient,
        RealtimeSession, SenseVoiceClient,
    },
    audio_recorder::AudioRecorder,
    config,
    llm_post_processor::LlmPostProcessor,
    pipeline::NormalPipeline,
    platform::InputTarget,
    search,
    streaming_recorder::StreamingRecorder,
    text_inserter::TextInserter,
    tnl,
    usage_stats::UsageStats,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
pub(crate) const DOUBAO_IME_MISSING_FALLBACK_ERROR: &str =
    "豆包输入法实时 ASR 暂不可用，且未配置备用 ASR。请稍后重试或在 ASR 设置中配置备用服务";

pub(crate) async fn transcribe_with_available_clients(
    qwen: Option<QwenASRClient>,
    doubao: Option<DoubaoASRClient>,
    sensevoice: Option<SenseVoiceClient>,
    audio_data: &[u8],
    enable_fallback: bool,
    active_provider: Option<config::AsrProvider>,
    fallback_provider: Option<config::AsrProvider>,
    log_prefix: &str,
) -> anyhow::Result<String> {
    if enable_fallback {
        // 根据配置的 active_provider 和 fallback_provider 选择客户端组合
        match (active_provider.as_ref(), fallback_provider.as_ref()) {
            (Some(config::AsrProvider::Qwen), Some(config::AsrProvider::SiliconFlow)) => {
                if let (Some(q), Some(s)) = (&qwen, &sensevoice) {
                    tracing::info!("{}使用千问+SenseVoice并行竞速", log_prefix);
                    asr::transcribe_with_fallback_clients(q.clone(), s.clone(), audio_data.to_vec())
                        .await
                } else {
                    Err(anyhow::anyhow!("千问或 SenseVoice 客户端未初始化"))
                }
            }
            (Some(config::AsrProvider::Doubao), Some(config::AsrProvider::SiliconFlow)) => {
                if let (Some(d), Some(s)) = (&doubao, &sensevoice) {
                    tracing::info!("{}使用豆包+SenseVoice并行竞速", log_prefix);
                    asr::transcribe_doubao_sensevoice_race(
                        d.clone(),
                        s.clone(),
                        audio_data.to_vec(),
                    )
                    .await
                } else {
                    Err(anyhow::anyhow!("豆包或 SenseVoice 客户端未初始化"))
                }
            }
            _ => {
                // 其他组合或只有主客户端，使用主客户端
                match active_provider {
                    Some(config::AsrProvider::Qwen) => {
                        if let Some(q) = qwen {
                            tracing::info!("{}使用千问 ASR", log_prefix);
                            q.transcribe_bytes(audio_data).await
                        } else {
                            Err(anyhow::anyhow!("千问客户端未初始化"))
                        }
                    }
                    Some(config::AsrProvider::Doubao) => {
                        if let Some(d) = doubao {
                            tracing::info!("{}使用豆包 ASR", log_prefix);
                            d.transcribe_bytes(audio_data).await
                        } else {
                            Err(anyhow::anyhow!("豆包客户端未初始化"))
                        }
                    }
                    Some(config::AsrProvider::SiliconFlow) => {
                        if let Some(s) = sensevoice {
                            tracing::info!("{}使用 SenseVoice ASR", log_prefix);
                            s.transcribe_bytes(audio_data).await
                        } else {
                            Err(anyhow::anyhow!("SenseVoice 客户端未初始化"))
                        }
                    }
                    Some(config::AsrProvider::DoubaoIme) => {
                        // 豆包输入法目前只支持实时流式模式，不支持 HTTP 模式
                        Err(anyhow::anyhow!("豆包输入法 ASR 不支持 HTTP 模式"))
                    }
                    None => {
                        tracing::error!("{}未配置 ASR 提供商", log_prefix);
                        Err(anyhow::anyhow!("ASR 提供商未配置"))
                    }
                }
            }
        }
    } else {
        // 非 fallback 模式：只使用主客户端
        match active_provider {
            Some(config::AsrProvider::Qwen) => {
                if let Some(q) = qwen {
                    tracing::info!("{}使用千问 ASR", log_prefix);
                    q.transcribe_bytes(audio_data).await
                } else {
                    Err(anyhow::anyhow!("千问客户端未初始化"))
                }
            }
            Some(config::AsrProvider::Doubao) => {
                if let Some(d) = doubao {
                    tracing::info!("{}使用豆包 ASR", log_prefix);
                    d.transcribe_bytes(audio_data).await
                } else {
                    Err(anyhow::anyhow!("豆包客户端未初始化"))
                }
            }
            Some(config::AsrProvider::SiliconFlow) => {
                if let Some(s) = sensevoice {
                    tracing::info!("{}使用 SenseVoice ASR", log_prefix);
                    s.transcribe_bytes(audio_data).await
                } else {
                    Err(anyhow::anyhow!("SenseVoice 客户端未初始化"))
                }
            }
            Some(config::AsrProvider::DoubaoIme) => {
                // 豆包输入法目前只支持实时流式模式，不支持 HTTP 模式
                Err(anyhow::anyhow!("豆包输入法 ASR 不支持 HTTP 模式"))
            }
            None => {
                tracing::error!("{}未配置 ASR 提供商", log_prefix);
                Err(anyhow::anyhow!("ASR 提供商未配置"))
            }
        }
    }
}

pub(crate) async fn handle_http_transcription(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    target_hwnd: Option<InputTarget>, // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    // 停止录音并直接获取内存中的音频数据
    let audio_data = {
        let mut recorder_guard = recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_recording_to_memory() {
                Ok(data) => Some(data),
                Err(e) => {
                    if is_audio_skip_error(&e) {
                        tracing::info!("音频已跳过: {}", e);
                        hide_overlay_silently(&app);
                    } else {
                        emit_error_and_hide_overlay(&app, format!("停止录音失败: {}", e));
                    }
                    None
                }
            }
        } else {
            None
        }
    };

    if let Some(audio_data) = audio_data {
        let _ = app.emit("transcribing", ());

        let enable_fallback = *enable_fallback_state.lock().unwrap();
        let qwen = { qwen_client_state.lock().unwrap().clone() };
        let doubao = { doubao_client_state.lock().unwrap().clone() };
        let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
        let active_prov = app
            .state::<AppState>()
            .realtime_provider
            .lock()
            .unwrap()
            .clone();
        let fallback_prov = app
            .state::<AppState>()
            .fallback_provider
            .lock()
            .unwrap()
            .clone();

        let asr_start = std::time::Instant::now();
        let result = transcribe_with_available_clients(
            qwen,
            doubao,
            sensevoice,
            &audio_data,
            enable_fallback,
            active_prov,
            fallback_prov,
            "(HTTP) ",
        )
        .await;
        let asr_time_ms = asr_start.elapsed().as_millis() as u64;

        handle_transcription_result(
            app,
            post_processor,
            text_inserter,
            result,
            asr_time_ms,
            target_hwnd,
            usage_stats,
            recording_start_instant,
        )
        .await;
    }
}

pub(crate) async fn handle_realtime_stop(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    doubao_ime_session: Arc<tokio::sync::Mutex<Option<DoubaoImeRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    target_hwnd: Option<InputTarget>, // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    let _ = app.emit("transcribing", ());
    let asr_start = std::time::Instant::now();
    let enable_fb = *enable_fallback_state.lock().unwrap();

    // 1. 停止流式录音，获取完整音频数据（用于备用方案）
    let audio_data = {
        let mut recorder_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_streaming() {
                Ok(data) => Some(data),
                Err(e) => {
                    tracing::error!("停止流式录音失败: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    // 2. 等待音频发送任务完成
    {
        let handle = audio_sender_handle.lock().unwrap().take();
        if let Some(h) = handle {
            tracing::info!("等待音频发送任务完成...");
            super::recording_resources::join_audio_sender(h).await;
        }
    }

    // 3. 检查使用的是哪个 provider
    let provider = realtime_provider.lock().unwrap().clone();

    match provider {
        Some(config::AsrProvider::Doubao) => {
            // 处理豆包流式会话
            let mut doubao_session_guard = doubao_session.lock().await;
            if let Some(ref mut session) = *doubao_session_guard {
                tracing::info!("豆包：发送 finish 并等待转录结果...");

                // 发送 finish
                if let Err(e) = session.finish_audio().await {
                    tracing::error!("豆包发送 finish 失败: {}", e);
                    drop(doubao_session_guard);
                    // 回退到备用方案
                    if let Some(audio_data) = audio_data {
                        fallback_transcription(
                            app,
                            post_processor,
                            text_inserter,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                            enable_fb,
                            target_hwnd,
                            Arc::clone(&usage_stats),
                            Arc::clone(&recording_start_instant),
                        )
                        .await;
                    }
                    return;
                }

                // 等待转录结果
                match session.wait_for_result().await {
                    Ok(text) => {
                        let asr_time_ms = asr_start.elapsed().as_millis() as u64;
                        tracing::info!("豆包实时转录成功: {} (ASR 耗时: {}ms)", text, asr_time_ms);
                        drop(doubao_session_guard);
                        *doubao_session.lock().await = None;
                        handle_transcription_result(
                            app,
                            post_processor,
                            text_inserter,
                            Ok(text),
                            asr_time_ms,
                            target_hwnd,
                            usage_stats,
                            recording_start_instant,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!("豆包等待转录结果失败: {}，尝试备用方案", e);
                        drop(doubao_session_guard);
                        *doubao_session.lock().await = None;

                        // 回退到备用方案
                        if let Some(audio_data) = audio_data {
                            fallback_transcription(
                                app,
                                post_processor,
                                text_inserter,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                                enable_fb,
                                target_hwnd,
                                Arc::clone(&usage_stats),
                                Arc::clone(&recording_start_instant),
                            )
                            .await;
                        } else {
                            emit_error_and_hide_overlay(&app, format!("转录失败: {}", e));
                        }
                    }
                }
            } else {
                // 没有活跃的豆包会话，使用备用方案
                tracing::warn!("没有活跃的豆包 WebSocket 会话，使用备用方案");
                drop(doubao_session_guard);

                if let Some(audio_data) = audio_data {
                    fallback_transcription(
                        app,
                        post_processor,
                        text_inserter,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                        enable_fb,
                        target_hwnd,
                        Arc::clone(&usage_stats),
                        Arc::clone(&recording_start_instant),
                    )
                    .await;
                } else {
                    emit_error_and_hide_overlay(&app, "没有录制到音频数据".to_string());
                }
            }
        }
        Some(config::AsrProvider::DoubaoIme) => {
            let mut doubao_ime_session_guard = doubao_ime_session.lock().await;
            if let Some(ref mut session) = *doubao_ime_session_guard {
                tracing::info!("豆包输入法：发送 finish 并等待转录结果...");

                if let Err(e) = session.finish_audio().await {
                    tracing::error!("豆包输入法发送 finish 失败: {}", e);
                    drop(doubao_ime_session_guard);
                    if let Some(audio_data) = audio_data {
                        fallback_transcription(
                            app,
                            post_processor,
                            text_inserter,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                            enable_fb,
                            target_hwnd,
                            Arc::clone(&usage_stats),
                            Arc::clone(&recording_start_instant),
                        )
                        .await;
                    }
                    return;
                }

                match session.wait_for_result().await {
                    Ok(text) => {
                        let asr_time_ms = asr_start.elapsed().as_millis() as u64;
                        tracing::info!(
                            "豆包输入法实时转录成功: {} (ASR 耗时: {}ms)",
                            text,
                            asr_time_ms
                        );
                        drop(doubao_ime_session_guard);
                        *doubao_ime_session.lock().await = None;
                        handle_transcription_result(
                            app,
                            post_processor,
                            text_inserter,
                            Ok(text),
                            asr_time_ms,
                            target_hwnd,
                            usage_stats,
                            recording_start_instant,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!("豆包输入法等待转录结果失败: {}，尝试备用方案", e);
                        drop(doubao_ime_session_guard);
                        *doubao_ime_session.lock().await = None;

                        if let Some(audio_data) = audio_data {
                            fallback_transcription(
                                app,
                                post_processor,
                                text_inserter,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                                enable_fb,
                                target_hwnd,
                                Arc::clone(&usage_stats),
                                Arc::clone(&recording_start_instant),
                            )
                            .await;
                        } else {
                            emit_error_and_hide_overlay(&app, format!("转录失败: {}", e));
                        }
                    }
                }
            } else {
                tracing::warn!("没有活跃的豆包输入法 WebSocket 会话，使用备用方案");
                drop(doubao_ime_session_guard);

                if let Some(audio_data) = audio_data {
                    fallback_transcription(
                        app,
                        post_processor,
                        text_inserter,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                        enable_fb,
                        target_hwnd,
                        Arc::clone(&usage_stats),
                        Arc::clone(&recording_start_instant),
                    )
                    .await;
                } else {
                    emit_error_and_hide_overlay(&app, "没有录制到音频数据".to_string());
                }
            }
        }
        _ => {
            // 处理千问流式会话
            let mut session_guard = active_session.lock().await;
            if let Some(ref mut session) = *session_guard {
                tracing::info!("千问：发送 commit 并等待转录结果...");

                // 发送 commit
                if let Err(e) = session.commit_audio().await {
                    tracing::error!("千问发送 commit 失败: {}", e);
                    drop(session_guard);
                    // 回退到备用方案
                    if let Some(audio_data) = audio_data {
                        fallback_transcription(
                            app,
                            post_processor,
                            text_inserter,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                            enable_fb,
                            target_hwnd,
                            Arc::clone(&usage_stats),
                            Arc::clone(&recording_start_instant),
                        )
                        .await;
                    }
                    return;
                }

                // 等待转录结果
                match session.wait_for_result().await {
                    Ok(text) => {
                        let asr_time_ms = asr_start.elapsed().as_millis() as u64;
                        tracing::info!("千问实时转录成功: {} (ASR 耗时: {}ms)", text, asr_time_ms);
                        let _ = session.close().await;
                        drop(session_guard);
                        *active_session.lock().await = None;
                        handle_transcription_result(
                            app,
                            post_processor,
                            text_inserter,
                            Ok(text),
                            asr_time_ms,
                            target_hwnd,
                            usage_stats,
                            recording_start_instant,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!("千问等待转录结果失败: {}，尝试备用方案", e);
                        let _ = session.close().await;
                        drop(session_guard);
                        *active_session.lock().await = None;

                        // 回退到备用方案
                        if let Some(audio_data) = audio_data {
                            fallback_transcription(
                                app,
                                post_processor,
                                text_inserter,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                                enable_fb,
                                target_hwnd,
                                Arc::clone(&usage_stats),
                                Arc::clone(&recording_start_instant),
                            )
                            .await;
                        } else {
                            emit_error_and_hide_overlay(&app, format!("转录失败: {}", e));
                        }
                    }
                }
            } else {
                // 没有活跃会话，使用备用方案（可能是连接失败时的回退）
                tracing::warn!("没有活跃的千问 WebSocket 会话，使用备用方案");
                drop(session_guard);

                if let Some(audio_data) = audio_data {
                    fallback_transcription(
                        app,
                        post_processor,
                        text_inserter,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                        enable_fb,
                        target_hwnd,
                        Arc::clone(&usage_stats),
                        Arc::clone(&recording_start_instant),
                    )
                    .await;
                } else {
                    emit_error_and_hide_overlay(&app, "没有录制到音频数据".to_string());
                }
            }
        }
    }
}

pub(crate) async fn fallback_transcription(
    app: AppHandle,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    audio_data: Vec<u8>,
    enable_fallback: bool,
    target_hwnd: Option<InputTarget>, // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    let qwen = { qwen_client_state.lock().unwrap().clone() };
    let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
    let doubao = { doubao_client_state.lock().unwrap().clone() };
    let active_prov = app
        .state::<AppState>()
        .realtime_provider
        .lock()
        .unwrap()
        .clone();
    let fallback_prov = app
        .state::<AppState>()
        .fallback_provider
        .lock()
        .unwrap()
        .clone();

    let asr_start = std::time::Instant::now();
    let result = match http_fallback_provider(active_prov, fallback_prov.clone(), enable_fallback) {
        Ok(effective_active_prov) => {
            transcribe_with_available_clients(
                qwen,
                doubao,
                sensevoice,
                &audio_data,
                enable_fallback,
                effective_active_prov,
                fallback_prov,
                "(备用) ",
            )
            .await
        }
        Err(error) => Err(error),
    };
    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    handle_transcription_result(
        app,
        post_processor,
        text_inserter,
        result,
        asr_time_ms,
        target_hwnd,
        usage_stats,
        recording_start_instant,
    )
    .await;
}

pub(crate) fn is_audio_skip_error(error: &anyhow::Error) -> bool {
    let msg = error.to_string();
    msg.contains("录音过短或无声音") || msg.contains("音频数据为空")
}

pub(crate) async fn handle_transcription_result(
    app: AppHandle,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    result: anyhow::Result<String>,
    asr_time_ms: u64,
    target_hwnd: Option<InputTarget>, // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    // 从锁中提取处理器（clone 后立即释放锁）
    let post_proc = { post_processor.lock().unwrap().clone() };

    // 从 state 获取最新词库与词库增强开关（避免 pipeline 内持锁）
    let state = app.state::<AppState>();
    let dictionary = { state.dictionary.lock().unwrap().clone() };
    let enable_post_process = { *state.enable_post_process.lock().unwrap() };
    let enable_dictionary_enhancement = { *state.enable_dictionary_enhancement.lock().unwrap() };

    let settings = state.recording.settings.lock().unwrap().clone();
    let Some(settings) = settings else {
        emit_error_and_hide_overlay(&app, "录音配置快照缺失".into());
        return;
    };

    // 听写模式：只使用 NormalPipeline
    let pipeline = NormalPipeline::new();
    let mut inserter = { text_inserter.lock().unwrap().clone() };
    let pipeline_result = pipeline
        .process(
            &app,
            post_proc,
            enable_post_process,
            dictionary,
            enable_dictionary_enhancement,
            &mut inserter,
            result,
            asr_time_ms,
            &settings,
            target_hwnd,
        )
        .await;

    // 处理管道结果
    match pipeline_result {
        Ok(result) => {
            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

            // 更新统计数据（后端全权负责）
            if let Some(start_time) = recording_start_instant.lock().unwrap().take() {
                let recording_ms = start_time.elapsed().as_millis() as u64;
                // 统计非空白字符数（与前端旧逻辑保持一致）
                let recognized_chars =
                    result.text.chars().filter(|c| !c.is_whitespace()).count() as u64;

                let mut stats = usage_stats.lock().unwrap();
                if let Err(e) = stats.update_and_save(recording_ms, recognized_chars) {
                    tracing::error!("更新统计数据失败: {}", e);
                }
            }

            // 构建兼容的 TranscriptionResult
            let transcription_result = TranscriptionResult {
                text: result.text,
                original_text: result.original_text,
                selected_text: result.selected_text,
                asr_time_ms: result.asr_time_ms,
                llm_time_ms: result.llm_time_ms,
                total_time_ms: result.total_time_ms,
                mode: Some(format!("{:?}", result.mode).to_lowercase()),
                inserted: Some(result.inserted),
                tnl_diagnostics: result.tnl_diagnostics,
                citations: None,
                tool_calls_summary: None,
                web_searched: false,
                search_failed: false,
            };

            // 发送完成事件
            let _ = app.emit("transcription_complete", transcription_result);
        }
        Err(e) => {
            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

            // 清理录音开始时间（防止下次录音时使用错误的时间）
            let _ = recording_start_instant.lock().unwrap().take();

            // 发送错误事件
            tracing::error!("转录处理失败: {}", e);
            let _ = app.emit("error", format!("转录失败: {}", e));
        }
    }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct TranscriptionResult {
    pub text: String,
    pub original_text: Option<String>, // 原始 ASR 文本（仅开启 LLM 润色时有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>, // 用户选中的引用文本（仅 AI 助手模式有值）
    pub asr_time_ms: u64,
    pub llm_time_ms: Option<u64>,
    pub total_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>, // 新增：处理模式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inserted: Option<bool>, // 新增：是否已自动插入
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tnl_diagnostics: Option<tnl::TnlDiagnostics>, // 可选：TNL 候选/替换诊断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<search::SearchResultItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls_summary: Option<Vec<search::ToolCallSummary>>,
    #[serde(default)]
    pub web_searched: bool,
    #[serde(default)]
    pub search_failed: bool,
}

pub(crate) fn http_fallback_provider(
    active: Option<config::AsrProvider>,
    fallback: Option<config::AsrProvider>,
    enabled: bool,
) -> anyhow::Result<Option<config::AsrProvider>> {
    match active {
        Some(config::AsrProvider::DoubaoIme) => fallback
            .filter(|_| enabled)
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!(DOUBAO_IME_MISSING_FALLBACK_ERROR)),
        other => Ok(other),
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    #[test]
    fn ime_without_fallback_reports_the_actionable_error() {
        let error =
            http_fallback_provider(Some(config::AsrProvider::DoubaoIme), None, true).unwrap_err();
        assert_eq!(error.to_string(), DOUBAO_IME_MISSING_FALLBACK_ERROR);
    }
    #[test]
    fn ime_uses_the_selected_fallback_but_other_providers_keep_their_primary() {
        assert_eq!(
            http_fallback_provider(
                Some(config::AsrProvider::DoubaoIme),
                Some(config::AsrProvider::Qwen),
                true,
            )
            .unwrap(),
            Some(config::AsrProvider::Qwen)
        );
        assert_eq!(
            http_fallback_provider(
                Some(config::AsrProvider::Doubao),
                Some(config::AsrProvider::Qwen),
                true,
            )
            .unwrap(),
            Some(config::AsrProvider::Doubao)
        );
    }
    #[test]
    fn disabled_fallback_must_not_send_audio_to_a_saved_provider() {
        assert!(http_fallback_provider(
            Some(config::AsrProvider::DoubaoIme),
            Some(config::AsrProvider::Qwen),
            false,
        )
        .is_err());
    }
    #[tokio::test]
    async fn an_unconfigured_provider_keeps_the_original_error() {
        let error = transcribe_with_available_clients(None, None, None, &[], false, None, None, "")
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "ASR 提供商未配置");
    }
}
