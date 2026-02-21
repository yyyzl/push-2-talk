// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod asr;
mod assistant_processor;
#[cfg(target_os = "windows")]
mod audio_mute_manager;
mod audio_recorder;
mod audio_utils;
mod beep_player;
mod builtin_dictionary_updater;
mod clipboard_manager;
mod commands;
mod config;
mod dictionary_utils;
mod hotkey_service;
mod learning;
mod llm_post_processor;
mod openai_client;
mod pipeline;
mod platform;
mod streaming_recorder;
mod text_inserter;
mod tray;
mod tnl;
#[cfg(target_os = "windows")]
mod uia_text_reader;
mod usage_stats;
#[cfg(target_os = "windows")]
mod win32_input;

use asr::{
    DoubaoASRClient, DoubaoImeCredentials, DoubaoImeRealtimeClient, DoubaoImeRealtimeSession,
    DoubaoRealtimeClient, DoubaoRealtimeSession, QwenASRClient, QwenRealtimeClient,
    RealtimeSession, SenseVoiceClient,
};
use assistant_processor::AssistantProcessor;
use audio_recorder::AudioRecorder;
use config::{AppConfig, CONFIG_LOCK};
use hotkey_service::HotkeyService;
use llm_post_processor::LlmPostProcessor;
use pipeline::{AssistantPipeline, NormalPipeline, TranscriptionContext};
use platform::audio_mute::AudioMuteManager;
use platform::types::WindowId;
use streaming_recorder::StreamingRecorder;
use text_inserter::TextInserter;
use usage_stats::UsageStats;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

pub(crate) use tray::{
    check_accessibility, check_input_monitoring, check_microphone_access, emit_config_updated,
    load_persisted_config, lock_hotwords_or_recover, mutate_persisted_config,
    mutate_persisted_config_with_result,
};

