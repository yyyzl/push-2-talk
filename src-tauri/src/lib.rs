// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_recorder;
mod beep_player;
mod config;
mod hotkey_service;
mod qwen_asr;
mod qwen_realtime;
mod streaming_recorder;
mod text_inserter;

use audio_recorder::AudioRecorder;
use config::AppConfig;
use hotkey_service::HotkeyService;
use qwen_asr::QwenASRClient;
use qwen_realtime::QwenRealtimeClient;
use streaming_recorder::StreamingRecorder;
use text_inserter::TextInserter;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

// 全局应用状态
struct AppState {
    audio_recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    is_running: Arc<Mutex<bool>>,
    use_realtime_asr: Arc<Mutex<bool>>,
    // 活跃的实时转录会话（用于真正的流式传输）
    active_session: Arc<tokio::sync::Mutex<Option<qwen_realtime::RealtimeSession>>>,
    // 音频发送任务句柄
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

// Tauri Commands

#[tauri::command]
async fn save_config(api_key: String, fallback_api_key: String, use_realtime: Option<bool>) -> Result<String, String> {
    tracing::info!("保存配置...");
    let config = AppConfig {
        dashscope_api_key: api_key,
        siliconflow_api_key: fallback_api_key,
        use_realtime_asr: use_realtime.unwrap_or(true),
    };

    config
        .save()
        .map_err(|e| format!("保存配置失败: {}", e))?;

    Ok("配置已保存".to_string())
}

#[tauri::command]
async fn load_config() -> Result<AppConfig, String> {
    tracing::info!("加载配置...");
    AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))
}

