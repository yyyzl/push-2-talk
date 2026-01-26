// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant_processor;
mod audio_mute_manager;
mod audio_recorder;
mod audio_utils;
mod asr;
mod beep_player;
mod clipboard_manager;
mod config;
mod hotkey_service;
mod learning;
mod llm_post_processor;
mod openai_client;
mod pipeline;
mod streaming_recorder;
mod text_inserter;
mod uia_text_reader;
mod usage_stats;
mod win32_input;

use audio_mute_manager::AudioMuteManager;
use audio_recorder::AudioRecorder;
use asr::{QwenASRClient, SenseVoiceClient, DoubaoASRClient, QwenRealtimeClient, DoubaoRealtimeClient, DoubaoRealtimeSession, RealtimeSession};
use assistant_processor::AssistantProcessor;
use config::AppConfig;
use hotkey_service::HotkeyService;
use llm_post_processor::LlmPostProcessor;
use pipeline::{AssistantPipeline, NormalPipeline, TranscriptionContext};
use streaming_recorder::StreamingRecorder;
use text_inserter::TextInserter;
use usage_stats::UsageStats;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    AppHandle, Emitter, Manager,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem},
    WindowEvent,
};

// ================== Windows 鼠标位置检测 ==================
#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
fn get_cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut point) != 0 {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

fn find_monitor_at_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let (cursor_x, cursor_y) = get_cursor_position()?;
    let monitors = window.available_monitors().ok()?;

    for monitor in monitors {
        let pos = monitor.position();
        let size = monitor.size();
        if cursor_x >= pos.x && cursor_x < pos.x + size.width as i32
           && cursor_y >= pos.y && cursor_y < pos.y + size.height as i32 {
            return Some(monitor);
        }
    }
    window.primary_monitor().ok().flatten()
}

// 全局应用状态
struct AppState {
    audio_recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    /// AI 助手处理器（支持双系统提示词）
    assistant_processor: Arc<Mutex<Option<AssistantProcessor>>>,
    is_running: Arc<Mutex<bool>>,
    use_realtime_asr: Arc<Mutex<bool>>,
    enable_post_process: Arc<Mutex<bool>>,
    enable_fallback: Arc<Mutex<bool>>,
    qwen_client: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client: Arc<Mutex<Option<DoubaoASRClient>>>,
    // 活跃的实时转录会话（用于真正的流式传输）
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    fallback_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    // 音频发送任务句柄
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    // 单例热键服务
    hotkey_service: Arc<HotkeyService>,
    /// 当前触发模式（听写/AI助手）
    current_trigger_mode: Arc<Mutex<Option<config::TriggerMode>>>,
    /// 松手模式：录音是否已锁定
    is_recording_locked: Arc<AtomicBool>,
    /// 松手模式：长按检测定时器句柄
    lock_timer_handle: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    /// 松手模式：录音开始时间（用于竞态条件检查）
    recording_start_time: Arc<Mutex<Option<std::time::Instant>>>,
    /// 松手模式：正在处理停止中（防止重复触发）
    is_processing_stop: Arc<AtomicBool>,
    /// 录音时静音其他应用的管理器
    audio_mute_manager: Arc<Mutex<Option<AudioMuteManager>>>,
    /// 目标窗口句柄（热键按下时保存，用于焦点恢复）
    target_window: Arc<Mutex<Option<isize>>>,
    /// 词库（用于 Realtime 模式热更新）
    dictionary: Arc<Mutex<Vec<String>>>,
    /// 使用统计数据
    usage_stats: Arc<Mutex<UsageStats>>,
    /// 录音开始时间（用于计算录音时长）
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
}

// Tauri Commands

#[tauri::command]
async fn save_config(
    app: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    smart_command_config: Option<config::SmartCommandConfig>,
    close_action: Option<String>,
    asr_config: Option<config::AsrConfig>,
    hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
    learning_config: Option<config::LearningConfig>,
    enable_mute_other_apps: Option<bool>,
    dictionary: Option<Vec<String>>,
    theme: Option<String>,
) -> Result<String, String> {
    tracing::info!("保存配置...");

    // 某些前端调用只会提交部分字段（例如仅更新热键/词库），这里用旧配置兜底避免意外清空。
    let existing = AppConfig::load().map(|(c, _)| c).unwrap_or_else(|_| AppConfig::new());

    // 智能合并 llm_config：如果传入的 presets 为空，保留旧值
    let final_llm_config = match llm_config {
        Some(mut cfg) if cfg.presets.is_empty() && !existing.llm_config.presets.is_empty() => {
            tracing::warn!("检测到空 presets，保留旧配置");
            cfg.presets = existing.llm_config.presets;
            cfg.active_preset_id = existing.llm_config.active_preset_id;
            cfg
        }
        Some(cfg) => cfg,
        None => existing.llm_config,
    };

    // 智能合并 assistant_config：如果传入的配置无效，保留旧值
    let final_assistant_config = match assistant_config {
        Some(cfg) if !cfg.is_valid_with_shared(&final_llm_config.shared) && existing.assistant_config.is_valid_with_shared(&final_llm_config.shared) => {
            tracing::warn!("检测到无效 assistant_config，保留旧配置");
            existing.assistant_config
        }
        Some(cfg) => cfg,
        None => existing.assistant_config,
    };

    // 智能合并 dictionary：如果传入空数组，保留旧值
    let final_dictionary = match dictionary {
        Some(dict) if dict.is_empty() && !existing.dictionary.is_empty() => {
            tracing::warn!("检测到空 dictionary，保留旧配置");
            existing.dictionary
        }
        Some(dict) => {
            // 前端传入的格式：纯词汇 "word" 或带来源 "word|auto"
            // 直接使用传入的数组，不再合并（前端已经是完整的词典状态）
            dict
        }
        None => existing.dictionary,
    };

    // 智能合并 dual_hotkey_config：如果传入空 keys，保留旧值
    let final_dual_hotkey_config = match dual_hotkey_config {
        Some(cfg) if cfg.dictation.keys.is_empty() || cfg.assistant.keys.is_empty() => {
            tracing::warn!("检测到空快捷键配置，保留旧配置");
            existing.dual_hotkey_config
        }
        Some(cfg) => cfg,
        None => existing.dual_hotkey_config,
    };

    let has_fallback = !fallback_api_key.is_empty();
    let config = AppConfig {
        dashscope_api_key: api_key.clone(),
        siliconflow_api_key: fallback_api_key.clone(),
        asr_config: asr_config.unwrap_or_else(|| config::AsrConfig {
            credentials: config::AsrCredentials {
                qwen_api_key: api_key,
                sensevoice_api_key: fallback_api_key,
                doubao_app_id: String::new(),
                doubao_access_token: String::new(),
            },
            selection: config::AsrSelection {
                active_provider: config::AsrProvider::Qwen,
                enable_fallback: has_fallback,
                fallback_provider: if has_fallback {
                    Some(config::AsrProvider::SiliconFlow)
                } else {
                    None
                },
            },
        }),
        use_realtime_asr: use_realtime.unwrap_or(existing.use_realtime_asr),
        enable_llm_post_process: enable_post_process.unwrap_or(existing.enable_llm_post_process),
        llm_config: final_llm_config,
        smart_command_config: smart_command_config.unwrap_or(existing.smart_command_config),
        assistant_config: final_assistant_config,
        learning_config: learning_config.unwrap_or(existing.learning_config),
        close_action: close_action.or(existing.close_action),
        hotkey_config,
        dual_hotkey_config: final_dual_hotkey_config,
        transcription_mode: existing.transcription_mode,
        enable_mute_other_apps: enable_mute_other_apps.unwrap_or(existing.enable_mute_other_apps),
        dictionary: final_dictionary,
        theme: theme.unwrap_or(existing.theme),
    };

    config
        .save()
        .map_err(|e| format!("保存配置失败: {}", e))?;

    tracing::info!("[save_config] 发送 config_updated 事件, theme={}", config.theme);
    let _ = app.emit("config_updated", &config);

    Ok("配置已保存".to_string())
}

#[tauri::command]
async fn load_config() -> Result<AppConfig, String> {
    tracing::info!("加载配置...");

    // 加载配置，如果发生迁移则自动保存
    match AppConfig::load() {
        Ok((config, migrated)) => {
            if migrated {
                tracing::info!("配置发生迁移，自动保存");
                if let Err(e) = config.save() {
                    tracing::error!("保存迁移后的配置失败: {}", e);
                }
            }
            Ok(config)
        }
        Err(e) => Err(format!("加载配置失败: {}", e)),
    }
}

#[tauri::command]
async fn load_usage_stats() -> Result<UsageStats, String> {
    tracing::info!("加载使用统计数据...");
    UsageStats::load().map_err(|e| format!("加载统计数据失败: {}", e))
}