fn find_monitor_at_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let (cursor_x, cursor_y) = platform::cursor::get_cursor_position()?;

    if let Ok(Some(monitor)) = window.monitor_from_point(cursor_x as f64, cursor_y as f64) {
        return Some(monitor);
    }

    let monitors = window.available_monitors().ok()?;

    for monitor in monitors {
        let pos = monitor.position();
        let size = monitor.size();
        if cursor_x >= pos.x
            && cursor_x < pos.x + size.width as i32
            && cursor_y >= pos.y
            && cursor_y < pos.y + size.height as i32
        {
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
    /// 语句润色：是否启用“词库增强”（将个人词库注入提示词）
    enable_dictionary_enhancement: Arc<Mutex<bool>>,
    enable_fallback: Arc<Mutex<bool>>,
    qwen_client: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client: Arc<Mutex<Option<DoubaoASRClient>>>,
    // 活跃的实时转录会话（用于真正的流式传输）
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    doubao_ime_session: Arc<tokio::sync::Mutex<Option<DoubaoImeRealtimeSession>>>,
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
    target_window: Arc<Mutex<Option<WindowId>>>,
    /// 词库（用于 Realtime 模式热更新）
    dictionary: Arc<Mutex<Vec<String>>>,
    /// 豆包输入法凭据（自动注册获取，跨会话复用）
    doubao_ime_credentials: Arc<Mutex<Option<DoubaoImeCredentials>>>,
    /// 使用统计数据
    usage_stats: Arc<Mutex<UsageStats>>,
    /// 录音开始时间（用于计算录音时长）
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
    /// 内置词库原始内容（用于前端动态解析）
    builtin_hotwords_raw: Arc<Mutex<String>>,
    /// 内置词库后台更新任务是否已启动（进程级单例）
    builtin_dictionary_updater_started: Arc<AtomicBool>,
}

#[derive(Clone, serde::Serialize)]
struct BuiltinDictionaryUpdatedPayload {
    endpoint: String,
    changed: bool,
    size_bytes: usize,
}

#[derive(Clone, serde::Serialize)]
struct PermissionStatus {
    microphone: bool,
    input_monitoring: bool,
    accessibility: bool,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

const BUILTIN_DICTIONARY_UPDATE_INTERVAL_SECS: u64 = 6 * 60 * 60;

fn merge_asr_config_for_save(
    asr_config: Option<config::AsrConfig>,
    existing_asr_config: &config::AsrConfig,
    api_key: &str,
    fallback_api_key: &str,
) -> config::AsrConfig {
    match asr_config {
        Some(cfg) => cfg,
        None => {
            let mut fallback = existing_asr_config.clone();

            if !api_key.is_empty() {
                fallback.credentials.qwen_api_key = api_key.to_string();
            }

            if !fallback_api_key.is_empty() {
                fallback.credentials.sensevoice_api_key = fallback_api_key.to_string();
            }

            fallback
        }
    }
}

#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ConfigFieldPatch {
    learning_enabled: Option<bool>,
    theme: Option<String>,
    enable_mute_other_apps: Option<bool>,
    close_action: Option<Option<String>>,
}

// Tauri Commands

#[cfg(test)]
mod save_config_merge_tests {
    use super::*;

    fn build_asr_config(qwen_key: &str, sensevoice_key: &str) -> config::AsrConfig {
        config::AsrConfig {
            credentials: config::AsrCredentials {
                qwen_api_key: qwen_key.to_string(),
                sensevoice_api_key: sensevoice_key.to_string(),
                doubao_app_id: "doubao_app".to_string(),
                doubao_access_token: "doubao_token".to_string(),
                doubao_ime_device_id: "ime_device".to_string(),
                doubao_ime_token: "ime_token".to_string(),
                doubao_ime_cdid: "ime_cdid".to_string(),
            },
            selection: config::AsrSelection {
                active_provider: config::AsrProvider::Doubao,
                enable_fallback: true,
                fallback_provider: Some(config::AsrProvider::Qwen),
            },
            language_mode: config::AsrLanguageMode::Zh,
        }
    }

    #[test]
    fn should_keep_asr_credentials_when_asr_config_is_provided() {
        let existing = build_asr_config("existing_qwen", "existing_sensevoice");
        let incoming = build_asr_config("incoming_qwen", "incoming_sensevoice");

        let merged = merge_asr_config_for_save(
            Some(incoming.clone()),
            &existing,
            "stale_top_level_qwen",
            "stale_top_level_sensevoice",
        );

        assert_eq!(
            merged.credentials.qwen_api_key,
            incoming.credentials.qwen_api_key
        );
        assert_eq!(
            merged.credentials.sensevoice_api_key,
            incoming.credentials.sensevoice_api_key
        );
        assert_eq!(
            merged.selection.active_provider,
            incoming.selection.active_provider
        );
        assert_eq!(merged.language_mode, incoming.language_mode);
    }

    #[test]
    fn should_only_apply_top_level_keys_when_asr_config_is_missing() {
        let existing = build_asr_config("existing_qwen", "existing_sensevoice");

        let merged = merge_asr_config_for_save(None, &existing, "", "new_top_level_sensevoice");

        assert_eq!(merged.credentials.qwen_api_key, "existing_qwen");
        assert_eq!(
            merged.credentials.sensevoice_api_key,
            "new_top_level_sensevoice"
        );
        assert_eq!(merged.credentials.doubao_app_id, "doubao_app");
        assert_eq!(
            merged.selection.active_provider,
            config::AsrProvider::Doubao
        );
    }
}

// 生命周期命令与录音处理逻辑已迁移到 commands/lifecycle.rs。
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
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| tray::setup_app(app, start_minimized))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("close_requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::save_config,
            commands::config::patch_config_fields,
            commands::config::load_config,
            commands::config::get_builtin_domains_raw,
            commands::config::load_usage_stats,
            commands::lifecycle::start_app,
            commands::lifecycle::stop_app,
            commands::lifecycle::cancel_transcription,
            commands::lifecycle::finish_locked_recording,
            commands::lifecycle::cancel_locked_recording,
            commands::window::hide_to_tray,
            commands::window::quit_app,
            commands::window::show_overlay,
            commands::window::hide_overlay,
            commands::system::set_autostart,
            commands::system::set_learning_enabled,
            commands::system::get_autostart,
            commands::system::check_permissions,
            commands::system::reset_hotkey_state,
            commands::system::get_hotkey_service_active,
            commands::system::set_hotkey_service_active,
            commands::system::get_hotkey_debug_info,
            commands::lifecycle::update_runtime_config,
            commands::dictionary::add_learned_word,
            commands::dictionary::get_dictionary_entries,
            commands::dictionary::delete_dictionary_entries,
            commands::dictionary::dismiss_learning_suggestion,
            commands::dictionary::show_notification_window,
            commands::system::test_llm_provider,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