#[tauri::command]
async fn start_app(
    app_handle: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
) -> Result<String, String> {
    tracing::info!("启动应用...");

    // 获取应用状态
    let state = app_handle.state::<AppState>();

    let mut is_running = state.is_running.lock().unwrap();
    if *is_running {
        return Err("应用已在运行中".to_string());
    }

    // 确定是否使用实时模式
    let use_realtime_mode = use_realtime.unwrap_or(true);
    *state.use_realtime_asr.lock().unwrap() = use_realtime_mode;

    tracing::info!("ASR 模式: {}", if use_realtime_mode { "实时 WebSocket" } else { "HTTP" });

    // 初始化文本插入器
    let text_inserter = TextInserter::new()
        .map_err(|e| format!("初始化文本插入器失败: {}", e))?;
    *state.text_inserter.lock().unwrap() = Some(text_inserter);

    // 根据模式初始化录音器
    if use_realtime_mode {
        let streaming_recorder = StreamingRecorder::new()
            .map_err(|e| format!("初始化流式录音器失败: {}", e))?;
        *state.streaming_recorder.lock().unwrap() = Some(streaming_recorder);
    } else {
        let audio_recorder = AudioRecorder::new()
            .map_err(|e| format!("初始化音频录制器失败: {}", e))?;
        *state.audio_recorder.lock().unwrap() = Some(audio_recorder);
    }

    // 启动全局快捷键监听
    let hotkey_service = HotkeyService::new();

    // 克隆状态用于回调
    let app_handle_start = app_handle.clone();
    let audio_recorder_start = Arc::clone(&state.audio_recorder);
    let streaming_recorder_start = Arc::clone(&state.streaming_recorder);
    let active_session_start = Arc::clone(&state.active_session);
    let audio_sender_handle_start = Arc::clone(&state.audio_sender_handle);
    let use_realtime_start = use_realtime_mode;
    let api_key_start = api_key.clone();

    let app_handle_stop = app_handle.clone();
    let audio_recorder_stop = Arc::clone(&state.audio_recorder);
    let streaming_recorder_stop = Arc::clone(&state.streaming_recorder);
    let active_session_stop = Arc::clone(&state.active_session);
    let audio_sender_handle_stop = Arc::clone(&state.audio_sender_handle);
    let text_inserter_stop = Arc::clone(&state.text_inserter);
    let api_key_clone = api_key.clone();
    let fallback_api_key_clone = fallback_api_key.clone();
    let use_realtime_stop = use_realtime_mode;

    // 按键按下回调
    let on_start = move || {
        let app = app_handle_start.clone();
        let recorder = Arc::clone(&audio_recorder_start);
        let streaming_recorder = Arc::clone(&streaming_recorder_start);
        let active_session = Arc::clone(&active_session_start);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_start);
        let use_realtime = use_realtime_start;
        let api_key = api_key_start.clone();

        // 播放开始录音提示音
        beep_player::play_start_beep();

        tauri::async_runtime::spawn(async move {
            tracing::info!("检测到快捷键按下");
            let _ = app.emit("recording_started", ());

            if use_realtime {
                // 实时模式：建立 WebSocket 连接 + 启动流式录音 + 启动发送任务
                tracing::info!("启动真正的实时流式转录...");

                // 1. 建立 WebSocket 连接
                let realtime_client = QwenRealtimeClient::new(api_key);
                match realtime_client.start_session().await {
                    Ok(session) => {
                        tracing::info!("WebSocket 连接已建立");

                        // 2. 启动流式录音
                        let chunk_rx = {
                            let mut streaming_guard = streaming_recorder.lock().unwrap();
                            if let Some(ref mut rec) = *streaming_guard {
                                match rec.start_streaming() {
                                    Ok(rx) => Some(rx),
                                    Err(e) => {
                                        tracing::error!("开始流式录音失败: {}", e);
                                        let _ = app.emit("error", format!("录音失败: {}", e));
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        };

                        if let Some(chunk_rx) = chunk_rx {
                            // 保存会话
                            *active_session.lock().await = Some(session);

                            // 3. 启动音频发送任务
                            let session_for_sender = Arc::clone(&active_session);
                            let sender_handle = tokio::spawn(async move {
                                tracing::info!("音频发送任务启动");
                                let mut chunk_count = 0;

                                while let Ok(chunk) = chunk_rx.recv() {
                                    let session_guard = session_for_sender.lock().await;
                                    if let Some(ref session) = *session_guard {
                                        if let Err(e) = session.send_audio_chunk(&chunk).await {
                                            tracing::error!("发送音频块失败: {}", e);
                                            break;
                                        }
                                        chunk_count += 1;
                                        if chunk_count % 10 == 0 {
                                            tracing::debug!("已发送 {} 个音频块", chunk_count);
                                        }
                                    } else {
                                        break;
                                    }
                                    drop(session_guard);
                                }

                                tracing::info!("音频发送任务结束，共发送 {} 个块", chunk_count);
                            });

                            *audio_sender_handle.lock().unwrap() = Some(sender_handle);
                        }
                    }
                    Err(e) => {
                        tracing::error!("建立 WebSocket 连接失败: {}，回退到普通录音", e);
                        let _ = app.emit("error", format!("实时连接失败: {}", e));

                        // 回退到普通流式录音（录完再传）
                        let mut streaming_guard = streaming_recorder.lock().unwrap();
                        if let Some(ref mut rec) = *streaming_guard {
                            if let Err(e) = rec.start_streaming() {
                                tracing::error!("开始流式录音失败: {}", e);
                            }
                        }
                    }
                }
            } else {
                // HTTP 模式：使用原有录音器
                let mut recorder_guard = recorder.lock().unwrap();
                if let Some(ref mut rec) = *recorder_guard {
                    if let Err(e) = rec.start_recording() {
                        tracing::error!("开始录音失败: {}", e);
                        let _ = app.emit("error", format!("录音失败: {}", e));
                    }
                }
            }
        });
    };

    // 按键释放回调
    let on_stop = move || {
        let app = app_handle_stop.clone();
        let recorder = Arc::clone(&audio_recorder_stop);
        let streaming_recorder = Arc::clone(&streaming_recorder_stop);
        let active_session = Arc::clone(&active_session_stop);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_stop);
        let inserter = Arc::clone(&text_inserter_stop);
        let key = api_key_clone.clone();
        let fallback_key = fallback_api_key_clone.clone();
        let use_realtime = use_realtime_stop;

        // 播放停止录音提示音
        beep_player::play_stop_beep();

        tauri::async_runtime::spawn(async move {
            tracing::info!("检测到快捷键释放");
            let _ = app.emit("recording_stopped", ());

            if use_realtime {
                // 实时模式：停止录音 + commit + 等待结果
                handle_realtime_stop(
                    app,
                    streaming_recorder,
                    active_session,
                    audio_sender_handle,
                    inserter,
                    key,
                    fallback_key,
                ).await;
            } else {
                // HTTP 模式：使用原有逻辑
                handle_http_transcription(
                    app,
                    recorder,
                    inserter,
                    key,
                    fallback_key,
                ).await;
            }
        });
    };

    hotkey_service
        .start(on_start, on_stop)
        .map_err(|e| format!("启动快捷键监听失败: {}", e))?;

    *is_running = true;
    let mode_str = if use_realtime_mode { "实时模式" } else { "HTTP 模式" };
    Ok(format!("应用已启动 ({})，按 Ctrl+Win 开始录音", mode_str))
}

/// HTTP 模式转录处理（原有逻辑）
async fn handle_http_transcription(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    key: String,
    fallback_key: String,
) {
    // 停止录音并直接获取内存中的音频数据
    let audio_data = {
        let mut recorder_guard = recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_recording_to_memory() {
                Ok(data) => Some(data),
                Err(e) => {
                    tracing::error!("停止录音失败: {}", e);
                    let _ = app.emit("error", format!("停止录音失败: {}", e));
                    None
                }
            }
        } else {
            None
        }
    };

    if let Some(audio_data) = audio_data {
        let _ = app.emit("transcribing", ());

        let result = if !fallback_key.is_empty() {
            tracing::info!("使用主备并行转录模式 (HTTP)");
            qwen_asr::transcribe_with_fallback_bytes(key, fallback_key, audio_data).await
        } else {
            tracing::info!("仅使用千问 ASR (HTTP)");
            let asr_client = QwenASRClient::new(key);
            asr_client.transcribe_bytes(&audio_data).await
        };

        handle_transcription_result(app, inserter, result).await;
    }
}