/// 处理录音开始的核心逻辑
async fn handle_recording_start(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    use_realtime: bool,
    api_key: String,
    doubao_app_id: Option<String>,
    doubao_access_token: Option<String>,
    audio_mute_manager: Arc<Mutex<Option<AudioMuteManager>>>,
    dictionary: Vec<String>,
) {
    tracing::info!("检测到快捷键按下");

    // 录音开始时：增加会话计数并静音其他应用
    if let Some(ref manager) = *audio_mute_manager.lock().unwrap() {
        manager.begin_session();
        if let Err(e) = manager.mute_other_apps() {
            tracing::warn!("静音其他应用失败: {}", e);
        }
    }

    let _ = app.emit("recording_started", ());

    // 显示录音悬浮窗并移动到鼠标所在屏幕底部居中
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Some(monitor) = find_monitor_at_cursor(&overlay) {
            let monitor_pos = monitor.position();
            let screen_size = monitor.size();
            let scale_factor = monitor.scale_factor();
            let overlay_size = overlay.outer_size().unwrap_or(tauri::PhysicalSize::new(120, 44));

            // 全程使用物理像素计算
            let x = monitor_pos.x + (screen_size.width as i32 - overlay_size.width as i32) / 2;
            let y = monitor_pos.y + screen_size.height as i32 - overlay_size.height as i32 - (100.0 * scale_factor) as i32;

            let _ = overlay.set_position(tauri::PhysicalPosition::new(x, y));
        }
        let _ = overlay.show();
    }

    if use_realtime {
        let provider = realtime_provider.lock().unwrap().clone();
        match provider {
            Some(config::AsrProvider::Doubao) => {
                handle_doubao_realtime_start(app, streaming_recorder, doubao_session, audio_sender_handle, doubao_app_id, doubao_access_token, dictionary).await;
            }
            _ => {
                handle_qwen_realtime_start(app, streaming_recorder, active_session, audio_sender_handle, api_key, dictionary).await;
            }
        }
    } else {
        let mut recorder_guard = recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            // 检查是否已在录音，如果是则先停止
            if rec.is_recording() {
                tracing::warn!("发现正在进行的录音，先停止它");
                let _ = rec.stop_recording_to_memory();
            }
            if let Err(e) = rec.start_recording(Some(app.clone())) {
                emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
            }
        } else {
            emit_error_and_hide_overlay(&app, "录音器未初始化".to_string());
        }
    }
}

