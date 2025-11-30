// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_recorder;
mod beep_player;
mod config;
mod hotkey_service;
mod qwen_asr;
mod text_inserter;

use audio_recorder::AudioRecorder;
use config::AppConfig;
use hotkey_service::HotkeyService;
use qwen_asr::QwenASRClient;
use text_inserter::TextInserter;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

// 全局应用状态
struct AppState {
    audio_recorder: Arc<Mutex<Option<AudioRecorder>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    is_running: Arc<Mutex<bool>>,
}

// Tauri Commands

#[tauri::command]
async fn save_config(api_key: String, fallback_api_key: String) -> Result<String, String> {
    tracing::info!("保存配置...");
    let config = AppConfig {
        dashscope_api_key: api_key,
        siliconflow_api_key: fallback_api_key,
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
) -> Result<String, String> {
    tracing::info!("启动应用...");

    // 获取应用状态
    let state = app_handle.state::<AppState>();

    let mut is_running = state.is_running.lock().unwrap();
    if *is_running {
        return Err("应用已在运行中".to_string());
    }

    // 初始化音频录制器
    let audio_recorder = AudioRecorder::new()
        .map_err(|e| format!("初始化音频录制器失败: {}", e))?;
    *state.audio_recorder.lock().unwrap() = Some(audio_recorder);

    // 初始化文本插入器
    let text_inserter = TextInserter::new()
        .map_err(|e| format!("初始化文本插入器失败: {}", e))?;
    *state.text_inserter.lock().unwrap() = Some(text_inserter);

    // 启动全局快捷键监听
    let hotkey_service = HotkeyService::new();

    let app_handle_start = app_handle.clone();
    let audio_recorder_start = Arc::clone(&state.audio_recorder);

    let app_handle_stop = app_handle.clone();
    let audio_recorder_stop = Arc::clone(&state.audio_recorder);
    let text_inserter_stop = Arc::clone(&state.text_inserter);
    let api_key_clone = api_key.clone();
    let fallback_api_key_clone = fallback_api_key.clone();

    // 按键按下回调
    let on_start = move || {
        let app = app_handle_start.clone();
        let recorder = Arc::clone(&audio_recorder_start);

        // 播放开始录音提示音
        beep_player::play_start_beep();

        tauri::async_runtime::spawn(async move {
            tracing::info!("检测到快捷键按下");
            let _ = app.emit("recording_started", ());

            let mut recorder_guard = recorder.lock().unwrap();
            if let Some(ref mut rec) = *recorder_guard {
                if let Err(e) = rec.start_recording() {
                    tracing::error!("开始录音失败: {}", e);
                    let _ = app.emit("error", format!("录音失败: {}", e));
                }
            }
        });
    };

    // 按键释放回调
    let on_stop = move || {
        let app = app_handle_stop.clone();
        let recorder = Arc::clone(&audio_recorder_stop);
        let inserter = Arc::clone(&text_inserter_stop);
        let key = api_key_clone.clone();
        let fallback_key = fallback_api_key_clone.clone();

        // 播放停止录音提示音
        beep_player::play_stop_beep();

        tauri::async_runtime::spawn(async move {
            tracing::info!("检测到快捷键释放");
            let _ = app.emit("recording_stopped", ());

            // 停止录音并直接获取内存中的音频数据（跳过文件写入）
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
                // 发送转录中事件
                let _ = app.emit("transcribing", ());

                // 使用主备并行转录逻辑（直接从内存转录，无文件 I/O）
                let result = if !fallback_key.is_empty() {
                    // 如果配置了备用 API，使用主备并行逻辑
                    tracing::info!("使用主备并行转录模式 (内存直传)");
                    qwen_asr::transcribe_with_fallback_bytes(key, fallback_key, audio_data).await
                } else {
                    // 否则只使用千问 API
                    tracing::info!("仅使用千问 ASR (内存直传)");
                    let asr_client = QwenASRClient::new(key);
                    asr_client.transcribe_bytes(&audio_data).await
                };

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

                        // 发送转录完成事件
                        let _ = app.emit("transcription_complete", text);
                    }
                    Err(e) => {
                        tracing::error!("转录失败: {}", e);
                        let _ = app.emit("error", format!("转录失败: {}", e));
                    }
                }
                // 无需删除临时文件，因为音频数据直接在内存中处理
            }
        });
    };

    hotkey_service
        .start(on_start, on_stop)
        .map_err(|e| format!("启动快捷键监听失败: {}", e))?;

    *is_running = true;
    Ok("应用已启动，按 Ctrl+Win 开始录音".to_string())
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
    *state.text_inserter.lock().unwrap() = None;
    *is_running = false;

    Ok("应用已停止".to_string())
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
                text_inserter: Arc::new(Mutex::new(None)),
                is_running: Arc::new(Mutex::new(false)),
            };

            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_config,
            load_config,
            start_app,
            stop_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