/// 真正的实时模式停止处理（边录边传后的 commit + 等待结果）
async fn handle_realtime_stop(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<qwen_realtime::RealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    key: String,
    fallback_key: String,
) {
    let _ = app.emit("transcribing", ());

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
            let _ = h.await;
        }
    }

    // 3. 检查是否有活跃的 WebSocket 会话
    let mut session_guard = active_session.lock().await;
    if let Some(ref mut session) = *session_guard {
        tracing::info!("发送 commit 并等待转录结果...");

        // 发送 commit
        if let Err(e) = session.commit_audio().await {
            tracing::error!("发送 commit 失败: {}", e);
            drop(session_guard);
            // 回退到备用方案
            if let Some(audio_data) = audio_data {
                fallback_transcription(app, inserter, key, fallback_key, audio_data).await;
            }
            return;
        }

        // 等待转录结果
        match session.wait_for_result().await {
            Ok(text) => {
                tracing::info!("实时转录成功: {}", text);
                let _ = session.close().await;
                drop(session_guard);
                *active_session.lock().await = None;
                handle_transcription_result(app, inserter, Ok(text)).await;
            }
            Err(e) => {
                tracing::warn!("等待转录结果失败: {}，尝试备用方案", e);
                let _ = session.close().await;
                drop(session_guard);
                *active_session.lock().await = None;

                // 回退到备用方案
                if let Some(audio_data) = audio_data {
                    fallback_transcription(app, inserter, key, fallback_key, audio_data).await;
                } else {
                    let _ = app.emit("error", format!("转录失败: {}", e));
                }
            }
        }
    } else {
        // 没有活跃会话，使用备用方案（可能是连接失败时的回退）
        tracing::warn!("没有活跃的 WebSocket 会话，使用备用方案");
        drop(session_guard);

        if let Some(audio_data) = audio_data {
            fallback_transcription(app, inserter, key, fallback_key, audio_data).await;
        } else {
            let _ = app.emit("error", "没有录制到音频数据".to_string());
        }
    }
}

/// 备用转录方案（HTTP 模式）
async fn fallback_transcription(
    app: AppHandle,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    key: String,
    fallback_key: String,
    audio_data: Vec<u8>,
) {
    let result = if !fallback_key.is_empty() {
        tracing::info!("使用 SenseVoice 备用方案");
        let sensevoice_client = qwen_asr::SenseVoiceClient::new(fallback_key);
        sensevoice_client.transcribe_bytes(&audio_data).await
    } else {
        tracing::info!("使用 HTTP 模式千问 ASR 备用");
        let asr_client = QwenASRClient::new(key);
        asr_client.transcribe_bytes(&audio_data).await
    };

    handle_transcription_result(app, inserter, result).await;
}

/// 实时模式转录处理（WebSocket）- 录完再传的回退模式
#[allow(dead_code)]
async fn handle_realtime_transcription(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    key: String,
    fallback_key: String,
) {
    let _ = app.emit("transcribing", ());

    // 停止流式录音，获取完整音频数据
    let audio_data = {
        let mut recorder_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_streaming() {
                Ok(data) => Some(data),
                Err(e) => {
                    tracing::error!("停止流式录音失败: {}", e);
                    let _ = app.emit("error", format!("停止录音失败: {}", e));
                    None
                }
            }
        } else {
            None
        }
    };

    if audio_data.is_none() {
        return;
    }

    let audio_data = audio_data.unwrap();

    // 尝试使用 WebSocket 实时 API
    tracing::info!("尝试使用 WebSocket 实时 API 转录...");

    let realtime_client = QwenRealtimeClient::new(key.clone());
    let ws_result = realtime_transcribe_audio(&realtime_client, &audio_data).await;

    match ws_result {
        Ok(text) => {
            tracing::info!("WebSocket 实时转录成功: {}", text);
            handle_transcription_result(app, inserter, Ok(text)).await;
        }
        Err(e) => {
            tracing::warn!("WebSocket 实时转录失败: {}，尝试备用方案", e);
            fallback_transcription(app, inserter, key, fallback_key, audio_data).await;
        }
    }
}