/// 处理豆包实时模式启动
async fn handle_doubao_realtime_start(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    doubao_app_id: Option<String>,
    doubao_access_token: Option<String>,
    dictionary: Vec<String>,
) {
    tracing::info!("启动豆包实时流式转录...");

    let chunk_rx = {
        let mut streaming_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *streaming_guard {
            // 检查是否已在录音，如果是则先停止
            if rec.is_recording() {
                tracing::warn!("发现正在进行的流式录音，先停止它");
                let _ = rec.stop_streaming();
            }
            match rec.start_streaming(Some(app.clone())) {
                Ok(rx) => Some(rx),
                Err(e) => {
                    emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                    None
                }
            }
        } else {
            emit_error_and_hide_overlay(&app, "流式录音器未初始化".to_string());
            None
        }
    };

    if let Some(chunk_rx) = chunk_rx {
        if let (Some(app_id), Some(access_token)) = (doubao_app_id.as_ref(), doubao_access_token.as_ref()) {
            let realtime_client = DoubaoRealtimeClient::new(app_id.clone(), access_token.clone(), dictionary);
            // 清理旧的会话和任务（防止资源泄漏）
            {
                let mut session_guard = doubao_session.lock().await;
                if let Some(mut old_session) = session_guard.take() {
                    tracing::warn!("发现旧的豆包会话，先关闭它");
                    let _ = old_session.finish_audio().await;
                }
            }
            {
                if let Some(old_handle) = audio_sender_handle.lock().unwrap().take() {
                    tracing::warn!("发现旧的音频发送任务，先取消它");
                    old_handle.abort();
                }
            }

            match realtime_client.start_session().await {
                Ok(session) => {
                    tracing::info!("豆包 WebSocket 连接已建立");
                    *doubao_session.lock().await = Some(session);

                    let session_for_sender = Arc::clone(&doubao_session);
                    let sender_handle = tokio::spawn(async move {
                        tracing::info!("豆包音频发送任务启动");
                        let mut chunk_count = 0;

                        while let Ok(chunk) = chunk_rx.recv() {
                            let mut session_guard = session_for_sender.lock().await;
                            if let Some(ref mut session) = *session_guard {
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

                        tracing::info!("豆包音频发送任务结束，共发送 {} 个块", chunk_count);
                    });

                    *audio_sender_handle.lock().unwrap() = Some(sender_handle);
                }
                Err(e) => {
                    tracing::error!("建立豆包 WebSocket 连接失败: {}，录音已启动，将使用备用方案", e);
                }
            }
        } else {
            tracing::error!("豆包凭证缺失：需要 app_id 和 access_token");
        }
    }
}

/// 处理千问实时模式启动
async fn handle_qwen_realtime_start(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    api_key: String,
    dictionary: Vec<String>,
) {
    tracing::info!("启动千问实时流式转录...");

    // 清理旧的会话和任务（防止资源泄漏）
    {
        let mut session_guard = active_session.lock().await;
        if let Some(old_session) = session_guard.take() {
            tracing::warn!("发现旧的千问会话，先关闭它");
            let _ = old_session.close().await;
        }
    }
    {
        if let Some(old_handle) = audio_sender_handle.lock().unwrap().take() {
            tracing::warn!("发现旧的音频发送任务，先取消它");
            old_handle.abort();
        }
    }

    let realtime_client = QwenRealtimeClient::new(api_key, dictionary);
    match realtime_client.start_session().await {
        Ok(session) => {
            tracing::info!("千问 WebSocket 连接已建立");

            let chunk_rx = {
                let mut streaming_guard = streaming_recorder.lock().unwrap();
                if let Some(ref mut rec) = *streaming_guard {
                    // 检查是否已在录音，如果是则先停止
                    if rec.is_recording() {
                        tracing::warn!("发现正在进行的流式录音，先停止它");
                        let _ = rec.stop_streaming();
                    }
                    match rec.start_streaming(Some(app.clone())) {
                        Ok(rx) => Some(rx),
                        Err(e) => {
                            emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                            None
                        }
                    }
                } else {
                    emit_error_and_hide_overlay(&app, "流式录音器未初始化".to_string());
                    None
                }
            };

            if let Some(chunk_rx) = chunk_rx {
                *active_session.lock().await = Some(session);

                let session_for_sender = Arc::clone(&active_session);
                let sender_handle = tokio::spawn(async move {
                    tracing::info!("千问音频发送任务启动");
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

                    tracing::info!("千问音频发送任务结束，共发送 {} 个块", chunk_count);
                });

                *audio_sender_handle.lock().unwrap() = Some(sender_handle);
            }
        }
        Err(e) => {
            tracing::error!("建立千问 WebSocket 连接失败: {}，回退到普通录音", e);

            let mut streaming_guard = streaming_recorder.lock().unwrap();
            if let Some(ref mut rec) = *streaming_guard {
                // 检查是否已在录音，如果是则先停止
                if rec.is_recording() {
                    tracing::warn!("发现正在进行的流式录音，先停止它");
                    let _ = rec.stop_streaming();
                }
                if let Err(e) = rec.start_streaming(Some(app.clone())) {
                    emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                }
            } else {
                emit_error_and_hide_overlay(&app, "录音器未初始化".to_string());
            }
        }
    }
}

#[tauri::command]
async fn start_app(
    app_handle: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    _smart_command_config: Option<config::SmartCommandConfig>,
    asr_config: Option<config::AsrConfig>,
    _hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
    enable_mute_other_apps: Option<bool>,
    dictionary: Option<Vec<String>>,
) -> Result<String, String> {
    tracing::info!("启动应用...");

    // 获取应用状态
    tracing::info!("[DEBUG] 获取应用状态...");
    let state = app_handle.state::<AppState>();
    tracing::info!("[DEBUG] 应用状态已获取");

    // 先检查是否已在运行（快速获取并释放锁）
    tracing::info!("[DEBUG] 检查运行状态...");
    let need_stop = {
        let is_running = state.is_running.lock().unwrap();
        tracing::info!("[DEBUG] 当前运行状态: {}", *is_running);
        *is_running
    }; // 锁在这里释放

    if need_stop {
        tracing::info!("[DEBUG] 检测到应用已在运行，自动停止中...");
        // 先停止应用（忽略停止时的错误）
        if let Err(e) = stop_app(app_handle.clone()).await {
            tracing::warn!("[DEBUG] 停止应用时出现警告: {}", e);
        }
        tracing::info!("[DEBUG] 应用已停止，继续启动流程");
    }

    tracing::info!("[DEBUG] 开始初始化...");

    // 确定是否使用实时模式
    let use_realtime_mode = use_realtime.unwrap_or(true);
    *state.use_realtime_asr.lock().unwrap() = use_realtime_mode;

    // 确定是否启用 LLM 后处理
    let enable_post_process_mode = enable_post_process.unwrap_or(false);
    *state.enable_post_process.lock().unwrap() = enable_post_process_mode;

    tracing::info!("ASR 模式: {}", if use_realtime_mode { "实时 WebSocket" } else { "HTTP" });
    tracing::info!("LLM 后处理: {}", if enable_post_process_mode { "启用" } else { "禁用" });

    let dict = dictionary.unwrap_or_default();
    tracing::info!("词库: {} 个词", dict.len());

    // 保存词库到 state（用于 Realtime 模式热更新）
    *state.dictionary.lock().unwrap() = dict.clone();

    // 根据 asr_config 初始化 ASR 客户端
    {
        *state.qwen_client.lock().unwrap() = None;
        *state.sensevoice_client.lock().unwrap() = None;
        *state.doubao_client.lock().unwrap() = None;

        if let Some(ref cfg) = asr_config {
            // 初始化所有有凭证的客户端
            if !cfg.credentials.qwen_api_key.is_empty() {
                *state.qwen_client.lock().unwrap() = Some(QwenASRClient::new(
                    cfg.credentials.qwen_api_key.clone(),
                    dict.clone(),
                ));
            }
            if !cfg.credentials.sensevoice_api_key.is_empty() {
                *state.sensevoice_client.lock().unwrap() = Some(SenseVoiceClient::new(
                    cfg.credentials.sensevoice_api_key.clone(),
                ));
            }
            if !cfg.credentials.doubao_app_id.is_empty() && !cfg.credentials.doubao_access_token.is_empty() {
                 *state.doubao_client.lock().unwrap() = Some(DoubaoASRClient::new(
                    cfg.credentials.doubao_app_id.clone(),
                    cfg.credentials.doubao_access_token.clone(),
                    dict.clone(),
                 ));
            }

            // 设置实时转录提供商
            *state.realtime_provider.lock().unwrap() = Some(cfg.selection.active_provider.clone());
            *state.fallback_provider.lock().unwrap() = cfg.selection.fallback_provider.clone();
        } else {
            // 旧逻辑回退（基本不会走到这里）
            if !api_key.is_empty() {
                *state.qwen_client.lock().unwrap() = Some(QwenASRClient::new(api_key.clone(), dict.clone()));
            }
            if !fallback_api_key.is_empty() {
                *state.sensevoice_client.lock().unwrap() = Some(SenseVoiceClient::new(fallback_api_key.clone()));
            }
        }
    }

    // 存储 fallback 配置
    {
        let enable_fb = asr_config
            .as_ref()
            .map(|c| c.selection.enable_fallback)
            .unwrap_or(false);
        *state.enable_fallback.lock().unwrap() = enable_fb;
        tracing::info!("并行 fallback: {}", if enable_fb { "启用" } else { "禁用" });
    }

    // 初始化 LLM 后处理器（复用连接）
    {
        let mut processor_guard = state.post_processor.lock().unwrap();
        let llm_cfg = llm_config.clone().unwrap_or_default();
        let resolved = llm_cfg.resolve_polishing();
        tracing::info!("[DEBUG] LLM 后处理配置: enabled={}, api_key_len={}", enable_post_process_mode, resolved.api_key.len());
        if enable_post_process_mode && !resolved.api_key.trim().is_empty() {
            tracing::info!("LLM 后处理器配置: endpoint={}, model={}", resolved.endpoint, resolved.model);
            *processor_guard = Some(LlmPostProcessor::new(llm_cfg));
            tracing::info!("LLM 后处理器已初始化");
        } else {
            *processor_guard = None;
            if enable_post_process_mode {
                tracing::warn!("LLM 后处理已启用但未配置 API Key，将跳过后处理");
            }
        }
    }

    // 初始化 AI 助手处理器（独立配置，支持双系统提示词，永远开启只需检查配置有效性）
    tracing::info!("[DEBUG] 初始化 AI 助手处理器...");
    {
        let mut processor_guard = state.assistant_processor.lock().unwrap();
        let assistant_cfg = assistant_config.unwrap_or_default();
        let llm_cfg = llm_config.unwrap_or_default();

        if assistant_cfg.is_valid_with_shared(&llm_cfg.shared) {
            tracing::info!("AI 助手处理器配置有效，正在初始化");
            *processor_guard = Some(AssistantProcessor::new(assistant_cfg, &llm_cfg.shared));
            tracing::info!("AI 助手处理器已初始化");
        } else {
            *processor_guard = None;
            tracing::info!("AI 助手未配置 API，Alt+Space 模式不可用");
        }
    }
    tracing::info!("[DEBUG] AI 助手处理器初始化完成");

    // 初始化文本插入器
    tracing::info!("[DEBUG] 初始化文本插入器...");
    let text_inserter = TextInserter::new()
        .map_err(|e| format!("初始化文本插入器失败: {}", e))?;
    *state.text_inserter.lock().unwrap() = Some(text_inserter);
    tracing::info!("[DEBUG] 文本插入器初始化完成");

    // 初始化或更新音频静音管理器
    {
        let should_mute = enable_mute_other_apps.unwrap_or(false);
        let mut manager_lock = state.audio_mute_manager.lock().unwrap();
        if let Some(ref manager) = *manager_lock {
            // 如果已经存在，直接更新开关状态
            manager.set_enabled(should_mute);
            tracing::info!("AudioMuteManager 已更新: enabled={}", should_mute);
        } else {
            // 如果不存在，创建新的
            *manager_lock = Some(AudioMuteManager::new(should_mute));
            tracing::info!("AudioMuteManager 已创建: enabled={}", should_mute);
        }
    }

    // 根据模式初始化录音器
    *state.audio_recorder.lock().unwrap() = None;
    *state.streaming_recorder.lock().unwrap() = None;

    if use_realtime_mode {
        let streaming_recorder = StreamingRecorder::new()
            .map_err(|e| format!("初始化流式录音器失败: {}", e))?;
        *state.streaming_recorder.lock().unwrap() = Some(streaming_recorder);
    } else {
        let audio_recorder = AudioRecorder::new()
            .map_err(|e| format!("初始化音频录制器失败: {}", e))?;
        *state.audio_recorder.lock().unwrap() = Some(audio_recorder);
    }

    // 启动全局快捷键监听（双模式支持）
    tracing::info!("[DEBUG] 准备热键配置...");
    let mut dual_hotkey_cfg = dual_hotkey_config.unwrap_or_default();

    // === 修复旧配置：如果 release_mode_keys 为 None，设置默认值 F2 ===
    if dual_hotkey_cfg.dictation.release_mode_keys.is_none() {
        dual_hotkey_cfg.dictation.release_mode_keys = Some(vec![config::HotkeyKey::F2]);
        tracing::info!("松手模式快捷键未配置，使用默认值 F2");
    }

    // 验证热键配置
    tracing::info!("[DEBUG] 验证热键配置...");
    dual_hotkey_cfg.validate()
        .map_err(|e| format!("热键配置无效: {}", e))?;
    tracing::info!("[DEBUG] 热键配置验证通过");

    let hotkey_service = Arc::clone(&state.hotkey_service);

    // 克隆状态用于回调（听写模式）
    let app_handle_start = app_handle.clone();
    let audio_recorder_start = Arc::clone(&state.audio_recorder);
    let streaming_recorder_start = Arc::clone(&state.streaming_recorder);
    let active_session_start = Arc::clone(&state.active_session);
    let doubao_session_start = Arc::clone(&state.doubao_session);
    let realtime_provider_start = Arc::clone(&state.realtime_provider);
    let audio_sender_handle_start = Arc::clone(&state.audio_sender_handle);
    let use_realtime_start = use_realtime_mode;
    let dictionary_state_start = Arc::clone(&state.dictionary);
    let is_running_start = Arc::clone(&state.is_running);
    // AI 助手模式专用
    let current_trigger_mode_start = Arc::clone(&state.current_trigger_mode);
    // 统计数据相关
    let recording_start_instant_start = Arc::clone(&state.recording_start_instant);

    // 保存当前的 provider 配置和凭证
    // 从 asr_config 中提取正确的 API Key（用于实时ASR）
    let (asr_api_key, doubao_app_id, doubao_access_token) = if let Some(ref cfg) = asr_config {
        *state.realtime_provider.lock().unwrap() = Some(cfg.selection.active_provider.clone());
        match cfg.selection.active_provider {
            config::AsrProvider::Qwen => (cfg.credentials.qwen_api_key.clone(), None, None),
            config::AsrProvider::Doubao => (
                String::new(),
                Some(cfg.credentials.doubao_app_id.clone()),
                Some(cfg.credentials.doubao_access_token.clone()),
            ),
            config::AsrProvider::SiliconFlow => (
                cfg.credentials.sensevoice_api_key.clone(),
                None,
                None,
            ),
        }
    } else {
        (String::new(), None, None)
    };
    let api_key_start = asr_api_key.clone();
    let doubao_app_id_start = doubao_app_id;
    let doubao_access_token_start = doubao_access_token;

    let app_handle_stop = app_handle.clone();
    let audio_recorder_stop = Arc::clone(&state.audio_recorder);
    let streaming_recorder_stop = Arc::clone(&state.streaming_recorder);
    let active_session_stop = Arc::clone(&state.active_session);
    let audio_sender_handle_stop = Arc::clone(&state.audio_sender_handle);
    let post_processor_stop = Arc::clone(&state.post_processor);
    let assistant_processor_stop = Arc::clone(&state.assistant_processor);
    let text_inserter_stop = Arc::clone(&state.text_inserter);
    let qwen_client_stop = Arc::clone(&state.qwen_client);
    let sensevoice_client_stop = Arc::clone(&state.sensevoice_client);
    let doubao_client_stop = Arc::clone(&state.doubao_client);
    let doubao_session_stop = Arc::clone(&state.doubao_session);
    let realtime_provider_stop = Arc::clone(&state.realtime_provider);
    let use_realtime_stop = use_realtime_mode;
    let is_running_stop = Arc::clone(&state.is_running);
    let enable_fallback_stop = Arc::clone(&state.enable_fallback);

    // 松手模式相关变量（用于 on_start）
    let is_recording_locked_start = Arc::clone(&state.is_recording_locked);
    let _lock_timer_handle_start = Arc::clone(&state.lock_timer_handle);
    let _recording_start_time_start = Arc::clone(&state.recording_start_time);
    let _dual_hotkey_cfg_start = dual_hotkey_cfg.clone();

    // 松手模式相关变量（用于 on_stop）
    let is_recording_locked_stop = Arc::clone(&state.is_recording_locked);
    let lock_timer_handle_stop = Arc::clone(&state.lock_timer_handle);
    let recording_start_time_stop = Arc::clone(&state.recording_start_time);
    let is_processing_stop_stop = Arc::clone(&state.is_processing_stop);

    // 音频静音管理器（用于 on_start 和 on_stop）
    let audio_mute_manager_start = Arc::clone(&state.audio_mute_manager);
    let audio_mute_manager_stop = Arc::clone(&state.audio_mute_manager);

    // 目标窗口句柄（用于焦点恢复）
    let target_window_start = Arc::clone(&state.target_window);
    let target_window_stop = Arc::clone(&state.target_window);

    // 统计数据相关（用于 on_stop）
    let usage_stats_stop = Arc::clone(&state.usage_stats);
    let recording_start_instant_stop = Arc::clone(&state.recording_start_instant);

    // 按键按下回调（支持双模式 + 松手模式）
    let on_start = move |trigger_mode: config::TriggerMode, is_release_mode: bool| {
        // === 防重入检查必须在保存窗口句柄之前 ===
        // 避免松手模式下误触热键覆盖正确的目标窗口句柄
        if is_recording_locked_start.load(Ordering::SeqCst) {
            tracing::info!("当前处于松手锁定模式，忽略新的按键触发");
            return;
        }

        if !*is_running_start.lock().unwrap() {
            tracing::debug!("服务已停止，忽略快捷键按下事件");
            return;
        }

        // === 保存目标窗口句柄（通过防重入检查后才保存） ===
        // 这是用户触发热键时的前台窗口，用于后续焦点恢复
        let target_hwnd = win32_input::get_foreground_window();
        *target_window_start.lock().unwrap() = target_hwnd;
        if let Some(hwnd) = target_hwnd {
            tracing::info!("已保存目标窗口句柄: 0x{:X}", hwnd);
        } else {
            tracing::warn!("未能获取目标窗口句柄");
        }

        // 保存当前触发模式
        *current_trigger_mode_start.lock().unwrap() = Some(trigger_mode);
        let mode_desc = if is_release_mode { "松手模式" } else { "普通模式" };
        tracing::info!("触发模式: {:?} ({})", trigger_mode, mode_desc);

        // 注意：剪贴板捕获已移至 on_stop 回调
        // 原因：在 on_start 时物理按键仍被按住，模拟 Ctrl+C 会与 Alt/Meta 等修饰键冲突

        beep_player::play_start_beep();

        let app = app_handle_start.clone();
        let recorder = Arc::clone(&audio_recorder_start);
        let streaming_recorder = Arc::clone(&streaming_recorder_start);
        let active_session = Arc::clone(&active_session_start);
        let doubao_session = Arc::clone(&doubao_session_start);
        let realtime_provider = Arc::clone(&realtime_provider_start);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_start);
        let use_realtime = use_realtime_start;
        let api_key = api_key_start.clone();
        let doubao_app_id = doubao_app_id_start.clone();
        let doubao_access_token = doubao_access_token_start.clone();
        let is_recording_locked_spawn = Arc::clone(&is_recording_locked_start);
        let audio_mute_manager = Arc::clone(&audio_mute_manager_start);
        let dictionary_state = Arc::clone(&dictionary_state_start);
        let recording_start_instant_spawn = Arc::clone(&recording_start_instant_start);

        tauri::async_runtime::spawn(async move {
            // 记录录音开始时间（包含录音准备时间：静音、显示窗口等）
            // 注意：这个时间略早于实际音频采集开始，但包含了用户感知到的准备时间
            *recording_start_instant_spawn.lock().unwrap() = Some(std::time::Instant::now());

            // 从 state 获取最新词库（支持热更新）
            let dictionary = dictionary_state.lock().unwrap().clone();
            // 1. 先执行开始录音逻辑 (内部会发送 recording_started 事件)
            handle_recording_start(
                app.clone(),
                recorder,
                streaming_recorder,
                active_session,
                doubao_session,
                realtime_provider,
                audio_sender_handle,
                use_realtime,
                api_key,
                doubao_app_id,
                doubao_access_token,
                audio_mute_manager,
                dictionary,
            ).await;

            // 2. 录音初始化完成后，再发送锁定事件
            // 这样前端会先收到 started (重置UI)，再收到 locked (切换为蓝色UI)
            if is_release_mode && trigger_mode == config::TriggerMode::Dictation {
                is_recording_locked_spawn.store(true, Ordering::SeqCst);
                let _ = app.emit("recording_locked", ());
                tracing::info!("通过松手模式快捷键启动，直接进入锁定状态");
            }
        });
    };

    // 按键释放回调（支持双模式）
    // 注意：is_release_mode = true 表示松手模式下再次按键完成录音
    let on_stop = move |trigger_mode: config::TriggerMode, is_release_mode: bool| {
        // 检查服务是否仍在运行
        if !*is_running_stop.lock().unwrap() {
            tracing::debug!("服务已停止，忽略快捷键释放事件");
            return;
        }

        // === 松手模式完成：用户再次按下快捷键完成录音并转写 ===
        if is_release_mode {
            tracing::info!("松手模式完成：用户再次按下快捷键，结束录音并转写");
            // 清除锁定状态，让代码继续执行正常的停止和转写流程
            is_recording_locked_stop.store(false, Ordering::SeqCst);
            *recording_start_time_stop.lock().unwrap() = None;
            if let Some(handle) = lock_timer_handle_stop.lock().unwrap().take() {
                handle.abort();
            }
            // 不 return，继续向下执行正常的停止录音和转写流程
        }

        // === 松手模式：立即清理定时器相关状态（防止竞态）===
        *recording_start_time_stop.lock().unwrap() = None;
        if let Some(handle) = lock_timer_handle_stop.lock().unwrap().take() {
            handle.abort();
        }

        // === 松手模式：检查锁定状态 ===
        if is_recording_locked_stop.load(Ordering::SeqCst) {
            tracing::info!("录音已锁定（松手模式），忽略物理按键释放");
            return; // 不停止录音，等待用户点击悬浮窗按钮
        }

        // === 防止与 finish_locked_recording 竞态 ===
        // 如果 finish_locked_recording 已经在处理，跳过 on_stop
        if is_processing_stop_stop.load(Ordering::SeqCst) {
            tracing::info!("finish_locked_recording 正在处理中，跳过 on_stop");
            return;
        }

        tracing::info!("检测到快捷键释放，模式: {:?}", trigger_mode);

        // 录音结束时：减少会话计数并恢复其他应用的音量
        if let Some(ref manager) = *audio_mute_manager_stop.lock().unwrap() {
            manager.end_session();
            if let Err(e) = manager.restore_volumes() {
                tracing::warn!("恢复其他应用音量失败: {}", e);
            }
        }

        let app = app_handle_stop.clone();
        let recorder = Arc::clone(&audio_recorder_stop);
        let streaming_recorder = Arc::clone(&streaming_recorder_stop);
        let active_session = Arc::clone(&active_session_stop);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_stop);
        let qwen_client_state = Arc::clone(&qwen_client_stop);
        let sensevoice_client_state = Arc::clone(&sensevoice_client_stop);
        let doubao_client_state = Arc::clone(&doubao_client_stop);
        let doubao_session_state = Arc::clone(&doubao_session_stop);
        let realtime_provider_state = Arc::clone(&realtime_provider_stop);
        let enable_fallback_state = Arc::clone(&enable_fallback_stop);
        let use_realtime = use_realtime_stop;

        // 根据触发模式选择处理器
        let post_processor = Arc::clone(&post_processor_stop);
        let assistant_processor = Arc::clone(&assistant_processor_stop);
        let text_inserter = Arc::clone(&text_inserter_stop);

        // 获取目标窗口句柄（用于焦点恢复）
        let target_hwnd = *target_window_stop.lock().unwrap();

        // 统计数据相关
        let usage_stats = Arc::clone(&usage_stats_stop);
        let recording_start_instant = Arc::clone(&recording_start_instant_stop);

        // 播放停止录音提示音
        beep_player::play_stop_beep();

        tauri::async_runtime::spawn(async move {
            let _ = app.emit("recording_stopped", ());

            match trigger_mode {
                config::TriggerMode::Dictation => {
                    // 听写模式：使用 NormalPipeline（纯转录 + 可选润色）
                    tracing::info!("使用听写模式处理");
                    if use_realtime {
                        handle_realtime_stop(
                            app,
                            streaming_recorder,
                            active_session,
                            doubao_session_state,
                            realtime_provider_state,
                            audio_sender_handle,
                            post_processor,
                            text_inserter,
                            qwen_client_state,
                            sensevoice_client_state,
                            doubao_client_state,
                            enable_fallback_state,
                            target_hwnd,
                            usage_stats.clone(),
                            recording_start_instant.clone(),
                        ).await;
                    } else {
                        handle_http_transcription(
                            app,
                            recorder,
                            post_processor,
                            text_inserter,
                            qwen_client_state,
                            sensevoice_client_state,
                            doubao_client_state,
                            enable_fallback_state,
                            target_hwnd,
                            usage_stats.clone(),
                            recording_start_instant.clone(),
                        ).await;
                    }
                }
                config::TriggerMode::AiAssistant => {
                    // AI 助手模式：使用 AssistantPipeline
                    tracing::info!("使用 AI 助手模式处理");

                    // 等待物理按键完全释放后再捕获剪贴板
                    // 原因：在 on_start 时物理按键仍被按住，模拟 Ctrl+C 会与 Alt/Meta 等修饰键冲突
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    // 捕获选中文本（此时用户已松开热键，Ctrl+C 模拟安全）
                    tracing::info!("AI 助手模式：开始捕获选中文本...");
                    let (clipboard_guard, selected_text) = match clipboard_manager::get_selected_text() {
                        Ok((guard, text)) => {
                            if let Some(ref t) = text {
                                tracing::info!("已捕获选中文本: {} 字符", t.len());
                            } else {
                                tracing::info!("无选中文本，将使用问答模式");
                            }
                            (Some(guard), text)
                        }
                        Err(e) => {
                            tracing::warn!("捕获选中文本失败: {}，继续处理但无上下文", e);
                            (None, None)
                        }
                    };

                    handle_assistant_mode(
                        app,
                        recorder,
                        streaming_recorder,
                        active_session,
                        doubao_session_state,
                        realtime_provider_state,
                        audio_sender_handle,
                        assistant_processor,
                        clipboard_guard,
                        selected_text,
                        qwen_client_state,
                        sensevoice_client_state,
                        doubao_client_state,
                        enable_fallback_state,
                        use_realtime,
                        target_hwnd,
                        usage_stats.clone(),
                        recording_start_instant.clone(),
                    ).await;
                }
            }
        });
    };

    tracing::info!("[DEBUG] 准备激活热键服务...");
    hotkey_service
        .activate_dual(dual_hotkey_cfg.clone(), on_start, on_stop)
        .map_err(|e| format!("启动快捷键监听失败: {}", e))?;
    tracing::info!("[DEBUG] 热键服务已激活");

    // 标记为运行中（重新获取锁）
    *state.is_running.lock().unwrap() = true;
    tracing::info!("[DEBUG] 启动完成!");
    let mode_str = if use_realtime_mode { "实时模式" } else { "HTTP 模式" };
    let dictation_display = dual_hotkey_cfg.dictation.format_display();
    let assistant_display = dual_hotkey_cfg.assistant.format_display();
    Ok(format!(
        "应用已启动 ({})，听写: {}，AI助手: {}",
        mode_str, dictation_display, assistant_display
    ))
}

/// AI 助手模式处理
///
/// 使用 AssistantPipeline 进行上下文感知的 LLM 处理
async fn handle_assistant_mode(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    assistant_processor: Arc<Mutex<Option<AssistantProcessor>>>,
    clipboard_guard: Option<clipboard_manager::ClipboardGuard>,
    selected_text: Option<String>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    use_realtime: bool,
    target_hwnd: Option<isize>,  // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    let _ = app.emit("transcribing", ());
    let asr_start = std::time::Instant::now();

    // 1. 停止录音并获取音频数据
    let (asr_result, audio_data) = if use_realtime {
        // 实时模式：先停止流式录音
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

        // 等待音频发送任务完成
        {
            let handle = audio_sender_handle.lock().unwrap().take();
            if let Some(h) = handle {
                tracing::info!("等待音频发送任务完成...");
                let _ = h.await;
            }
        }

        // 获取实时转录结果
        let provider = realtime_provider.lock().unwrap().clone();
        let result = match provider {
            Some(config::AsrProvider::Doubao) => {
                let mut session_guard = doubao_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.finish_audio().await;
                    let res = session.wait_for_result().await;
                    drop(session_guard);
                    *doubao_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的豆包会话"))
                }
            }
            _ => {
                let mut session_guard = active_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.commit_audio().await;
                    let res = session.wait_for_result().await;
                    let _ = session.close().await;
                    drop(session_guard);
                    *active_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的千问会话"))
                }
            }
        };

        (result, audio_data)
    } else {
        // HTTP 模式：停止录音并获取数据
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

        let result = if let Some(ref data) = audio_data {
            // 使用 HTTP ASR
            let enable_fb = *enable_fallback_state.lock().unwrap();
            let qwen = { qwen_client_state.lock().unwrap().clone() };
            let doubao = { doubao_client_state.lock().unwrap().clone() };
            let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
            let active_prov = realtime_provider.lock().unwrap().clone();
            let fallback_prov = app.state::<AppState>().fallback_provider.lock().unwrap().clone();

            transcribe_with_available_clients(qwen, doubao, sensevoice, data, enable_fb, active_prov, fallback_prov, "(AI助手HTTP) ").await
        } else {
            Err(anyhow::anyhow!("未获取到音频数据"))
        };

        (result, audio_data)
    };

    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    // 2. 如果实时模式失败且有音频数据，尝试 HTTP 备用
    let final_result = if asr_result.is_err() && audio_data.is_some() {
        tracing::warn!("实时 ASR 失败，尝试 HTTP 备用");
        let data = audio_data.unwrap();
        let enable_fb = *enable_fallback_state.lock().unwrap();
        let qwen = { qwen_client_state.lock().unwrap().clone() };
        let doubao = { doubao_client_state.lock().unwrap().clone() };
        let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
        let active_prov = realtime_provider.lock().unwrap().clone();
        let fallback_prov = app.state::<AppState>().fallback_provider.lock().unwrap().clone();

        transcribe_with_available_clients(qwen, doubao, sensevoice, &data, enable_fb, active_prov, fallback_prov, "(AI助手备用) ").await
    } else {
        asr_result
    };

    // 3. 使用 AssistantPipeline 处理
    let processor = { assistant_processor.lock().unwrap().clone() };
    let pipeline = AssistantPipeline::new();

    let context = TranscriptionContext {
        selected_text,
        ..Default::default()
    };

    let pipeline_result = pipeline
        .process(&app, processor, clipboard_guard, final_result, asr_time_ms, context, target_hwnd)
        .await;

    // 4. 处理结果
    match pipeline_result {
        Ok(result) => {
            hide_overlay_window(&app).await;

            // 更新统计数据（后端全权负责）
            if let Some(start_time) = recording_start_instant.lock().unwrap().take() {
                let recording_ms = start_time.elapsed().as_millis() as u64;
                // 统计非空白字符数（与前端旧逻辑保持一致）
                let recognized_chars = result.text.chars().filter(|c| !c.is_whitespace()).count() as u64;

                let mut stats = usage_stats.lock().unwrap();
                if let Err(e) = stats.update_and_save(recording_ms, recognized_chars) {
                    tracing::error!("更新统计数据失败: {}", e);
                }
            }

            let transcription_result = TranscriptionResult {
                text: result.text,
                original_text: result.original_text,
                asr_time_ms: result.asr_time_ms,
                llm_time_ms: result.llm_time_ms,
                total_time_ms: result.total_time_ms,
                mode: Some(format!("{:?}", result.mode).to_lowercase()),
                inserted: Some(result.inserted),
            };

            let _ = app.emit("transcription_complete", transcription_result);
        }
        Err(e) => {
            hide_overlay_window(&app).await;
            // 清理录音开始时间（防止下次录音时使用错误的时间）
            let _ = recording_start_instant.lock().unwrap().take();
            tracing::error!("AI 助手处理失败: {}", e);
            let _ = app.emit("error", format!("AI 助手处理失败: {}", e));
        }
    }
}