/// 使用 WebSocket 实时 API 转录音频
async fn realtime_transcribe_audio(
    client: &QwenRealtimeClient,
    wav_data: &[u8],
) -> anyhow::Result<String> {
    // 创建 WebSocket 会话
    let mut session = client.start_session().await?;

    // 从 WAV 数据中提取 PCM 样本
    let pcm_samples = extract_pcm_from_wav(wav_data)?;

    // 分块发送音频数据（每块 3200 样本 = 0.2秒 @ 16kHz）
    const CHUNK_SIZE: usize = 3200;
    for chunk in pcm_samples.chunks(CHUNK_SIZE) {
        session.send_audio_chunk(chunk).await?;
        // 模拟实时发送的间隔（可选，用于更真实的流式体验）
        // tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // 提交音频缓冲区
    session.commit_audio().await?;

    // 等待转录结果
    let result = session.wait_for_result().await?;

    // 关闭会话
    let _ = session.close().await;

    Ok(result)
}

/// 从 WAV 数据中提取 PCM 样本（16-bit, 16kHz, 单声道）
fn extract_pcm_from_wav(wav_data: &[u8]) -> anyhow::Result<Vec<i16>> {
    use std::io::Cursor;

    let cursor = Cursor::new(wav_data);
    let reader = hound::WavReader::new(cursor)?;

    let samples: Vec<i16> = reader.into_samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();

    Ok(samples)
}

/// 处理转录结果
async fn handle_transcription_result(
    app: AppHandle,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    result: anyhow::Result<String>,
) {
    match result {
        Ok(text) => {
            tracing::info!("转录结果: {}", text);

            // 插入文本
            let mut inserter_guard = inserter.lock().unwrap();
            if let Some(ref mut ins) = *inserter_guard {
                if let Err(e) = ins.insert_text(&text) {
                    tracing::error!("插入文本失败: {}", e);
                    let _ = app.emit("error", format!("插入文本失败: {}", e));
                }
            }

            let _ = app.emit("transcription_complete", text);
        }
        Err(e) => {
            tracing::error!("转录失败: {}", e);
            let _ = app.emit("error", format!("转录失败: {}", e));
        }
    }
}

#[tauri::command]
async fn stop_app(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("停止应用...");

    let state = app_handle.state::<AppState>();

    let mut is_running = state.is_running.lock().unwrap();
    if !*is_running {
        return Err("应用未在运行".to_string());
    }

    *state.audio_recorder.lock().unwrap() = None;
    *state.streaming_recorder.lock().unwrap() = None;
    *state.text_inserter.lock().unwrap() = None;
    *is_running = false;

    Ok("应用已停止".to_string())
}

#[tauri::command]
async fn cancel_transcription(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("取消转录...");

    let state = app_handle.state::<AppState>();

    // 1. 停止流式录音
    {
        let mut recorder_guard = state.streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            let _ = rec.stop_streaming();
        }
    }

    // 2. 停止普通录音
    {
        let mut recorder_guard = state.audio_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            let _ = rec.stop_recording_to_memory();
        }
    }

    // 3. 取消音频发送任务
    {
        let handle = state.audio_sender_handle.lock().unwrap().take();
        if let Some(h) = handle {
            h.abort();
            tracing::info!("已取消音频发送任务");
        }
    }

    // 4. 关闭 WebSocket 会话
    {
        let mut session_guard = state.active_session.lock().await;
        if let Some(ref session) = *session_guard {
            let _ = session.close().await;
            tracing::info!("已关闭 WebSocket 会话");
        }
        *session_guard = None;
    }

    // 5. 发送取消事件
    let _ = app_handle.emit("transcription_cancelled", ());

    Ok("已取消转录".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .setup(|app| {
            // 初始化应用状态
            let app_state = AppState {
                audio_recorder: Arc::new(Mutex::new(None)),
                streaming_recorder: Arc::new(Mutex::new(None)),
                text_inserter: Arc::new(Mutex::new(None)),
                is_running: Arc::new(Mutex::new(false)),
                use_realtime_asr: Arc::new(Mutex::new(true)),
                active_session: Arc::new(tokio::sync::Mutex::new(None)),
                audio_sender_handle: Arc::new(Mutex::new(None)),
            };

            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_config,
            load_config,
            start_app,
            stop_app,
            cancel_transcription,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