/// 统一的 HTTP ASR 转录逻辑
///
/// 根据配置的 active_provider 和 fallback_provider 选择合适的转录方式
async fn transcribe_with_available_clients(
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
                    asr::transcribe_with_fallback_clients(q.clone(), s.clone(), audio_data.to_vec()).await
                } else {
                    Err(anyhow::anyhow!("千问或 SenseVoice 客户端未初始化"))
                }
            }
            (Some(config::AsrProvider::Doubao), Some(config::AsrProvider::SiliconFlow)) => {
                if let (Some(d), Some(s)) = (&doubao, &sensevoice) {
                    tracing::info!("{}使用豆包+SenseVoice并行竞速", log_prefix);
                    asr::transcribe_doubao_sensevoice_race(d.clone(), s.clone(), audio_data.to_vec()).await
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
            None => {
                tracing::error!("{}未配置 ASR 提供商", log_prefix);
                Err(anyhow::anyhow!("ASR 提供商未配置"))
            }
        }
    }
}

/// HTTP 模式转录处理（听写模式专用）
async fn handle_http_transcription(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    target_hwnd: Option<isize>,  // 目标窗口句柄（用于焦点恢复）
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
        let active_prov = app.state::<AppState>().realtime_provider.lock().unwrap().clone();
        let fallback_prov = app.state::<AppState>().fallback_provider.lock().unwrap().clone();

        let asr_start = std::time::Instant::now();
        let result = transcribe_with_available_clients(
            qwen, doubao, sensevoice, &audio_data, enable_fallback, active_prov, fallback_prov, "(HTTP) "
        ).await;
        let asr_time_ms = asr_start.elapsed().as_millis() as u64;

        handle_transcription_result(app, post_processor, text_inserter, result, asr_time_ms, target_hwnd, usage_stats, recording_start_instant).await;
    }
}

/// 真正的实时模式停止处理（边录边传后的 commit + 等待结果）
async fn handle_realtime_stop(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    target_hwnd: Option<isize>,  // 目标窗口句柄（用于焦点恢复）
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
            let _ = h.await;
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
                        handle_transcription_result(app, post_processor, text_inserter, Ok(text), asr_time_ms, target_hwnd, usage_stats, recording_start_instant).await;
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
                        handle_transcription_result(app, post_processor, text_inserter, Ok(text), asr_time_ms, target_hwnd, usage_stats, recording_start_instant).await;
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

/// 备用转录方案（HTTP 模式，听写模式专用）
async fn fallback_transcription(
    app: AppHandle,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    audio_data: Vec<u8>,
    enable_fallback: bool,
    target_hwnd: Option<isize>,  // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    let qwen = { qwen_client_state.lock().unwrap().clone() };
    let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
    let doubao = { doubao_client_state.lock().unwrap().clone() };
    let active_prov = app.state::<AppState>().realtime_provider.lock().unwrap().clone();
    let fallback_prov = app.state::<AppState>().fallback_provider.lock().unwrap().clone();

    let asr_start = std::time::Instant::now();
    let result = transcribe_with_available_clients(
        qwen, doubao, sensevoice, &audio_data, enable_fallback, active_prov, fallback_prov, "(备用) "
    ).await;
    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    handle_transcription_result(app, post_processor, text_inserter, result, asr_time_ms, target_hwnd, usage_stats, recording_start_instant).await;
}

/// 统一的错误处理辅助函数 - 发送错误事件并隐藏悬浮窗
fn emit_error_and_hide_overlay(app: &AppHandle, error_msg: String) {
    tracing::error!("发送错误并隐藏悬浮窗: {}", error_msg);
    let _ = app.emit("error", error_msg);

    // 隐藏悬浮窗，带重试机制
    hide_overlay_silently(app);
}

/// 静默隐藏悬浮窗（不发送错误事件）
fn hide_overlay_silently(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败: {}", e);
            // 延迟 50ms 重试一次
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Err(e) = overlay.hide() {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }
}

/// 检查错误是否为"音频跳过"类型（用户误触等正常情况）
fn is_audio_skip_error(error: &anyhow::Error) -> bool {
    let msg = error.to_string();
    msg.contains("录音过短或无声音") || msg.contains("音频数据为空")
}

/// 转录完成事件的 payload
#[derive(Clone, serde::Serialize)]
struct TranscriptionResult {
    text: String,
    original_text: Option<String>, // 原始 ASR 文本（仅开启 LLM 润色时有值）
    asr_time_ms: u64,
    llm_time_ms: Option<u64>,
    total_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,  // 新增：处理模式
    #[serde(skip_serializing_if = "Option::is_none")]
    inserted: Option<bool>, // 新增：是否已自动插入
}

/// 处理转录结果（听写模式专用，使用 NormalPipeline）
///
/// 听写模式（Ctrl+Win）使用此函数处理 ASR 结果
/// AI 助手模式（Alt+Space）使用独立的 handle_assistant_mode 函数
async fn handle_transcription_result(
    app: AppHandle,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    result: anyhow::Result<String>,
    asr_time_ms: u64,
    target_hwnd: Option<isize>,  // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    // 从锁中提取处理器（clone 后立即释放锁）
    let post_proc = { post_processor.lock().unwrap().clone() };

    // 听写模式：只使用 NormalPipeline
    let pipeline = NormalPipeline::new();
    let mut inserter = { text_inserter.lock().unwrap().take() };
    let pipeline_result = pipeline
        .process(&app, post_proc, &mut inserter, result, asr_time_ms, TranscriptionContext::empty(), target_hwnd)
        .await;
    // 归还 text_inserter
    *text_inserter.lock().unwrap() = inserter;

    // 处理管道结果
    match pipeline_result {
        Ok(result) => {
            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

            // 更新统计数据（后端全权负责）
            if let Some(start_time) = recording_start_instant.lock().unwrap().take() {
                let recording_ms = start_time.elapsed().as_millis() as u64;
                // 统计非空白字符数（与前端旧逻辑保持一致）
                let recognized_chars = result.text.chars().filter(|c| !c.is_whitespace()).count() as u64;

                let mut stats = usage_stats.lock().unwrap();
                if let Err(e) = stats.update_and_save(recording_ms, recognized_chars) {
                    tracing::error!("更新统计数据失败: {}", e);
                }
            }

            // 构建兼容的 TranscriptionResult
            let transcription_result = TranscriptionResult {
                text: result.text,
                original_text: result.original_text,
                asr_time_ms: result.asr_time_ms,
                llm_time_ms: result.llm_time_ms,
                total_time_ms: result.total_time_ms,
                mode: Some(format!("{:?}", result.mode).to_lowercase()),
                inserted: Some(result.inserted),
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

/// 隐藏悬浮窗的辅助函数
async fn hide_overlay_window(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败: {}", e);
            // 延迟 50ms 重试一次
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Err(e) = overlay.hide() {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }
}

#[tauri::command]
async fn stop_app(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("停止应用...");

    let state = app_handle.state::<AppState>();

    {
        let is_running = state.is_running.lock().unwrap();
        if !*is_running {
            return Err("应用未在运行".to_string());
        }
    }

    // 停用热键服务（不终止线程）
    state.hotkey_service.deactivate();

    // 显式关闭活跃的 WebSocket Session
    {
        let mut session_guard = state.active_session.lock().await;
        if let Some(session) = session_guard.take() {
            let _ = session.close().await;
            tracing::info!("已关闭千问 WebSocket 会话");
        }
    }
    {
        let mut session_guard = state.doubao_session.lock().await;
        if let Some(mut session) = session_guard.take() {
            let _ = session.finish_audio().await;
            tracing::info!("已关闭豆包 WebSocket 会话");
        }
    }

    *state.audio_recorder.lock().unwrap() = None;
    *state.streaming_recorder.lock().unwrap() = None;
    *state.text_inserter.lock().unwrap() = None;
    *state.post_processor.lock().unwrap() = None;
    *state.assistant_processor.lock().unwrap() = None;
    *state.qwen_client.lock().unwrap() = None;
    *state.sensevoice_client.lock().unwrap() = None;
    *state.doubao_client.lock().unwrap() = None;
    *state.is_running.lock().unwrap() = false;

    Ok("应用已停止".to_string())
}

#[tauri::command]
async fn hide_to_tray(app_handle: AppHandle) -> Result<String, String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok("已最小化到托盘".to_string())
}

#[tauri::command]
async fn quit_app(app_handle: AppHandle) -> Result<(), String> {
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

    // 5. 隐藏录音悬浮窗（带重试机制）
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("取消转录时隐藏悬浮窗失败，准备重试: {}", e);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Err(e) = overlay.hide() {
                tracing::error!("取消转录时隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }

    // 6. 发送取消事件
    let _ = app_handle.emit("transcription_cancelled", ());

    Ok("已取消转录".to_string())
}

/// 完成锁定录音（松手模式）
/// 用户点击悬浮窗完成按钮时调用
#[tauri::command]
async fn finish_locked_recording(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("用户点击完成按钮，结束锁定录音");

    let state = app_handle.state::<AppState>();

    if !state.is_recording_locked.load(Ordering::SeqCst) {
        return Err("未处于锁定录音状态".to_string());
    }

    // 防止与 on_stop 竞态：使用 compare_exchange 原子操作
    // 如果已经在处理中，直接返回
    if state.is_processing_stop.compare_exchange(
        false, true, Ordering::SeqCst, Ordering::SeqCst
    ).is_err() {
        tracing::warn!("已有停止处理正在进行中，跳过重复触发");
        return Err("正在处理中".to_string());
    }

    // 清除锁定状态
    state.is_recording_locked.store(false, Ordering::SeqCst);
    *state.recording_start_time.lock().unwrap() = None;

    // 重置热键服务状态（防止状态卡死）
    state.hotkey_service.reset_state();

    // 获取并清空触发模式（松手模式仅支持听写模式）
    let trigger_mode = state.current_trigger_mode.lock().unwrap()
        .take()
        .unwrap_or(config::TriggerMode::Dictation);

    // 播放停止提示音
    beep_player::play_stop_beep();

    // 结束会话并恢复其他应用的音量
    if let Some(ref manager) = *state.audio_mute_manager.lock().unwrap() {
        manager.end_session();
        if let Err(e) = manager.restore_volumes() {
            tracing::warn!("恢复其他应用音量失败: {}", e);
        }
    }

    // 发送录音停止事件（前端会显示处理动画）
    let _ = app_handle.emit("recording_stopped", ());

    // 注意：不在这里隐藏窗口！
    // 窗口会在 Pipeline 的 insert_text 之前隐藏，这样用户能看到完整的处理动画
    // 隐藏逻辑已移至 pipeline/normal.rs 和 pipeline/assistant.rs

    // 获取需要的状态变量
    let use_realtime = *state.use_realtime_asr.lock().unwrap();
    let streaming_recorder = Arc::clone(&state.streaming_recorder);
    let audio_recorder = Arc::clone(&state.audio_recorder);
    let active_session = Arc::clone(&state.active_session);
    let doubao_session = Arc::clone(&state.doubao_session);
    let realtime_provider = Arc::clone(&state.realtime_provider);
    let audio_sender_handle = Arc::clone(&state.audio_sender_handle);
    let post_processor = Arc::clone(&state.post_processor);
    let text_inserter = Arc::clone(&state.text_inserter);
    let qwen_client = Arc::clone(&state.qwen_client);
    let sensevoice_client = Arc::clone(&state.sensevoice_client);
    let doubao_client = Arc::clone(&state.doubao_client);
    let enable_fallback = Arc::clone(&state.enable_fallback);
    let target_hwnd = *state.target_window.lock().unwrap();  // 获取目标窗口句柄
    let usage_stats = Arc::clone(&state.usage_stats);
    let recording_start_instant = Arc::clone(&state.recording_start_instant);

    // 执行停止处理（仅听写模式）
    let app = app_handle.clone();
    match trigger_mode {
        config::TriggerMode::Dictation => {
            if use_realtime {
                handle_realtime_stop(
                    app,
                    streaming_recorder,
                    active_session,
                    doubao_session,
                    realtime_provider,
                    audio_sender_handle,
                    post_processor,
                    text_inserter,
                    qwen_client,
                    sensevoice_client,
                    doubao_client,
                    enable_fallback,
                    target_hwnd,
                    usage_stats,
                    recording_start_instant,
                ).await;
            } else {
                handle_http_transcription(
                    app,
                    audio_recorder,
                    post_processor,
                    text_inserter,
                    qwen_client,
                    sensevoice_client,
                    doubao_client,
                    enable_fallback,
                    target_hwnd,
                    usage_stats,
                    recording_start_instant,
                ).await;
            }
        }
        config::TriggerMode::AiAssistant => {
            // 松手模式不支持 AI 助手模式，但为了安全性仍然处理
            tracing::warn!("松手模式不支持 AI 助手模式，跳过处理");
        }
    }

    // 重置处理标志
    state.is_processing_stop.store(false, Ordering::SeqCst);

    Ok("录音已完成".to_string())
}

/// 取消锁定录音（松手模式）
/// 用户点击悬浮窗取消按钮时调用
#[tauri::command]
async fn cancel_locked_recording(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("用户点击取消按钮，取消锁定录音");

    let state = app_handle.state::<AppState>();

    if !state.is_recording_locked.load(Ordering::SeqCst) {
        return Err("未处于锁定录音状态".to_string());
    }

    // 防止与 on_stop 竞态：使用 compare_exchange 原子操作
    if state.is_processing_stop.compare_exchange(
        false, true, Ordering::SeqCst, Ordering::SeqCst
    ).is_err() {
        tracing::warn!("已有停止处理正在进行中，跳过重复触发");
        return Err("正在处理中".to_string());
    }

    // 清除锁定状态
    state.is_recording_locked.store(false, Ordering::SeqCst);
    *state.recording_start_time.lock().unwrap() = None;
    *state.current_trigger_mode.lock().unwrap() = None;

    // 重置热键服务状态（防止状态卡死）
    state.hotkey_service.reset_state();

    // ===== 隐藏悬浮窗并主动恢复焦点 =====
    let target_hwnd = *state.target_window.lock().unwrap();
    tracing::info!("取消录音：隐藏悬浮窗并恢复焦点...");
    pipeline::focus::hide_overlay_and_restore_focus(&app_handle, target_hwnd).await;

    // 结束会话并恢复其他应用的音量
    if let Some(ref manager) = *state.audio_mute_manager.lock().unwrap() {
        manager.end_session();
        if let Err(e) = manager.restore_volumes() {
            tracing::warn!("恢复其他应用音量失败: {}", e);
        }
    }

    // 克隆 is_processing_stop 用于后续重置
    let is_processing_stop = Arc::clone(&state.is_processing_stop);

    // 调用现有的取消逻辑
    let result = cancel_transcription(app_handle).await;

    // 重置处理标志
    is_processing_stop.store(false, Ordering::SeqCst);

    result
}

/// 显示录音悬浮窗
#[tauri::command]
async fn show_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        overlay.show().map_err(|e| e.to_string())?;
        // 注意：不调用 set_focus()，避免抢夺用户当前窗口的焦点
    }
    Ok(())
}

/// 隐藏录音悬浮窗（带重试机制）
#[tauri::command]
async fn hide_overlay(app_handle: AppHandle) -> Result<(), String> {
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

/// 设置开机自启动
#[tauri::command]
async fn set_autostart(app: AppHandle, enabled: bool) -> Result<String, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(if enabled { "已启用开机自启" } else { "已禁用开机自启" }.to_string())
}

/// 获取开机自启动状态
#[tauri::command]
async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// 重置热键状态（用于手动修复状态卡死问题）
#[tauri::command]
async fn reset_hotkey_state(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    state.hotkey_service.reset_state();
    Ok("热键状态已重置".to_string())
}

/// 获取热键服务是否激活
#[tauri::command]
async fn get_hotkey_service_active(app_handle: AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<AppState>();
    Ok(state.hotkey_service.is_service_active())
}

/// 设置热键服务是否激活（用于录制快捷键时临时屏蔽）
#[tauri::command]
async fn set_hotkey_service_active(app_handle: AppHandle, active: bool) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    if active {
        state.hotkey_service.resume();
    } else {
        state.hotkey_service.deactivate();
    }
    Ok(())
}

/// 获取热键调试信息
#[tauri::command]
async fn get_hotkey_debug_info(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    Ok(state.hotkey_service.get_debug_info())
}

/// 运行时配置热更新（无需重启服务）
///
/// 用于在服务运行中即时更新配置，避免 stopApp → startApp 的延迟
#[tauri::command]
async fn update_runtime_config(
    app_handle: AppHandle,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    assistant_config: Option<config::AssistantConfig>,
    enable_mute_other_apps: Option<bool>,
    dictionary: Option<Vec<String>>,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();

    // 检查服务是否运行中
    let is_running = *state.is_running.lock().unwrap();
    if !is_running {
        return Ok("服务未运行，配置将在启动时生效".to_string());
    }

    let mut updated = Vec::new();

    // 1. 更新 LLM 后处理开关
    if let Some(enabled) = enable_post_process {
        *state.enable_post_process.lock().unwrap() = enabled;
        tracing::info!("热更新: LLM 后处理 = {}", enabled);
        updated.push("LLM后处理开关");
    }

    // 2. 更新 LLM 配置（重新初始化处理器）
    if let Some(ref cfg) = llm_config {
        let enable_pp = *state.enable_post_process.lock().unwrap();
        let mut processor_guard = state.post_processor.lock().unwrap();
        let resolved = cfg.resolve_polishing();
        if enable_pp && !resolved.api_key.trim().is_empty() {
            *processor_guard = Some(LlmPostProcessor::new(cfg.clone()));
            tracing::info!("热更新: LLM 处理器已重新初始化");
        } else {
            *processor_guard = None;
        }
        updated.push("LLM配置");
    }

    // 3. 更新 AI 助手配置
    if let Some(cfg) = assistant_config {
        let mut processor_guard = state.assistant_processor.lock().unwrap();

        // 获取 shared LLM 配置（从参数或从配置文件加载）
        let shared_config = if let Some(ref llm_cfg) = llm_config {
            llm_cfg.shared.clone()
        } else {
            // 如果没有传递 llm_config，从配置文件加载
            match config::AppConfig::load() {
                Ok((app_cfg, _)) => app_cfg.llm_config.shared,
                Err(e) => {
                    tracing::warn!("热更新: 无法加载 LLM 配置: {}", e);
                    config::SharedLlmConfig::default()
                }
            }
        };

        if cfg.is_valid_with_shared(&shared_config) {
            *processor_guard = Some(AssistantProcessor::new(cfg, &shared_config));
            tracing::info!("热更新: AI 助手处理器已重新初始化");
        } else {
            *processor_guard = None;
        }
        updated.push("AI助手配置");
    }

    // 4. 更新静音其他应用开关
    if let Some(should_mute) = enable_mute_other_apps {
        if let Some(ref manager) = *state.audio_mute_manager.lock().unwrap() {
            manager.set_enabled(should_mute);
            tracing::info!("热更新: 静音其他应用 = {}", should_mute);
            updated.push("静音开关");
        }
    }

    // 5. 更新词库（HTTP 客户端 + state.dictionary 用于 Realtime 模式）
    if let Some(dict) = dictionary {
        // 更新 state.dictionary（Realtime 模式会在每次录音开始时读取）
        *state.dictionary.lock().unwrap() = dict.clone();
        tracing::info!("热更新: state.dictionary 已更新 ({} 词)", dict.len());

        // 更新千问 HTTP 客户端
        if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
            client.update_dictionary(dict.clone());
            tracing::info!("热更新: 千问 ASR HTTP 客户端词库已更新");
        }
        // 更新豆包 HTTP 客户端
        if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
            client.update_dictionary(dict.clone());
            tracing::info!("热更新: 豆包 ASR HTTP 客户端词库已更新");
        }
        updated.push("词库");
    }

    if updated.is_empty() {
        Ok("无配置需要更新".to_string())
    } else {
        Ok(format!("已即时更新: {}", updated.join(", ")))
    }
}

// ============================================================================
// 词典管理命令（自动词库学习功能）
// ============================================================================

/// 添加学习到的词汇到词典
#[tauri::command]
async fn add_learned_word(
    app_handle: AppHandle,
    word: String,
    source: String,
) -> Result<(), String> {
    use crate::learning::store::{upsert_entry, entries_to_words};
    use crate::config::CONFIG_LOCK;

    tracing::info!("添加学习词汇: {} (来源: {})", word, source);

    // 使用全局锁保护配置操作，防止并发覆盖
    let _guard = CONFIG_LOCK.lock().map_err(|e| format!("获取配置锁失败: {}", e))?;

    // 加载当前配置
    let (mut config, _) = AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))?;

    // 添加词条（source: "manual" 或 "auto"）
    upsert_entry(&mut config.dictionary, &word, &source);

    // 保存配置
    config.save().map_err(|e| format!("保存配置失败: {}", e))?;

    // 热更新运行时词库
    let state = app_handle.state::<AppState>();
    let words = entries_to_words(&config.dictionary);
    *state.dictionary.lock().unwrap() = words.clone();

    // 更新 ASR 客户端词库
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_dictionary(words.clone());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_dictionary(words.clone());
    }

    // 发送事件通知前端刷新词典
    app_handle.emit("dictionary_updated", ()).ok();

    tracing::info!("词汇 '{}' 已添加到词典", word);
    Ok(())
}

/// 获取所有词典条目
#[tauri::command]
async fn get_dictionary_entries() -> Result<Vec<String>, String> {
    tracing::info!("获取词典条目...");

    let (config, _) = AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))?;

    tracing::info!("返回 {} 个词典条目", config.dictionary.len());
    Ok(config.dictionary)
}

/// 删除指定词汇的词典条目（按 word 匹配）
#[tauri::command]
async fn delete_dictionary_entries(
    app_handle: AppHandle,
    words: Vec<String>,
) -> Result<(), String> {
    use crate::learning::store::{remove_entries, entries_to_words};
    use crate::config::CONFIG_LOCK;

    tracing::info!("删除词典条目: {:?}", words);

    // 使用全局锁保护配置操作，防止并发覆盖
    let _guard = CONFIG_LOCK.lock().map_err(|e| format!("获取配置锁失败: {}", e))?;

    // 加载当前配置
    let (mut config, _) = AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))?;

    // 删除指定词汇（按 word 匹配，不区分来源）
    remove_entries(&mut config.dictionary, &words);

    // 保存配置
    config.save().map_err(|e| format!("保存配置失败: {}", e))?;

    // 热更新运行时词库
    let state = app_handle.state::<AppState>();
    let dict_words = entries_to_words(&config.dictionary);
    *state.dictionary.lock().unwrap() = dict_words.clone();

    // 更新 ASR 客户端词库
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_dictionary(dict_words.clone());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_dictionary(dict_words.clone());
    }

    // 发送事件通知前端刷新词典
    app_handle.emit("dictionary_updated", ()).ok();

    tracing::info!("词典条目删除完成");
    Ok(())
}

/// 忽略学习建议（暂不实现黑名单，仅关闭通知）
#[tauri::command]
async fn dismiss_learning_suggestion(id: String) -> Result<(), String> {
    tracing::debug!("忽略学习建议: {}", id);
    // 当前版本仅关闭通知，不实现黑名单机制
    // 未来可在此添加：将 id 对应的词汇加入黑名单，避免重复建议
    Ok(())
}

/// 显示通知窗口并定位到鼠标所在屏幕的悬浮窗上方
#[tauri::command]
async fn show_notification_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(notification) = app_handle.get_webview_window("notification") {
        // 使用 overlay 或 main 窗口获取显示器列表（这些窗口已正确初始化）
        // notification 窗口在首次显示前可能没有正确初始化
        let reference_window = app_handle.get_webview_window("overlay")
            .or_else(|| app_handle.get_webview_window("main"));

        if let Some(ref_win) = reference_window {
            if let Some(monitor) = find_monitor_at_cursor(&ref_win) {
                let monitor_pos = monitor.position();
                let screen_size = monitor.size();
                let scale_factor = monitor.scale_factor();

                // 通知窗口尺寸（tauri.conf.json 中是逻辑像素，需转换为物理像素）
                let window_width = (360.0 * scale_factor) as i32;
                let window_height = (600.0 * scale_factor) as i32;

                // 悬浮窗底部边距 100px + 悬浮窗高度 80px + 间隔 80px = 260px（逻辑像素）
                // 通知窗口底部距离屏幕底部的距离（物理像素）
                let bottom_offset = (260.0 * scale_factor) as i32;

                // 水平居中
                let x = monitor_pos.x + (screen_size.width as i32 - window_width) / 2;
                // 垂直方向：在悬浮窗上方约 150px
                let y = monitor_pos.y + screen_size.height as i32 - window_height - bottom_offset;

                // 确保不超出屏幕顶部（至少留 50 逻辑像素）
                let top_margin = (50.0 * scale_factor) as i32;
                let y = y.max(monitor_pos.y + top_margin);

                notification.set_position(tauri::PhysicalPosition::new(x, y))
                    .map_err(|e| format!("设置窗口位置失败: {}", e))?;
            }
        }

        // 避免抢占焦点：学习观察依赖前台窗口 hwnd，一旦通知窗口 set_focus 会导致学习误判“失焦”。
        // 通知窗口只需可见即可。
        notification.show().map_err(|e| format!("显示窗口失败: {}", e))?;

        Ok(())
    } else {
        Err("通知窗口不存在".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 检查是否静默启动（开机自启时）
    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.contains(&"--minimized".to_string());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 当第二个实例启动时，将焦点切换到已有实例的主窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| {
            // 如果是静默启动，隐藏主窗口
            if start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    tracing::info!("静默启动模式：主窗口已隐藏");
                }
            }

            // 初始化应用状态
            let usage_stats = UsageStats::load().unwrap_or_else(|e| {
                tracing::warn!("加载统计数据失败: {}, 使用默认值", e);
                UsageStats::default()
            });

            let app_state = AppState {
                audio_recorder: Arc::new(Mutex::new(None)),
                streaming_recorder: Arc::new(Mutex::new(None)),
                text_inserter: Arc::new(Mutex::new(None)),
                post_processor: Arc::new(Mutex::new(None)),
                assistant_processor: Arc::new(Mutex::new(None)),
                is_running: Arc::new(Mutex::new(false)),
                use_realtime_asr: Arc::new(Mutex::new(true)),
                enable_post_process: Arc::new(Mutex::new(false)),
                enable_fallback: Arc::new(Mutex::new(false)),
                qwen_client: Arc::new(Mutex::new(None)),
                sensevoice_client: Arc::new(Mutex::new(None)),
                doubao_client: Arc::new(Mutex::new(None)),
                active_session: Arc::new(tokio::sync::Mutex::new(None)),
                doubao_session: Arc::new(tokio::sync::Mutex::new(None)),
                realtime_provider: Arc::new(Mutex::new(None)),
                fallback_provider: Arc::new(Mutex::new(None)),
                audio_sender_handle: Arc::new(Mutex::new(None)),
                hotkey_service: Arc::new(HotkeyService::new()),
                current_trigger_mode: Arc::new(Mutex::new(None)),
                is_recording_locked: Arc::new(AtomicBool::new(false)),
                lock_timer_handle: Arc::new(Mutex::new(None)),
                recording_start_time: Arc::new(Mutex::new(None)),
                is_processing_stop: Arc::new(AtomicBool::new(false)),
                audio_mute_manager: Arc::new(Mutex::new(None)),
                target_window: Arc::new(Mutex::new(None)),
                dictionary: Arc::new(Mutex::new(Vec::new())),
                usage_stats: Arc::new(Mutex::new(usage_stats)),
                recording_start_instant: Arc::new(Mutex::new(None)),
            };

            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("PushToTalk - AI 语音转写助手")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            app.manage(app_state);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("close_requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            save_config,
            load_config,
            load_usage_stats,
            start_app,
            stop_app,
            cancel_transcription,
            finish_locked_recording,
            cancel_locked_recording,
            hide_to_tray,
            quit_app,
            show_overlay,
            hide_overlay,
            set_autostart,
            get_autostart,
            reset_hotkey_state,
            get_hotkey_service_active,
            set_hotkey_service_active,
            get_hotkey_debug_info,
            update_runtime_config,
            add_learned_word,
            get_dictionary_entries,
            delete_dictionary_entries,
            dismiss_learning_suggestion,
            show_notification_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
