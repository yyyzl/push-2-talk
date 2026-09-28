// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod application;
mod shell;
use application::assistant::{cancel_assistant_generation, handle_assistant_mode};
use application::configuration::*;
use application::runtime::AppState;
use application::transcription::{handle_http_transcription, handle_realtime_stop};
use shell::windows::{
    emit_error_and_hide_overlay, find_monitor_at_cursor, hide_result_panel_window,
};
pub mod asr;
use asr::qwen_models::{profile_model, QwenMode, QwenModel};
mod assistant_processor;
mod audio_recorder;
mod audio_utils;
mod beep_player;
mod builtin_dictionary_updater;
mod clipboard_manager;
mod config;
mod dictionary_utils;
mod learning;
mod llm_post_processor;
mod llm_reasoning;
mod openai_client;
pub mod personalization;
pub use tnl::{clean_disfluency, DisfluencyMode, DisfluencyResult};
mod pipeline;
mod platform;
#[cfg(all(feature = "atdd", not(debug_assertions)))]
compile_error!("The ATDD harness must not be included in a release build");
#[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
mod atdd;
use platform::InputTarget;
mod search;
mod streaming_recorder;
mod text_inserter;
mod tnl;
mod usage_stats;

use asr::{
    DoubaoASRClient, DoubaoImeCredentials, DoubaoImeRealtimeClient, DoubaoImeRealtimeSession,
    DoubaoRealtimeClient, DoubaoRealtimeSession, QwenASRClient, QwenRealtimeClient,
    RealtimeSession, SenseVoiceClient,
};
use assistant_processor::AssistantProcessor;
use audio_recorder::AudioRecorder;
use config::AppConfig;
use futures_util::FutureExt;
use llm_post_processor::LlmPostProcessor;
use openai_client::{ChatOptions, Message, OpenAiClient, OpenAiClientConfig};
use personalization::CorrectionPair;
use platform::AudioMuteManager;
use streaming_recorder::StreamingRecorder;
use text_inserter::TextInserter;
use usage_stats::UsageStats;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

#[derive(Clone, serde::Serialize)]
struct BuiltinDictionaryUpdatedPayload {
    endpoint: String,
    changed: bool,
    size_bytes: usize,
}

const BUILTIN_DICTIONARY_UPDATE_INTERVAL_SECS: u64 = 6 * 60 * 60;

struct TrayMenuState {
    post_process_item: CheckMenuItem<tauri::Wry>,
    dictionary_enhancement_item: CheckMenuItem<tauri::Wry>,
    web_search_item: CheckMenuItem<tauri::Wry>,
    asr_qwen_item: CheckMenuItem<tauri::Wry>,
    asr_doubao_item: CheckMenuItem<tauri::Wry>,
    asr_doubao_ime_item: CheckMenuItem<tauri::Wry>,
}

const TRAY_MENU_ID_SHOW: &str = "show";
const TRAY_MENU_ID_QUIT: &str = "quit";
const TRAY_MENU_ID_TOGGLE_POST_PROCESS: &str = "tray_toggle_post_process";
const TRAY_MENU_ID_TOGGLE_DICTIONARY_ENHANCEMENT: &str = "tray_toggle_dictionary_enhancement";
const TRAY_MENU_ID_TOGGLE_WEB_SEARCH: &str = "tray_toggle_web_search";
const TRAY_MENU_ID_ASR_QWEN: &str = "tray_asr_qwen";
const TRAY_MENU_ID_ASR_DOUBAO: &str = "tray_asr_doubao";
const TRAY_MENU_ID_ASR_DOUBAO_IME: &str = "tray_asr_doubao_ime";

/// 全局互斥标志：防止并发 ASR 引擎切换导致多个 restart 并行执行
static TRAY_ASR_SWITCHING: AtomicBool = AtomicBool::new(false);

fn sync_tray_menu_from_config(app_handle: &AppHandle, config: &AppConfig) {
    let Some(tray_state) = app_handle.try_state::<TrayMenuState>() else {
        return;
    };

    if let Err(e) = tray_state
        .post_process_item
        .set_checked(config.enable_llm_post_process)
    {
        tracing::warn!("同步托盘语句润色状态失败: {}", e);
    }
    if let Err(e) = tray_state
        .dictionary_enhancement_item
        .set_checked(config.enable_dictionary_enhancement)
    {
        tracing::warn!("同步托盘词库增强状态失败: {}", e);
    }
    if let Err(e) = tray_state
        .web_search_item
        .set_checked(config.assistant_config.enable_web_search)
    {
        tracing::warn!("同步托盘联网搜索状态失败: {}", e);
    }

    sync_asr_provider_checks(
        &tray_state.asr_qwen_item,
        &tray_state.asr_doubao_item,
        &tray_state.asr_doubao_ime_item,
        &config.asr_config.selection.active_provider,
    );
}

fn emit_config_updated(app: &AppHandle, config: &AppConfig) {
    // Concurrent commits may finish emitting out of order. Versioned consumers ignore stale snapshots.
    match load_config_snapshot() {
        Ok(snapshot) => {
            sync_tray_menu_from_config(app, &snapshot.config);
            let _ = app.emit("config_updated", &snapshot.config);
            let _ = app.emit("config_snapshot_updated", &snapshot);
        }
        Err(error) => {
            tracing::warn!("读取已提交配置快照失败: {error}");
            sync_tray_menu_from_config(app, config);
            let _ = app.emit("config_updated", config);
        }
    }
}

fn hotwords_content_changed(current: &str, next: &str) -> bool {
    current.trim() != next.trim()
}

fn lock_hotwords_or_recover<'a>(
    hotwords: &'a Arc<Mutex<String>>,
) -> std::sync::MutexGuard<'a, String> {
    match hotwords.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("内置词库缓存锁已 poisoned，继续使用恢复后的数据");
            poisoned.into_inner()
        }
    }
}

async fn refresh_builtin_dictionary_once(
    app_handle: &AppHandle,
    builtin_hotwords_raw: &Arc<Mutex<String>>,
) {
    let (content, endpoint) = match builtin_dictionary_updater::fetch_remote_hotwords().await {
        Ok(result) => result,
        Err(err) => {
            tracing::warn!("拉取内置词库失败: {}", err);
            return;
        }
    };

    let changed_before_persist = {
        let guard = lock_hotwords_or_recover(builtin_hotwords_raw);
        hotwords_content_changed(&guard, &content)
    };

    if !changed_before_persist {
        tracing::debug!("内置词库内容未变化，跳过更新广播");
        return;
    }

    if let Err(err) = builtin_dictionary_updater::save_cache_atomic(&content) {
        tracing::warn!("保存内置词库缓存失败: {}", err);
        return;
    }

    let changed = {
        let mut guard = lock_hotwords_or_recover(builtin_hotwords_raw);
        if !hotwords_content_changed(&guard, &content) {
            false
        } else {
            *guard = content.clone();
            true
        }
    };

    if !changed {
        tracing::debug!("内置词库内存快照已更新，跳过重复广播");
        return;
    }

    let payload = BuiltinDictionaryUpdatedPayload {
        endpoint,
        changed: true,
        size_bytes: content.len(),
    };

    if let Err(err) = app_handle.emit("builtin_dictionary_updated", payload) {
        tracing::warn!("广播内置词库更新事件失败: {}", err);
    }
}

fn start_builtin_dictionary_updater(
    app_handle: &AppHandle,
    updater_started: &Arc<AtomicBool>,
    builtin_hotwords_raw: &Arc<Mutex<String>>,
) {
    if updater_started
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::info!("内置词库后台更新任务已启动，跳过重复创建");
        return;
    }

    let app_handle = app_handle.clone();
    let updater_started = Arc::clone(updater_started);
    let builtin_hotwords_raw = Arc::clone(builtin_hotwords_raw);
    tauri::async_runtime::spawn(async move {
        let updater_loop = async {
            refresh_builtin_dictionary_once(&app_handle, &builtin_hotwords_raw).await;

            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                BUILTIN_DICTIONARY_UPDATE_INTERVAL_SECS,
            ));
            interval.tick().await;
            loop {
                interval.tick().await;
                refresh_builtin_dictionary_once(&app_handle, &builtin_hotwords_raw).await;
            }
        };

        let run_result = std::panic::AssertUnwindSafe(updater_loop)
            .catch_unwind()
            .await;
        updater_started.store(false, Ordering::SeqCst);

        if run_result.is_err() {
            tracing::error!("内置词库后台更新任务异常退出，已允许重启");
        }
    });
}

fn asr_provider_name(provider: &config::AsrProvider) -> &'static str {
    match provider {
        config::AsrProvider::Qwen => "千问",
        config::AsrProvider::Doubao => "豆包",
        config::AsrProvider::DoubaoIme => "豆包输入法",
        config::AsrProvider::SiliconFlow => "硅基流动",
    }
}

fn is_asr_provider_configured(config: &AppConfig, provider: &config::AsrProvider) -> bool {
    match provider {
        config::AsrProvider::Qwen => !config.asr_config.credentials.qwen_api_key.trim().is_empty(),
        config::AsrProvider::Doubao => {
            !config
                .asr_config
                .credentials
                .doubao_app_id
                .trim()
                .is_empty()
                && !config
                    .asr_config
                    .credentials
                    .doubao_access_token
                    .trim()
                    .is_empty()
        }
        // DoubaoIme 凭证是首次使用时自动注册获取的，无需用户预先配置
        config::AsrProvider::DoubaoIme => true,
        config::AsrProvider::SiliconFlow => !config
            .asr_config
            .credentials
            .sensevoice_api_key
            .trim()
            .is_empty(),
    }
}

fn sync_asr_provider_checks(
    qwen_item: &CheckMenuItem<tauri::Wry>,
    doubao_item: &CheckMenuItem<tauri::Wry>,
    doubao_ime_item: &CheckMenuItem<tauri::Wry>,
    provider: &config::AsrProvider,
) {
    let qwen_checked = matches!(provider, config::AsrProvider::Qwen);
    let doubao_checked = matches!(provider, config::AsrProvider::Doubao);
    let doubao_ime_checked = matches!(provider, config::AsrProvider::DoubaoIme);

    if let Err(e) = qwen_item.set_checked(qwen_checked) {
        tracing::warn!("更新托盘千问勾选状态失败: {}", e);
    }
    if let Err(e) = doubao_item.set_checked(doubao_checked) {
        tracing::warn!("更新托盘豆包勾选状态失败: {}", e);
    }
    if let Err(e) = doubao_ime_item.set_checked(doubao_ime_checked) {
        tracing::warn!("更新托盘豆包输入法勾选状态失败: {}", e);
    }
}

fn load_asr_correction_pairs_or_empty() -> Vec<CorrectionPair> {
    let enabled = crate::application::configuration::load_persisted_config()
        .map(|config| config.tnl_config.enable_personalization_hotwords)
        .unwrap_or(false);
    match crate::personalization::default_correction_pairs_path() {
        Ok(path) => load_asr_correction_pairs_from_path_if_enabled(&path, enabled),
        Err(e) => {
            tracing::warn!("ASR 热词纠错对路径解析失败，跳过 correction pairs: {}", e);
            Vec::new()
        }
    }
}

fn load_asr_correction_pairs_from_path_if_enabled(
    path: &std::path::Path,
    enabled: bool,
) -> Vec<CorrectionPair> {
    if !enabled {
        return Vec::new();
    }
    load_asr_correction_pairs_from_path_or_empty(path)
}

fn load_asr_correction_pairs_from_path_or_empty(path: &std::path::Path) -> Vec<CorrectionPair> {
    match crate::personalization::CorrectionPairStore::load_json_or_default(path) {
        Ok(store) => store.pairs().to_vec(),
        Err(e) => {
            tracing::warn!("ASR 热词纠错对加载失败，降级为仅用户词热词: {}", e);
            Vec::new()
        }
    }
}

fn refresh_asr_correction_pairs_runtime(state: &AppState) -> Vec<CorrectionPair> {
    let correction_pairs = load_asr_correction_pairs_or_empty();
    *state.asr_correction_pairs.lock().unwrap() = correction_pairs.clone();
    update_asr_http_clients_correction_pairs(state, &correction_pairs);
    tracing::info!("ASR 热词纠错对缓存已刷新: {} 条", correction_pairs.len());
    correction_pairs
}

fn update_asr_http_clients_correction_pairs(state: &AppState, correction_pairs: &[CorrectionPair]) {
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_correction_pairs(correction_pairs.to_vec());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_correction_pairs(correction_pairs.to_vec());
    }
}

#[cfg(test)]
mod asr_hotword_runtime_tests {
    use super::*;

    #[test]
    fn missing_correction_pairs_file_loads_empty_for_asr_hotwords() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("missing.json");

        let pairs = load_asr_correction_pairs_from_path_or_empty(&path);

        assert!(pairs.is_empty());
    }

    #[test]
    fn loads_correction_pairs_for_asr_hotwords() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp
            .path()
            .join("personalization")
            .join("correction_pairs.json");
        let pair = CorrectionPair::new("cloud-code", "cloud code", "Claude Code");
        crate::personalization::CorrectionPairStore::new(vec![pair])
            .save_json(&path)
            .expect("save correction pairs");

        let pairs = load_asr_correction_pairs_from_path_or_empty(&path);

        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].corrected_text, "Claude Code");
        assert!(load_asr_correction_pairs_from_path_if_enabled(&path, false).is_empty());
        assert_eq!(
            load_asr_correction_pairs_from_path_if_enabled(&path, true).len(),
            1
        );
    }
}

async fn restart_service_with_config(
    app_handle: AppHandle,
    config: AppConfig,
) -> Result<(), String> {
    if let Err(e) = stop_app(app_handle.clone()).await {
        tracing::warn!("切换 ASR 引擎时停止服务失败: {}", e);
    }

    start_app(
        app_handle,
        config.dashscope_api_key.clone(),
        config.siliconflow_api_key.clone(),
        Some(config.use_realtime_asr),
        Some(config.enable_llm_post_process),
        Some(config.enable_dictionary_enhancement),
        Some(config.llm_config.clone()),
        Some(config.smart_command_config.clone()),
        Some(config.asr_config.clone()),
        config.hotkey_config.clone(),
        Some(config.dual_hotkey_config.clone()),
        Some(config.assistant_config.clone()),
        Some(config.enable_mute_other_apps),
        Some(config.dictionary.clone()),
    )
    .await
    .map(|_| ())
}

fn refresh_post_processor_after_toggle(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let enable_post_process = *state.enable_post_process.lock().unwrap();
    let enable_dictionary_enhancement = *state.enable_dictionary_enhancement.lock().unwrap();

    let mut processor_guard = state.post_processor.lock().unwrap();
    if enable_post_process || enable_dictionary_enhancement {
        if processor_guard.is_none() {
            match load_persisted_config() {
                Ok(config) => {
                    let resolved = config.llm_config.resolve_polishing();
                    if !resolved.api_key.trim().is_empty() {
                        *processor_guard = Some(LlmPostProcessor::new(config.llm_config));
                    } else {
                        tracing::warn!(
                            "托盘开启语句润色/词库增强，但 polishing API Key 未配置，将跳过后处理"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("托盘刷新 LLM 后处理器失败: {}", e);
                }
            }
        }
    } else if processor_guard.is_some() {
        *processor_guard = None;
    }
}

fn toggle_post_process_from_tray(
    app_handle: &AppHandle,
    post_process_item: &CheckMenuItem<tauri::Wry>,
) -> Result<(), String> {
    let (updated_config, new_value) = mutate_persisted_config_with_result(|config| {
        let new_value = !config.enable_llm_post_process;
        config.enable_llm_post_process = new_value;
        Ok(new_value)
    })?;

    emit_config_updated(app_handle, &updated_config);

    // 磁盘保存成功后，再更新内存状态
    {
        let state = app_handle.state::<AppState>();
        *state.enable_post_process.lock().unwrap() = new_value;
    }

    post_process_item
        .set_checked(new_value)
        .map_err(|e| format!("更新托盘语句润色勾选状态失败: {}", e))?;

    refresh_post_processor_after_toggle(app_handle);

    tracing::info!("托盘已{}语句润色", if new_value { "开启" } else { "关闭" });
    Ok(())
}

fn toggle_dictionary_enhancement_from_tray(
    app_handle: &AppHandle,
    dictionary_item: &CheckMenuItem<tauri::Wry>,
) -> Result<(), String> {
    let (updated_config, new_value) = mutate_persisted_config_with_result(|config| {
        let new_value = !config.enable_dictionary_enhancement;
        config.enable_dictionary_enhancement = new_value;
        Ok(new_value)
    })?;

    emit_config_updated(app_handle, &updated_config);

    // 磁盘保存成功后，再更新内存状态
    {
        let state = app_handle.state::<AppState>();
        *state.enable_dictionary_enhancement.lock().unwrap() = new_value;
    }

    dictionary_item
        .set_checked(new_value)
        .map_err(|e| format!("更新托盘词库增强勾选状态失败: {}", e))?;

    refresh_post_processor_after_toggle(app_handle);

    tracing::info!("托盘已{}词库增强", if new_value { "开启" } else { "关闭" });
    Ok(())
}

fn toggle_web_search_from_tray(
    app_handle: &AppHandle,
    web_search_item: &CheckMenuItem<tauri::Wry>,
) -> Result<(), String> {
    let (updated_config, new_value) = mutate_persisted_config_with_result(|config| {
        let new_value = !config.assistant_config.enable_web_search;
        config.assistant_config.enable_web_search = new_value;
        Ok(new_value)
    })?;

    emit_config_updated(app_handle, &updated_config);

    {
        let state = app_handle.state::<AppState>();
        let mut processor_guard = state.assistant.processor.lock().unwrap();
        if updated_config
            .assistant_config
            .is_valid_with_shared(&updated_config.llm_config.shared)
        {
            *processor_guard = Some(AssistantProcessor::new(
                updated_config.assistant_config.clone(),
                &updated_config.llm_config.shared,
            ));
        } else {
            *processor_guard = None;
        }
    }

    web_search_item
        .set_checked(new_value)
        .map_err(|e| format!("更新托盘联网搜索勾选状态失败: {}", e))?;

    tracing::info!("托盘已{}联网搜索", if new_value { "开启" } else { "关闭" });
    Ok(())
}

async fn switch_asr_provider_from_tray(
    app_handle: AppHandle,
    target_provider: config::AsrProvider,
    qwen_item: CheckMenuItem<tauri::Wry>,
    doubao_item: CheckMenuItem<tauri::Wry>,
    doubao_ime_item: CheckMenuItem<tauri::Wry>,
) -> Result<(), String> {
    // 并发互斥：防止快速连续点击导致多个 restart 并行执行
    if TRAY_ASR_SWITCHING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!("ASR 引擎切换正在进行中，忽略重复请求");
        return Ok(());
    }
    let result = switch_asr_provider_from_tray_inner(
        &app_handle,
        target_provider,
        &qwen_item,
        &doubao_item,
        &doubao_ime_item,
    )
    .await;
    TRAY_ASR_SWITCHING.store(false, Ordering::SeqCst);
    result
}

async fn switch_asr_provider_from_tray_inner(
    app_handle: &AppHandle,
    target_provider: config::AsrProvider,
    qwen_item: &CheckMenuItem<tauri::Wry>,
    doubao_item: &CheckMenuItem<tauri::Wry>,
    doubao_ime_item: &CheckMenuItem<tauri::Wry>,
) -> Result<(), String> {
    let change = mutate_persisted_config_with_result(|config| {
        if !is_asr_provider_configured(config, &target_provider) {
            return Err(format!(
                "{} 未配置凭证，无法切换",
                asr_provider_name(&target_provider)
            ));
        }
        let changed = config.asr_config.selection.active_provider != target_provider;
        config.asr_config.selection.active_provider = target_provider.clone();
        Ok(changed)
    });
    let (config, changed) = match change {
        Ok(result) => result,
        Err(error) => {
            if let Ok(config) = load_persisted_config() {
                sync_asr_provider_checks(
                    qwen_item,
                    doubao_item,
                    doubao_ime_item,
                    &config.asr_config.selection.active_provider,
                );
            }
            return Err(error);
        }
    };
    if !changed {
        sync_asr_provider_checks(qwen_item, doubao_item, doubao_ime_item, &target_provider);
        return Ok(());
    }

    emit_config_updated(app_handle, &config);

    {
        let state = app_handle.state::<AppState>();
        *state.realtime_provider.lock().unwrap() = Some(target_provider.clone());
    }

    sync_asr_provider_checks(qwen_item, doubao_item, doubao_ime_item, &target_provider);

    let is_running = {
        let state = app_handle.state::<AppState>();
        let running = *state.is_running.lock().unwrap();
        running
    };

    if is_running {
        restart_service_with_config(app_handle.clone(), config).await?;
    }

    tracing::info!(
        "托盘切换 ASR 引擎为: {}",
        asr_provider_name(&target_provider)
    );
    Ok(())
}

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
struct TnlConfigFieldPatch {
    disfluency_mode: Option<crate::tnl::DisfluencyMode>,
    enable_context_hotwords: Option<bool>,
}

impl TnlConfigFieldPatch {
    fn apply(self, config: &mut crate::config::TnlConfig) {
        if let Some(mode) = self.disfluency_mode {
            config.disfluency_mode = mode;
        }
        if let Some(enabled) = self.enable_context_hotwords {
            config.enable_context_hotwords = enabled;
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
    tnl_config: Option<TnlConfigFieldPatch>,
}

#[cfg(test)]
mod config_field_patch_tests {
    use super::*;

    #[test]
    fn context_hotword_patch_round_trips_explicit_false_and_preserves_other_settings() {
        let mut config = crate::config::TnlConfig::default();
        config.disfluency_mode = crate::tnl::DisfluencyMode::Aggressive;
        config.personalization_apply_threshold = 0.97;
        for enabled in [true, false] {
            let patch: TnlConfigFieldPatch =
                serde_json::from_value(serde_json::json!({"enableContextHotwords": enabled}))
                    .unwrap();
            patch.apply(&mut config);
            let config = serde_json::from_value::<crate::config::TnlConfig>(
                serde_json::to_value(&config).unwrap(),
            )
            .unwrap();
            assert_eq!(config.enable_context_hotwords, enabled);
            assert_eq!(
                config.disfluency_mode,
                crate::tnl::DisfluencyMode::Aggressive
            );
            assert_eq!(config.personalization_apply_threshold, 0.97);
        }
    }

    #[test]
    fn should_deserialize_tnl_disfluency_mode_patch() {
        let patch: ConfigFieldPatch = serde_json::from_value(serde_json::json!({
            "tnlConfig": {
                "disfluencyMode": "aggressive"
            }
        }))
        .expect("tnl config patch should deserialize");

        assert_eq!(
            patch
                .tnl_config
                .expect("tnl patch")
                .disfluency_mode
                .expect("disfluency mode"),
            crate::tnl::DisfluencyMode::Aggressive
        );
    }
}

// Tauri Commands
#[tauri::command]
fn get_platform_status() -> platform::PlatformStatus {
    platform::desktop().status()
}

#[tauri::command]
fn request_platform_permission(permission: String) -> Result<(), String> {
    platform::desktop()
        .request_permission(&permission)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_config(
    app: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    enable_dictionary_enhancement: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    smart_command_config: Option<config::SmartCommandConfig>,
    close_action: Option<String>,
    asr_config: Option<config::AsrConfig>,
    hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
    search_config: Option<config::SearchConfig>,
    learning_config: Option<config::LearningConfig>,
    enable_mute_other_apps: Option<bool>,
    dictionary: Option<Vec<String>>,
    builtin_dictionary_domains: Option<Vec<String>>,
    theme: Option<String>,
) -> Result<String, String> {
    let should_sync_user_terms_sidecar = dictionary.is_some();
    let config = mutate_persisted_config_with_result(|existing| {
        tracing::info!("保存配置...");

        // 智能合并 llm_config：如果传入的 presets 为空，保留旧值
        let final_llm_config = match llm_config {
            Some(mut cfg) if cfg.presets.is_empty() && !existing.llm_config.presets.is_empty() => {
                tracing::warn!("检测到空 presets，保留旧配置");
                cfg.presets = existing.llm_config.presets.clone();
                cfg.active_preset_id = existing.llm_config.active_preset_id.clone();
                cfg
            }
            Some(cfg) => cfg,
            None => existing.llm_config.clone(),
        };

        // 智能合并 assistant_config：如果传入的配置无效，保留旧值
        let final_assistant_config = match assistant_config {
            Some(cfg)
                if !cfg.is_valid_with_shared(&final_llm_config.shared)
                    && existing
                        .assistant_config
                        .is_valid_with_shared(&final_llm_config.shared) =>
            {
                tracing::warn!("检测到无效 assistant_config，保留旧配置");
                existing.assistant_config.clone()
            }
            Some(cfg) => cfg,
            None => existing.assistant_config.clone(),
        };

        // 智能合并 dictionary：如果传入空数组，保留旧值
        let final_dictionary = match dictionary {
            Some(dict) if dict.is_empty() && !existing.dictionary.is_empty() => {
                tracing::warn!("检测到空 dictionary，保留旧配置");
                existing.dictionary.clone()
            }
            Some(dict) => {
                // 前端传入的格式：纯词汇 "word" 或带来源 "word|auto"
                // 直接使用传入的数组，不再合并（前端已经是完整的词典状态）
                normalize_dictionary_for_config_storage(dict)
            }
            None => normalize_dictionary_for_config_storage(existing.dictionary.clone()),
        };

        // 智能合并 dual_hotkey_config：如果传入空 keys，保留旧值
        let final_dual_hotkey_config = match dual_hotkey_config {
            Some(cfg) if cfg.dictation.keys.is_empty() || cfg.assistant.keys.is_empty() => {
                tracing::warn!("检测到空快捷键配置，保留旧配置");
                existing.dual_hotkey_config.clone()
            }
            Some(cfg) => cfg,
            None => existing.dual_hotkey_config.clone(),
        };

        let final_asr_config = merge_asr_config_for_save(
            asr_config,
            &existing.asr_config,
            &api_key,
            &fallback_api_key,
        );

        *existing = AppConfig {
            dashscope_api_key: final_asr_config.credentials.qwen_api_key.clone(),
            siliconflow_api_key: final_asr_config.credentials.sensevoice_api_key.clone(),
            asr_config: final_asr_config,
            use_realtime_asr: use_realtime.unwrap_or(existing.use_realtime_asr),
            enable_llm_post_process: enable_post_process
                .unwrap_or(existing.enable_llm_post_process),
            enable_dictionary_enhancement: enable_dictionary_enhancement
                .unwrap_or(existing.enable_dictionary_enhancement),
            llm_config: final_llm_config,
            smart_command_config: smart_command_config
                .unwrap_or_else(|| existing.smart_command_config.clone()),
            assistant_config: final_assistant_config,
            search_config: search_config.unwrap_or_else(|| existing.search_config.clone()),
            learning_config: learning_config.unwrap_or_else(|| existing.learning_config.clone()),
            tnl_config: existing.tnl_config.clone(),
            close_action: close_action.or_else(|| existing.close_action.clone()),
            hotkey_config: hotkey_config.or_else(|| existing.hotkey_config.clone()),
            dual_hotkey_config: final_dual_hotkey_config,
            transcription_mode: existing.transcription_mode,
            enable_mute_other_apps: enable_mute_other_apps
                .unwrap_or(existing.enable_mute_other_apps),
            dictionary: final_dictionary,
            builtin_dictionary_domains: builtin_dictionary_domains
                .unwrap_or_else(|| existing.builtin_dictionary_domains.clone()),
            theme: theme.unwrap_or_else(|| existing.theme.clone()),
        };

        Ok(())
    })?
    .0;

    if should_sync_user_terms_sidecar {
        sync_user_terms_sidecar_from_dictionary_or_warn(&config.dictionary, "显式配置词典保存");
    }

    emit_config_updated(&app, &config);

    tracing::info!("[save_config] 配置已保存, theme={}", config.theme);

    Ok("配置已保存".to_string())
}

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
            qwen_profile: config::QwenAsrProfile::Qwen3Legacy,
            qwen_models: config::QwenModelSelection::default(),
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

    #[test]
    fn should_backfill_dictionary_before_config_save() {
        let normalized = normalize_dictionary_for_config_storage(vec!["useState|auto".to_string()]);

        assert_eq!(normalized, vec!["useState|auto|code_symbol"]);
    }
}

#[tauri::command]
fn get_config_snapshot() -> Result<config::repository::ConfigSnapshot, String> {
    load_config_snapshot()
}

#[tauri::command]
fn update_config(
    app: AppHandle,
    patch: serde_json::Value,
) -> Result<config::repository::ConfigSnapshot, String> {
    let snapshot = update_config_snapshot(&patch)?;
    emit_config_updated(&app, &snapshot.config);
    Ok(snapshot)
}

#[tauri::command]
async fn load_config() -> Result<AppConfig, String> {
    tracing::info!("加载配置...");
    load_persisted_config()
}

#[tauri::command]
fn get_builtin_domains_raw(state: tauri::State<'_, AppState>) -> String {
    lock_hotwords_or_recover(&state.builtin_hotwords_raw).clone()
}

#[tauri::command]
async fn patch_config_fields(app: AppHandle, patch: ConfigFieldPatch) -> Result<String, String> {
    let updated_config = mutate_persisted_config(|config| {
        if let Some(enabled) = patch.learning_enabled {
            config.learning_config.enabled = enabled;
        }

        if let Some(theme) = patch.theme {
            let theme = theme.trim();
            if matches!(theme, "light" | "dark") {
                config.theme = theme.to_string();
            }
        }

        if let Some(enabled) = patch.enable_mute_other_apps {
            config.enable_mute_other_apps = enabled;
        }

        if let Some(close_action_patch) = patch.close_action {
            match close_action_patch {
                Some(action) => {
                    let action = action.trim();
                    if matches!(action, "close" | "minimize") {
                        config.close_action = Some(action.to_string());
                    }
                }
                None => {
                    config.close_action = None;
                }
            }
        }

        if let Some(tnl_patch) = patch.tnl_config {
            tnl_patch.apply(&mut config.tnl_config);
        }

        Ok(())
    })?;

    emit_config_updated(&app, &updated_config);

    Ok("配置字段已更新".to_string())
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
    doubao_ime_session: Arc<tokio::sync::Mutex<Option<DoubaoImeRealtimeSession>>>,
    doubao_ime_credentials: Arc<Mutex<Option<DoubaoImeCredentials>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    use_realtime: bool,
    api_key: String,
    doubao_app_id: Option<String>,
    doubao_access_token: Option<String>,
    dictionary: Vec<String>,
    correction_pairs: Vec<CorrectionPair>,
    language_mode: config::AsrLanguageMode,
    qwen_model: &'static QwenModel,
) {
    tracing::info!("检测到快捷键按下");

    let _ = app.emit("recording_started", ());

    // 显示录音悬浮窗并移动到鼠标所在屏幕底部居中
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Some(monitor) = find_monitor_at_cursor(&overlay) {
            let monitor_pos = monitor.position();
            let screen_size = monitor.size();
            let scale_factor = monitor.scale_factor();
            let overlay_size = overlay
                .outer_size()
                .unwrap_or(tauri::PhysicalSize::new(120, 44));

            // 全程使用物理像素计算
            let x = monitor_pos.x + (screen_size.width as i32 - overlay_size.width as i32) / 2;
            let y = monitor_pos.y + screen_size.height as i32
                - overlay_size.height as i32
                - (100.0 * scale_factor) as i32;

            let _ = overlay.set_position(tauri::PhysicalPosition::new(x, y));
        }
        let _ = overlay.show();
    }

    if use_realtime {
        let provider = realtime_provider.lock().unwrap().clone();
        match provider {
            Some(config::AsrProvider::Doubao) => {
                handle_doubao_realtime_start(
                    app,
                    streaming_recorder,
                    doubao_session,
                    audio_sender_handle,
                    doubao_app_id,
                    doubao_access_token,
                    dictionary,
                    correction_pairs,
                    language_mode,
                )
                .await;
            }
            Some(config::AsrProvider::DoubaoIme) => {
                handle_doubao_ime_realtime_start(
                    app,
                    streaming_recorder,
                    doubao_ime_session,
                    audio_sender_handle,
                    doubao_ime_credentials,
                    dictionary,
                )
                .await;
            }
            _ => {
                handle_qwen_realtime_start(
                    app,
                    streaming_recorder,
                    active_session,
                    audio_sender_handle,
                    api_key,
                    dictionary,
                    correction_pairs,
                    language_mode,
                    qwen_model,
                )
                .await;
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
    correction_pairs: Vec<CorrectionPair>,
    language_mode: config::AsrLanguageMode,
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
        if let (Some(app_id), Some(access_token)) =
            (doubao_app_id.as_ref(), doubao_access_token.as_ref())
        {
            let realtime_client = DoubaoRealtimeClient::new_with_correction_pairs(
                app_id.clone(),
                access_token.clone(),
                dictionary,
                correction_pairs,
                language_mode,
            );
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
                    tracing::error!(
                        "建立豆包 WebSocket 连接失败: {}，录音已启动，将使用备用方案",
                        e
                    );
                }
            }
        } else {
            tracing::error!("豆包凭证缺失：需要 app_id 和 access_token");
        }
    }
}

async fn handle_doubao_ime_realtime_start(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    doubao_ime_session: Arc<tokio::sync::Mutex<Option<DoubaoImeRealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    doubao_ime_credentials: Arc<Mutex<Option<DoubaoImeCredentials>>>,
    _dictionary: Vec<String>,
) {
    tracing::info!("启动豆包输入法实时流式转录...");

    let chunk_rx = {
        let mut streaming_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *streaming_guard {
            if rec.is_recording() {
                tracing::warn!("发现正在进行中的流式录音，先停止它");
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
        // 检查是否有已保存的凭据
        let saved_credentials = doubao_ime_credentials.lock().unwrap().clone();
        let had_credentials = saved_credentials.is_some();

        let mut realtime_client = if let Some(creds) = saved_credentials {
            tracing::info!(
                "豆包输入法 ASR: 使用已保存的凭据 (device_id={})",
                creds.device_id
            );
            DoubaoImeRealtimeClient::with_credentials(
                reqwest::Client::new(),
                asr::DoubaoImeClientConfig::default(),
                creds,
            )
        } else {
            tracing::info!("豆包输入法 ASR: 无已保存凭据，将自动注册");
            DoubaoImeRealtimeClient::new(
                reqwest::Client::new(),
                asr::DoubaoImeClientConfig::default(),
            )
        };

        {
            let mut session_guard = doubao_ime_session.lock().await;
            if let Some(mut old_session) = session_guard.take() {
                tracing::warn!("发现旧的豆包输入法会话，先关闭它");
                let _ = old_session.finish_audio().await;
            }
        }
        {
            if let Some(old_handle) = audio_sender_handle.lock().unwrap().take() {
                tracing::warn!("发现旧的音频发送任务，先取消它");
                old_handle.abort();
            }
        }

        let mut session_result = realtime_client.start_session().await;
        if session_result.is_err() && had_credentials {
            if let Some(err_text) = session_result.as_ref().err().map(|e| e.to_string()) {
                let normalized = err_text.to_lowercase();
                let should_refresh_credentials = normalized.contains("taskfailed")
                    || normalized.contains("sessionfailed")
                    || normalized.contains("token")
                    || normalized.contains("auth")
                    || normalized.contains("401")
                    || normalized.contains("403");

                if should_refresh_credentials {
                    tracing::warn!(
                        "豆包输入法 ASR: 现有凭据可能失效，清除后重试。原始错误: {}",
                        err_text
                    );
                    *doubao_ime_credentials.lock().unwrap() = None;
                    if let Err(e) = clear_doubao_ime_credentials_from_config(&app).await {
                        tracing::error!("清除豆包输入法配置凭据失败: {}", e);
                    }

                    realtime_client = DoubaoImeRealtimeClient::new(
                        reqwest::Client::new(),
                        asr::DoubaoImeClientConfig::default(),
                    );
                    session_result = realtime_client.start_session().await;
                }
            }
        }

        match session_result {
            Ok(session) => {
                tracing::info!("豆包输入法 WebSocket 连接已建立");

                if let Some(new_creds) = realtime_client.credentials() {
                    let should_save = !had_credentials
                        || doubao_ime_credentials
                            .lock()
                            .unwrap()
                            .as_ref()
                            .map(|old| {
                                old.device_id != new_creds.device_id
                                    || old.token != new_creds.token
                                    || old.cdid != new_creds.cdid
                            })
                            .unwrap_or(true);

                    if should_save {
                        tracing::info!(
                            "豆包输入法 ASR: 更新凭据缓存 (device_id={})",
                            new_creds.device_id
                        );
                        *doubao_ime_credentials.lock().unwrap() = Some(new_creds.clone());
                        if let Err(e) =
                            save_doubao_ime_credentials_to_config(&app, new_creds.clone()).await
                        {
                            tracing::error!("保存豆包输入法凭据到配置文件失败: {}", e);
                        }
                    }
                }

                *doubao_ime_session.lock().await = Some(session);

                let session_for_sender = Arc::clone(&doubao_ime_session);
                let sender_handle = tokio::spawn(async move {
                    tracing::info!("豆包输入法音频发送任务启动");
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

                    tracing::info!("豆包输入法音频发送任务结束，共发送 {} 个块", chunk_count);
                });

                *audio_sender_handle.lock().unwrap() = Some(sender_handle);
            }
            Err(e) => {
                tracing::error!(
                    "建立豆包输入法 WebSocket 连接失败: {}，录音已启动，将使用备用方案",
                    e
                );
            }
        }
    }
}

/// 保存豆包输入法凭据到配置文件
async fn save_doubao_ime_credentials_to_config(
    app: &AppHandle,
    creds: DoubaoImeCredentials,
) -> anyhow::Result<()> {
    let updated_config = mutate_persisted_config(|config| {
        config.asr_config.credentials.doubao_ime_device_id = creds.device_id;
        config.asr_config.credentials.doubao_ime_token = creds.token;
        config.asr_config.credentials.doubao_ime_cdid = creds.cdid;
        Ok(())
    })
    .map_err(anyhow::Error::msg)?;

    emit_config_updated(app, &updated_config);
    tracing::info!("豆包输入法凭据已保存到配置文件");
    Ok(())
}

async fn clear_doubao_ime_credentials_from_config(app: &AppHandle) -> anyhow::Result<()> {
    let updated_config = mutate_persisted_config(|config| {
        config.asr_config.credentials.doubao_ime_device_id.clear();
        config.asr_config.credentials.doubao_ime_token.clear();
        config.asr_config.credentials.doubao_ime_cdid.clear();
        Ok(())
    })
    .map_err(anyhow::Error::msg)?;

    emit_config_updated(app, &updated_config);
    tracing::info!("已清除豆包输入法凭据缓存");
    Ok(())
}

/// 处理千问实时模式启动
async fn handle_qwen_realtime_start(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    api_key: String,
    dictionary: Vec<String>,
    correction_pairs: Vec<CorrectionPair>,
    language_mode: config::AsrLanguageMode,
    qwen_model: &'static QwenModel,
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

    let realtime_client = QwenRealtimeClient::new_with_model_and_correction_pairs(
        api_key,
        dictionary,
        correction_pairs,
        language_mode,
        qwen_model,
    );
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

fn non_empty_selected_text(text: Option<String>) -> Option<String> {
    text.filter(|value| !value.trim().is_empty())
}

fn capture_native_selection(target_hwnd: Option<InputTarget>) -> Option<String> {
    let hwnd = target_hwnd?;
    match platform::desktop().read_selection(hwnd) {
        Ok(text) => {
            let text = non_empty_selected_text(Some(text));
            if let Some(ref text) = text {
                tracing::info!("原生选区读取捕获选中文本: {} 字符", text.len());
            } else {
                tracing::debug!("原生选区读取未检测到选中文本");
            }
            text
        }
        Err(e) => {
            tracing::debug!("原生选区读取捕获选中文本失败: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod selected_text_capture_tests {
    use super::non_empty_selected_text;

    #[test]
    fn non_empty_selected_text_rejects_blank_text() {
        assert_eq!(non_empty_selected_text(None), None);
        assert_eq!(non_empty_selected_text(Some(" \r\n\t ".to_string())), None);
    }

    #[test]
    fn non_empty_selected_text_keeps_original_content() {
        assert_eq!(
            non_empty_selected_text(Some("  class Solution {}\n".to_string())),
            Some("  class Solution {}\n".to_string())
        );
    }
}

#[tauri::command]
async fn start_app(
    app_handle: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    enable_dictionary_enhancement: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    _smart_command_config: Option<config::SmartCommandConfig>,
    asr_config: Option<config::AsrConfig>,
    _hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
    enable_mute_other_apps: Option<bool>,
    dictionary: Option<Vec<String>>,
) -> Result<String, String> {
    if !platform::desktop().status().ready() {
        return Err("请在偏好设置中授权麦克风、辅助功能和输入监控，再启动服务".into());
    }
    if let Some(cfg) = &asr_config {
        cfg.validate_models(use_realtime.unwrap_or(true))
            .map_err(|e| e.to_string())?;
    }
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

    // The provider owns transport capabilities. Legacy use_realtime flags must
    // never route a SenseVoice credential to the Qwen WebSocket implementation.
    let requested_realtime = use_realtime.unwrap_or(true);
    let use_realtime_mode = asr_config.as_ref().map_or(requested_realtime, |cfg| {
        cfg.selection
            .active_provider
            .realtime_enabled(requested_realtime)
    });

    *state.use_realtime_asr.lock().unwrap() = use_realtime_mode;

    // 确定是否启用 LLM 后处理
    let enable_post_process_mode = enable_post_process.unwrap_or(false);
    *state.enable_post_process.lock().unwrap() = enable_post_process_mode;

    // 确定是否启用词库增强（默认启用）
    let enable_dictionary_enhancement_mode = enable_dictionary_enhancement.unwrap_or(true);
    *state.enable_dictionary_enhancement.lock().unwrap() = enable_dictionary_enhancement_mode;

    tracing::info!(
        "ASR 模式: {}",
        if use_realtime_mode {
            "实时 WebSocket"
        } else {
            "HTTP"
        }
    );
    tracing::info!(
        "LLM 后处理: {}",
        if enable_post_process_mode {
            "启用"
        } else {
            "禁用"
        }
    );
    tracing::info!(
        "词库增强: {}",
        if enable_dictionary_enhancement_mode {
            "启用"
        } else {
            "禁用"
        }
    );

    let input_dictionary = dictionary.unwrap_or_default();
    let dict = runtime_dictionary_entries_from_user_terms_or_input(&input_dictionary);
    tracing::info!("词库: {} 个词", dict.len());

    // 保存词库到 state（用于 Realtime 模式热更新）
    *state.dictionary.lock().unwrap() = dict.clone();
    let correction_pairs = refresh_asr_correction_pairs_runtime(&state);

    // 根据 asr_config 初始化 ASR 客户端
    {
        *state.qwen_client.lock().unwrap() = None;
        *state.sensevoice_client.lock().unwrap() = None;
        *state.doubao_client.lock().unwrap() = None;

        if let Some(ref cfg) = asr_config {
            // 初始化所有有凭证的客户端
            if !cfg.credentials.qwen_api_key.is_empty() {
                // An unsupported saved HTTP model must not block an unrelated provider
                // or an explicitly selected realtime model. Used paths were validated above.
                if let Ok(model) = cfg.qwen_model(QwenMode::Http) {
                    *state.qwen_client.lock().unwrap() =
                        Some(QwenASRClient::new_with_model_and_correction_pairs(
                            cfg.credentials.qwen_api_key.clone(),
                            dict.clone(),
                            correction_pairs.clone(),
                            cfg.language_mode,
                            model,
                        ));
                }
            }
            if !cfg.credentials.sensevoice_api_key.is_empty() {
                *state.sensevoice_client.lock().unwrap() = Some(SenseVoiceClient::new(
                    cfg.credentials.sensevoice_api_key.clone(),
                ));
            }
            if !cfg.credentials.doubao_app_id.is_empty()
                && !cfg.credentials.doubao_access_token.is_empty()
            {
                *state.doubao_client.lock().unwrap() =
                    Some(DoubaoASRClient::new_with_correction_pairs(
                        cfg.credentials.doubao_app_id.clone(),
                        cfg.credentials.doubao_access_token.clone(),
                        dict.clone(),
                        correction_pairs.clone(),
                        cfg.language_mode,
                    ));
            }

            // 设置实时转录提供商
            *state.realtime_provider.lock().unwrap() = Some(cfg.selection.active_provider.clone());
            *state.fallback_provider.lock().unwrap() = cfg.selection.fallback_provider.clone();
        } else {
            // 旧逻辑回退（基本不会走到这里）
            if !api_key.is_empty() {
                *state.qwen_client.lock().unwrap() =
                    Some(QwenASRClient::new_with_correction_pairs(
                        api_key.clone(),
                        dict.clone(),
                        correction_pairs.clone(),
                        config::AsrLanguageMode::Auto,
                    ));
            }
            if !fallback_api_key.is_empty() {
                *state.sensevoice_client.lock().unwrap() =
                    Some(SenseVoiceClient::new(fallback_api_key.clone()));
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
        let should_enable_post_processing =
            enable_post_process_mode || enable_dictionary_enhancement_mode;
        tracing::info!(
            "[DEBUG] LLM 后处理配置: post_process={}, dictionary_enhancement={}, api_key_len={}",
            enable_post_process_mode,
            enable_dictionary_enhancement_mode,
            resolved.api_key.len()
        );
        if should_enable_post_processing && !resolved.api_key.trim().is_empty() {
            tracing::info!(
                "LLM 后处理器配置: endpoint={}, model={}",
                resolved.endpoint,
                resolved.model
            );
            *processor_guard = Some(LlmPostProcessor::new(llm_cfg));
            tracing::info!("LLM 后处理器已初始化");
        } else {
            *processor_guard = None;
            if should_enable_post_processing {
                tracing::warn!(
                    "LLM 后处理已启用（语句润色或词库增强）但未配置 API Key，将跳过后处理"
                );
            }
        }
    }

    // 初始化 AI 助手处理器（独立配置，支持双系统提示词，永远开启只需检查配置有效性）
    tracing::info!("[DEBUG] 初始化 AI 助手处理器...");
    {
        let mut processor_guard = state.assistant.processor.lock().unwrap();
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
    let text_inserter = TextInserter::new().map_err(|e| format!("初始化文本插入器失败: {}", e))?;
    *state.text_inserter.lock().unwrap() = Some(text_inserter);
    tracing::info!("[DEBUG] 文本插入器初始化完成");

    // 初始化或更新音频静音管理器
    {
        let should_mute = enable_mute_other_apps.unwrap_or(false);
        let mut manager_lock = state.recording.audio_mute_manager.lock().unwrap();
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
    *state.recording.audio_recorder.lock().unwrap() = None;
    *state.recording.streaming_recorder.lock().unwrap() = None;

    if use_realtime_mode {
        let streaming_recorder =
            StreamingRecorder::new().map_err(|e| format!("初始化流式录音器失败: {}", e))?;
        *state.recording.streaming_recorder.lock().unwrap() = Some(streaming_recorder);
    } else {
        let audio_recorder =
            AudioRecorder::new().map_err(|e| format!("初始化音频录制器失败: {}", e))?;
        *state.recording.audio_recorder.lock().unwrap() = Some(audio_recorder);
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
    dual_hotkey_cfg
        .validate()
        .map_err(|e| format!("热键配置无效: {}", e))?;
    tracing::info!("[DEBUG] 热键配置验证通过");

    let hotkey_service = Arc::clone(&state.hotkey_service);

    // 克隆状态用于回调（听写模式）
    let app_handle_start = app_handle.clone();
    let audio_recorder_start = Arc::clone(&state.recording.audio_recorder);
    let streaming_recorder_start = Arc::clone(&state.recording.streaming_recorder);
    let active_session_start = Arc::clone(&state.recording.active_session);
    let doubao_session_start = Arc::clone(&state.recording.doubao_session);
    let doubao_ime_session_start = Arc::clone(&state.recording.doubao_ime_session);
    let doubao_ime_credentials_start = Arc::clone(&state.doubao_ime_credentials);
    let realtime_provider_start = Arc::clone(&state.realtime_provider);
    let audio_sender_handle_start = Arc::clone(&state.recording.audio_sender_handle);
    let use_realtime_start = use_realtime_mode;
    let dictionary_state_start = Arc::clone(&state.dictionary);
    let asr_correction_pairs_start = Arc::clone(&state.asr_correction_pairs);
    let is_running_start = Arc::clone(&state.is_running);
    // AI 助手模式专用
    let current_trigger_mode_start = Arc::clone(&state.recording.current_trigger_mode);
    // 统计数据相关
    let recording_start_instant_start = Arc::clone(&state.recording.recording_start_instant);

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
            config::AsrProvider::DoubaoIme => {
                // 豆包输入法模式：加载已保存的凭据（如果有的话）
                if !cfg.credentials.doubao_ime_device_id.is_empty()
                    && !cfg.credentials.doubao_ime_token.is_empty()
                {
                    let saved_creds = DoubaoImeCredentials {
                        device_id: cfg.credentials.doubao_ime_device_id.clone(),
                        token: cfg.credentials.doubao_ime_token.clone(),
                        cdid: cfg.credentials.doubao_ime_cdid.clone(),
                        ..Default::default()
                    };
                    *state.doubao_ime_credentials.lock().unwrap() = Some(saved_creds);
                    tracing::info!("已加载保存的豆包输入法凭据");
                }
                (String::new(), None, None)
            }
            config::AsrProvider::SiliconFlow => {
                (cfg.credentials.sensevoice_api_key.clone(), None, None)
            }
        }
    } else {
        (String::new(), None, None)
    };
    let api_key_start = asr_api_key.clone();
    let doubao_app_id_start = doubao_app_id;
    let doubao_access_token_start = doubao_access_token;
    let asr_language_mode_start = asr_config
        .as_ref()
        .map(|cfg| cfg.language_mode)
        .unwrap_or(config::AsrLanguageMode::Auto);
    let qwen_model_start = if use_realtime_mode
        && asr_config
            .as_ref()
            .is_some_and(|cfg| cfg.selection.active_provider == config::AsrProvider::Qwen)
    {
        asr_config
            .as_ref()
            .unwrap()
            .qwen_model(QwenMode::Realtime)
            .map_err(|e| e.to_string())?
    } else {
        // Unused by non-Qwen or HTTP recording paths; keeps legacy callers compatible.
        profile_model(config::QwenAsrProfile::Qwen3Legacy, QwenMode::Realtime)
    };

    let app_handle_stop = app_handle.clone();
    let audio_recorder_stop = Arc::clone(&state.recording.audio_recorder);
    let streaming_recorder_stop = Arc::clone(&state.recording.streaming_recorder);
    let active_session_stop = Arc::clone(&state.recording.active_session);
    let audio_sender_handle_stop = Arc::clone(&state.recording.audio_sender_handle);
    let post_processor_stop = Arc::clone(&state.post_processor);
    let assistant_processor_stop = Arc::clone(&state.assistant.processor);
    let text_inserter_stop = Arc::clone(&state.text_inserter);
    let qwen_client_stop = Arc::clone(&state.qwen_client);
    let sensevoice_client_stop = Arc::clone(&state.sensevoice_client);
    let doubao_client_stop = Arc::clone(&state.doubao_client);
    let doubao_session_stop = Arc::clone(&state.recording.doubao_session);
    let doubao_ime_session_stop = Arc::clone(&state.recording.doubao_ime_session);
    let realtime_provider_stop = Arc::clone(&state.realtime_provider);
    let use_realtime_stop = use_realtime_mode;
    let is_running_stop = Arc::clone(&state.is_running);
    let enable_fallback_stop = Arc::clone(&state.enable_fallback);

    // AI 助手处理中标记（用于 on_start 防重复触发）
    let is_assistant_processing_start = Arc::clone(&state.assistant.processing);

    // 松手模式相关变量（用于 on_start）
    let is_recording_locked_start = Arc::clone(&state.recording.is_recording_locked);
    let _dual_hotkey_cfg_start = dual_hotkey_cfg.clone();

    // 松手模式相关变量（用于 on_stop）
    let is_recording_locked_stop = Arc::clone(&state.recording.is_recording_locked);

    // 音频静音管理器（用于 on_start 和 on_stop）

    // 目标窗口句柄（用于焦点恢复）
    let target_window_start = Arc::clone(&state.recording.target_window);
    let target_window_stop = Arc::clone(&state.recording.target_window);
    let assistant_selected_text_snapshot = Arc::new(Mutex::new(None::<String>));
    let assistant_selected_text_snapshot_start = Arc::clone(&assistant_selected_text_snapshot);
    let assistant_selected_text_snapshot_stop = Arc::clone(&assistant_selected_text_snapshot);

    // 统计数据相关（用于 on_stop）
    let usage_stats_stop = Arc::clone(&state.usage_stats);
    let recording_start_instant_stop = Arc::clone(&state.recording.recording_start_instant);

    let recording_session_start = state.recording_session.clone();
    let recording_session_stop = state.recording_session.clone();
    let recording_resources_start = state.recording.clone();
    let recording_resources_stop = state.recording.clone();

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

        // === AI 助手处理中阻止重复触发（R8）===
        if trigger_mode == config::TriggerMode::AiAssistant
            && is_assistant_processing_start.load(Ordering::SeqCst)
        {
            tracing::info!("AI 助手正在处理中，忽略重复触发");
            return;
        }

        let accepted = recording_session_start.start(|| {
            // === 保存目标窗口句柄（通过防重入检查后才保存） ===
            // 这是用户触发热键时的前台窗口，用于后续焦点恢复
            let target_hwnd = platform::desktop().capture_target();
            *target_window_start.lock().unwrap() = target_hwnd;
            *assistant_selected_text_snapshot_start.lock().unwrap() = None;
            if let Some(hwnd) = target_hwnd {
                tracing::info!("已保存目标输入位置: {}", hwnd);
            } else {
                tracing::warn!("未能获取目标窗口句柄");
            }

            // 保存当前触发模式
            *current_trigger_mode_start.lock().unwrap() = Some(trigger_mode);
            let mode_desc = if is_release_mode {
                "松手模式"
            } else {
                "普通模式"
            };
            tracing::info!("触发模式: {:?} ({})", trigger_mode, mode_desc);

            // 注意：剪贴板捕获已移至 on_stop 回调
            // 原因：在 on_start 时物理按键仍被按住，模拟 Ctrl+C 会与 Alt/Meta 等修饰键冲突

            beep_player::play_start_beep();

            let app = app_handle_start.clone();
            let recorder = Arc::clone(&audio_recorder_start);
            let streaming_recorder = Arc::clone(&streaming_recorder_start);
            let active_session = Arc::clone(&active_session_start);
            let doubao_session = Arc::clone(&doubao_session_start);
            let doubao_ime_session = Arc::clone(&doubao_ime_session_start);
            let doubao_ime_credentials = Arc::clone(&doubao_ime_credentials_start);
            let realtime_provider = Arc::clone(&realtime_provider_start);
            let audio_sender_handle = Arc::clone(&audio_sender_handle_start);
            let use_realtime = use_realtime_start;
            let api_key = api_key_start.clone();
            let doubao_app_id = doubao_app_id_start.clone();
            let doubao_access_token = doubao_access_token_start.clone();
            let language_mode = asr_language_mode_start;
            let qwen_model = qwen_model_start;
            let is_recording_locked_spawn = Arc::clone(&is_recording_locked_start);
            let dictionary_state = Arc::clone(&dictionary_state_start);
            let asr_correction_pairs = Arc::clone(&asr_correction_pairs_start);
            let recording_start_instant_spawn = Arc::clone(&recording_start_instant_start);
            let selected_text_snapshot = Arc::clone(&assistant_selected_text_snapshot_start);

            let resources = recording_resources_start.clone();
            let cleanup_resources = recording_resources_start.clone();
            let cleanup_app = app.clone();
            let assistant_busy = is_assistant_processing_start.clone();
            is_recording_locked_spawn.store(
                is_release_mode && trigger_mode == config::TriggerMode::Dictation,
                Ordering::SeqCst,
            );
            (
                async move {
                    let settings = match load_persisted_config() {
                        Ok(settings) => settings,
                        Err(error) => {
                            emit_error_and_hide_overlay(&app, error.clone());
                            return Err(error);
                        }
                    };
                    let context_hotwords_enabled = settings.tnl_config.enable_context_hotwords;
                    *resources.settings.lock().unwrap() = Some(settings);
                    resources.begin_audio();

                    // 记录录音开始时间（包含录音准备时间：静音、显示窗口等）
                    // 注意：这个时间略早于实际音频采集开始，但包含了用户感知到的准备时间
                    *recording_start_instant_spawn.lock().unwrap() =
                        Some(std::time::Instant::now());

                    if trigger_mode == config::TriggerMode::AiAssistant {
                        if let Some(hwnd) = target_hwnd {
                            let selection_read_start = std::time::Instant::now();
                            match tokio::task::spawn_blocking(move || {
                                platform::desktop().read_selection(hwnd)
                            })
                            .await
                            {
                                Ok(Ok(text)) => {
                                    if let Some(text) = non_empty_selected_text(Some(text)) {
                                        tracing::info!(
                                            "AI 助手预捕获选中文本: {} 字符（原生读取 {}ms）",
                                            text.len(),
                                            selection_read_start.elapsed().as_millis()
                                        );
                                        *selected_text_snapshot.lock().unwrap() = Some(text);
                                    } else {
                                        tracing::debug!("AI 助手预捕获未检测到选中文本");
                                    }
                                }
                                Ok(Err(e)) => {
                                    tracing::debug!("AI 助手预捕获选中文本失败: {}", e);
                                }
                                Err(e) => {
                                    tracing::warn!("AI 助手预捕获选中文本任务异常: {}", e);
                                }
                            }
                        }
                    }

                    // 从 state 获取最新词库（支持热更新），并为本次录音追加临时上下文热词。
                    let mut dictionary = dictionary_state.lock().unwrap().clone();
                    if let Some(hwnd) = target_hwnd.filter(|_| context_hotwords_enabled) {
                        let context_read_start = std::time::Instant::now();
                        match tokio::task::spawn_blocking(move || {
                            platform::desktop().read_text(hwnd)
                        })
                        .await
                        {
                            Ok(Ok(context_text)) if !context_text.trim().is_empty() => {
                                let before_len = dictionary.len();
                                dictionary =
                                    personalization::augment_dictionary_with_app_context_hotwords(
                                        dictionary,
                                        &context_text,
                                    );
                                let added_count = dictionary.len().saturating_sub(before_len);
                                if added_count > 0 {
                                    tracing::debug!(
                                        "已追加当前 App 上下文 ASR 热词: {}（原生读取 {}ms）",
                                        added_count,
                                        context_read_start.elapsed().as_millis()
                                    );
                                }
                            }
                            Ok(Ok(_)) => {
                                tracing::debug!("当前 App 上下文为空，跳过临时 ASR 热词追加");
                            }
                            Ok(Err(e)) => {
                                tracing::debug!(
                                    "读取当前 App 上下文失败，跳过临时 ASR 热词追加: {}",
                                    e
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "当前 App 上下文读取任务异常，跳过临时 ASR 热词追加: {}",
                                    e
                                );
                            }
                        }
                    }
                    let correction_pairs = asr_correction_pairs.lock().unwrap().clone();
                    // 1. 先执行开始录音逻辑 (内部会发送 recording_started 事件)
                    handle_recording_start(
                        app.clone(),
                        recorder,
                        streaming_recorder,
                        active_session,
                        doubao_session,
                        doubao_ime_session,
                        doubao_ime_credentials,
                        realtime_provider,
                        audio_sender_handle,
                        use_realtime,
                        api_key,
                        doubao_app_id,
                        doubao_access_token,
                        dictionary,
                        correction_pairs,
                        language_mode,
                        qwen_model,
                    )
                    .await;

                    if !resources.is_recording(use_realtime) {
                        return Err("麦克风未成功启动".into());
                    }
                    if is_recording_locked_spawn.load(Ordering::SeqCst) {
                        let _ = app.emit("recording_locked", ());
                    }
                    Ok(())
                },
                async move {
                    cleanup_resources.cleanup().await;
                    if trigger_mode == config::TriggerMode::AiAssistant
                        && assistant_busy.load(Ordering::SeqCst)
                    {
                        let state = cleanup_app.state::<AppState>();
                        let _ = cancel_assistant_generation(cleanup_app.clone(), state).await;
                    }
                    if let Some(overlay) = cleanup_app.get_webview_window("overlay") {
                        let _ = overlay.hide();
                    }
                },
            )
        });
        if !accepted {
            tracing::debug!("上一轮录音仍在处理或收尾，忽略重复触发");
        }
        #[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
        if accepted {
            let session = recording_session_start.clone();
            atdd::track_start_task(tauri::async_runtime::spawn(async move {
                let _ = session.wait_started().await;
            }));
        }
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
            // 不 return，继续向下执行正常的停止录音和转写流程
        }

        // === 松手模式：检查锁定状态 ===
        if is_recording_locked_stop.load(Ordering::SeqCst) {
            tracing::info!("录音已锁定（松手模式），忽略物理按键释放");
            return; // 不停止录音，等待用户点击悬浮窗按钮
        }

        tracing::info!("检测到快捷键释放，模式: {:?}", trigger_mode);

        let app = app_handle_stop.clone();
        let recorder = Arc::clone(&audio_recorder_stop);
        let streaming_recorder = Arc::clone(&streaming_recorder_stop);
        let active_session = Arc::clone(&active_session_stop);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_stop);
        let qwen_client_state = Arc::clone(&qwen_client_stop);
        let sensevoice_client_state = Arc::clone(&sensevoice_client_stop);
        let doubao_client_state = Arc::clone(&doubao_client_stop);
        let doubao_session_state = Arc::clone(&doubao_session_stop);
        let doubao_ime_session_state = Arc::clone(&doubao_ime_session_stop);
        let realtime_provider_state = Arc::clone(&realtime_provider_stop);
        let enable_fallback_state = Arc::clone(&enable_fallback_stop);
        let use_realtime = use_realtime_stop;

        // 根据触发模式选择处理器
        let post_processor = Arc::clone(&post_processor_stop);
        let assistant_processor = Arc::clone(&assistant_processor_stop);
        let text_inserter = Arc::clone(&text_inserter_stop);

        // 获取目标窗口句柄（用于焦点恢复）
        let target_hwnd = *target_window_stop.lock().unwrap();
        let selection_snapshot = assistant_selected_text_snapshot_stop.clone();
        let resources = recording_resources_stop.clone();

        // 统计数据相关
        let usage_stats = Arc::clone(&usage_stats_stop);
        let recording_start_instant = Arc::clone(&recording_start_instant_stop);

        // 播放停止录音提示音
        beep_player::play_stop_beep();

        recording_session_stop.finish(async move {
            resources.is_recording_locked.store(false, Ordering::SeqCst);
            resources.restore_audio();
            let pre_captured_selected_text =
                non_empty_selected_text(selection_snapshot.lock().unwrap().take());
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
                            doubao_ime_session_state,
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
                        )
                        .await;
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
                        )
                        .await;
                    }
                }
                config::TriggerMode::AiAssistant => {
                    // AI 助手模式：多轮对话处理
                    tracing::info!("使用 AI 助手模式处理");

                    // 等待物理按键完全释放后再捕获剪贴板
                    // 原因：在 on_start 时物理按键仍被按住，模拟 Ctrl+C 会与 Alt/Meta 等修饰键冲突
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    // 捕获选中文本（此时用户已松开热键，Ctrl+C 模拟安全）
                    // 剪贴板即时释放：ClipboardGuard 在此 scope 结束时 drop，立即恢复用户剪贴板
                    let selected_text = if let Some(text) = pre_captured_selected_text {
                        tracing::info!("使用 AI 助手预捕获选中文本: {} 字符", text.len());
                        Some(text)
                    } else {
                        tracing::info!("AI 助手模式：开始捕获选中文本...");
                        match clipboard_manager::get_selected_text(target_hwnd) {
                            Ok((guard, text)) => {
                                if let Some(ref t) = text {
                                    tracing::info!("已捕获选中文本: {} 字符", t.len());
                                } else {
                                    tracing::info!("剪贴板未捕获选中文本，尝试 原生选区读取");
                                }
                                // guard 在此 scope 结束时 drop，自动恢复剪贴板
                                drop(guard);
                                text.or_else(|| capture_native_selection(target_hwnd))
                            }
                            Err(e) => {
                                tracing::warn!("剪贴板捕获选中文本失败: {}，尝试 原生选区读取", e);
                                capture_native_selection(target_hwnd)
                            }
                        }
                    };
                    if selected_text.is_none() {
                        tracing::info!("无选中文本，将使用问答模式");
                    }

                    #[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
                    atdd::observe_selection(selected_text.as_deref());

                    handle_assistant_mode(
                        app,
                        recorder,
                        streaming_recorder,
                        active_session,
                        doubao_session_state,
                        doubao_ime_session_state,
                        realtime_provider_state,
                        audio_sender_handle,
                        assistant_processor,
                        selected_text,
                        qwen_client_state,
                        sensevoice_client_state,
                        doubao_client_state,
                        enable_fallback_state,
                        use_realtime,
                        target_hwnd,
                        usage_stats.clone(),
                        recording_start_instant.clone(),
                    )
                    .await;
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
    let mode_str = if use_realtime_mode {
        "实时模式"
    } else {
        "HTTP 模式"
    };
    let dictation_display = dual_hotkey_cfg.dictation.format_display();
    let assistant_display = dual_hotkey_cfg.assistant.format_display();
    Ok(format!(
        "应用已启动 ({})，听写: {}，AI助手: {}",
        mode_str, dictation_display, assistant_display
    ))
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

    *state.is_running.lock().unwrap() = false;
    state.recording_session.cancel().await;

    *state.recording.audio_recorder.lock().unwrap() = None;
    *state.recording.streaming_recorder.lock().unwrap() = None;
    *state.text_inserter.lock().unwrap() = None;
    *state.post_processor.lock().unwrap() = None;
    *state.assistant.processor.lock().unwrap() = None;
    *state.qwen_client.lock().unwrap() = None;
    *state.sensevoice_client.lock().unwrap() = None;
    *state.doubao_client.lock().unwrap() = None;

    // 清理 AI 助手会话状态并隐藏结果面板
    state.assistant.conversation.lock().unwrap().take();
    state.assistant.processing.store(false, Ordering::SeqCst);
    hide_result_panel_window(&app_handle).await;

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
    let running = *app_handle.state::<AppState>().is_running.lock().unwrap();
    if running {
        stop_app(app_handle.clone()).await?;
    }
    app_handle.exit(0);
    Ok(())
}

#[tauri::command]
async fn cancel_transcription(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("取消转录...");

    let state = app_handle.state::<AppState>();

    state.hotkey_service.reset_state();
    state.recording_session.cancel().await;

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

    if !state.recording.is_recording_locked.load(Ordering::SeqCst) {
        return Err("未处于锁定录音状态".to_string());
    }

    state
        .recording
        .is_recording_locked
        .store(false, Ordering::SeqCst);

    // 重置热键服务状态（防止状态卡死）
    state.hotkey_service.reset_state();

    // 获取并清空触发模式（松手模式仅支持听写模式）
    let trigger_mode = state
        .recording
        .current_trigger_mode
        .lock()
        .unwrap()
        .take()
        .unwrap_or(config::TriggerMode::Dictation);

    // 播放停止提示音
    beep_player::play_stop_beep();

    // 获取需要的状态变量
    let use_realtime = *state.use_realtime_asr.lock().unwrap();
    let streaming_recorder = Arc::clone(&state.recording.streaming_recorder);
    let audio_recorder = Arc::clone(&state.recording.audio_recorder);
    let active_session = Arc::clone(&state.recording.active_session);
    let doubao_session = Arc::clone(&state.recording.doubao_session);
    let doubao_ime_session = Arc::clone(&state.recording.doubao_ime_session);
    let realtime_provider = Arc::clone(&state.realtime_provider);
    let audio_sender_handle = Arc::clone(&state.recording.audio_sender_handle);
    let post_processor = Arc::clone(&state.post_processor);
    let text_inserter = Arc::clone(&state.text_inserter);
    let qwen_client = Arc::clone(&state.qwen_client);
    let sensevoice_client = Arc::clone(&state.sensevoice_client);
    let doubao_client = Arc::clone(&state.doubao_client);
    let enable_fallback = Arc::clone(&state.enable_fallback);
    let target_hwnd = *state.recording.target_window.lock().unwrap(); // 获取目标窗口句柄
    let usage_stats = Arc::clone(&state.usage_stats);
    let recording_start_instant = Arc::clone(&state.recording.recording_start_instant);

    // 执行停止处理（仅听写模式）
    let app = app_handle.clone();
    let resources = state.recording.clone();
    let completion = state
        .recording_session
        .finish(async move {
            resources.restore_audio();
            let _ = app.emit("recording_stopped", ());
            match trigger_mode {
                config::TriggerMode::Dictation => {
                    if use_realtime {
                        handle_realtime_stop(
                            app,
                            streaming_recorder,
                            active_session,
                            doubao_session,
                            doubao_ime_session,
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
                        )
                        .await;
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
                        )
                        .await;
                    }
                }
                config::TriggerMode::AiAssistant => {
                    // 松手模式不支持 AI 助手模式，但为了安全性仍然处理
                    tracing::warn!("松手模式不支持 AI 助手模式，跳过处理");
                }
            }
        })
        .ok_or("录音已在处理中或已结束")?;
    completion.wait().await;

    Ok("录音已完成".to_string())
}

/// 取消锁定录音（松手模式）
/// 用户点击悬浮窗取消按钮时调用
#[tauri::command]
async fn cancel_locked_recording(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("用户点击取消按钮，取消锁定录音");

    let state = app_handle.state::<AppState>();

    if !state.recording.is_recording_locked.load(Ordering::SeqCst) {
        return Err("未处于锁定录音状态".to_string());
    }

    let target_hwnd = *state.recording.target_window.lock().unwrap();
    let result = cancel_transcription(app_handle.clone()).await;
    pipeline::focus::hide_overlay_and_restore_focus(&app_handle, target_hwnd).await;
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
    Ok(if enabled {
        "已启用开机自启"
    } else {
        "已禁用开机自启"
    }
    .to_string())
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

#[tauri::command]
async fn set_learning_enabled(app: AppHandle, enabled: bool) -> Result<String, String> {
    patch_config_fields(
        app,
        ConfigFieldPatch {
            learning_enabled: Some(enabled),
            ..ConfigFieldPatch::default()
        },
    )
    .await?;
    tracing::info!("自动学习已{}", if enabled { "开启" } else { "关闭" });
    Ok("ok".to_string())
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
    enable_dictionary_enhancement: Option<bool>,
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

    // 1.1 更新词库增强开关
    if let Some(enabled) = enable_dictionary_enhancement {
        *state.enable_dictionary_enhancement.lock().unwrap() = enabled;
        tracing::info!("热更新: 词库增强 = {}", enabled);
        updated.push("词库增强");
    }

    // 1.2 检查是否需要初始化/销毁 LLM 处理器（当仅更新开关而未传入 llm_config 时）
    if llm_config.is_none()
        && (enable_post_process.is_some() || enable_dictionary_enhancement.is_some())
    {
        let enable_pp = *state.enable_post_process.lock().unwrap();
        let enable_dict = *state.enable_dictionary_enhancement.lock().unwrap();
        let mut processor_guard = state.post_processor.lock().unwrap();

        if enable_pp || enable_dict {
            // 需要处理器但当前为空，从配置文件加载
            if processor_guard.is_none() {
                match crate::application::configuration::load_persisted_config() {
                    Ok(app_cfg) => {
                        let resolved = app_cfg.llm_config.resolve_polishing();
                        if !resolved.api_key.trim().is_empty() {
                            *processor_guard = Some(LlmPostProcessor::new(app_cfg.llm_config));
                            tracing::info!("热更新: LLM 处理器已从配置文件初始化");
                            updated.push("LLM处理器");
                        } else {
                            tracing::warn!("热更新: 词库增强/后处理已启用但 API Key 未配置");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("热更新: 无法加载配置文件: {}", e);
                    }
                }
            }
        } else {
            // 两个开关都关闭，销毁处理器
            if processor_guard.is_some() {
                *processor_guard = None;
                tracing::info!("热更新: LLM 处理器已销毁（后处理和词库增强均已禁用）");
                updated.push("LLM处理器");
            }
        }
    }

    // 2. 更新 LLM 配置（仅在配置变化时重新初始化处理器）
    if let Some(ref cfg) = llm_config {
        let enable_pp = *state.enable_post_process.lock().unwrap();
        let enable_dict = *state.enable_dictionary_enhancement.lock().unwrap();
        let mut processor_guard = state.post_processor.lock().unwrap();
        let resolved = cfg.resolve_polishing();

        if (enable_pp || enable_dict) && !resolved.api_key.trim().is_empty() {
            // 检查配置是否真的变了
            let needs_rebuild = match &*processor_guard {
                Some(existing) => existing.config_changed(cfg),
                None => true,
            };

            if needs_rebuild {
                *processor_guard = Some(LlmPostProcessor::new(cfg.clone()));
                tracing::info!("热更新: LLM 处理器已重新初始化（配置变更）");
                updated.push("LLM配置");
            } else {
                tracing::debug!("热更新: LLM 配置未变，跳过重建");
            }
        } else {
            if processor_guard.is_some() {
                *processor_guard = None;
                tracing::info!("热更新: LLM 处理器已销毁");
                updated.push("LLM配置");
            }
        }
    }

    // 3. 更新 AI 助手配置
    if let Some(cfg) = assistant_config {
        let mut processor_guard = state.assistant.processor.lock().unwrap();

        // 获取 shared LLM 配置（从参数或从配置文件加载）
        let shared_config = if let Some(ref llm_cfg) = llm_config {
            llm_cfg.shared.clone()
        } else {
            // 如果没有传递 llm_config，从配置文件加载
            match crate::application::configuration::load_persisted_config() {
                Ok(app_cfg) => app_cfg.llm_config.shared,
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
        if let Some(ref manager) = *state.recording.audio_mute_manager.lock().unwrap() {
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
    original: Option<String>,
    corrected: Option<String>,
    category: Option<String>,
    context: Option<String>,
) -> Result<(), String> {
    tracing::info!("添加学习词汇: {} (来源: {})", word, source);
    let stored_correction_pair = crate::personalization::record_accepted_correction_pair(
        original.as_deref(),
        corrected.as_deref(),
        category.as_deref(),
        context.as_deref(),
    )
    .map_err(|e| format!("保存个性化纠错对失败: {}", e))?;

    let (updated_config, dictionary_entries) =
        upsert_user_term_sidecar_entry_and_snapshot_config(&word, &source, category.as_deref())?;

    // 热更新运行时词库
    let state = app_handle.state::<AppState>();
    *state.dictionary.lock().unwrap() = dictionary_entries.clone();

    // 更新 ASR 客户端词库
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_dictionary(dictionary_entries.clone());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_dictionary(dictionary_entries.clone());
    }

    if stored_correction_pair.is_some() {
        refresh_asr_correction_pairs_runtime(&state);
    }

    // 发送事件通知前端刷新配置和词典
    emit_config_updated(&app_handle, &updated_config);
    app_handle.emit("dictionary_updated", ()).ok();

    if let Some(pair) = stored_correction_pair {
        tracing::info!(
            "个性化纠错对已保存: {} → {} (id: {})",
            pair.original_text,
            pair.corrected_text,
            pair.id
        );
    }
    tracing::info!("词汇 '{}' 已添加到词典", word);
    Ok(())
}

/// 获取所有词典条目
#[tauri::command]
async fn get_dictionary_entries() -> Result<Vec<String>, String> {
    tracing::info!("获取词典条目...");

    let config = load_persisted_config()?;
    let entries = dictionary_entries_from_user_terms_or_config(&config.dictionary);

    tracing::info!("返回 {} 个词典条目", entries.len());
    Ok(entries)
}

/// 删除指定词汇的词典条目（按 word 匹配）
#[tauri::command]
async fn delete_dictionary_entries(
    app_handle: AppHandle,
    words: Vec<String>,
) -> Result<(), String> {
    tracing::info!("删除词典条目: {:?}", words);
    let (updated_config, dictionary_entries) =
        delete_user_term_sidecar_entries_and_snapshot_config(&words)?;

    // 热更新运行时词库
    let state = app_handle.state::<AppState>();
    *state.dictionary.lock().unwrap() = dictionary_entries.clone();

    // 更新 ASR 客户端词库
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_dictionary(dictionary_entries.clone());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_dictionary(dictionary_entries.clone());
    }

    // 发送事件通知前端刷新配置和词典
    emit_config_updated(&app_handle, &updated_config);
    app_handle.emit("dictionary_updated", ()).ok();

    tracing::info!("词典条目删除完成");
    Ok(())
}

/// 忽略学习建议
#[tauri::command]
async fn dismiss_learning_suggestion(
    app_handle: AppHandle,
    id: String,
    original: Option<String>,
    corrected: Option<String>,
) -> Result<(), String> {
    tracing::debug!("忽略学习建议: {}", id);
    let rejected_pair = crate::personalization::record_rejected_correction_pair(
        original.as_deref(),
        corrected.as_deref(),
    )
    .map_err(|e| format!("记录学习负反馈失败: {}", e))?;
    if rejected_pair.is_some() {
        let state = app_handle.state::<AppState>();
        refresh_asr_correction_pairs_runtime(&state);
    }
    if let Some(pair) = rejected_pair {
        tracing::info!(
            "个性化纠错对负反馈: {} → {} (id: {}, confidence: {:.2}, rejected: {})",
            pair.original_text,
            pair.corrected_text,
            pair.id,
            pair.confidence,
            pair.rejected_count
        );
    }
    Ok(())
}

/// 显示通知窗口并定位到鼠标所在屏幕的悬浮窗上方
#[tauri::command]
async fn show_notification_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(notification) = app_handle.get_webview_window("notification") {
        // 使用 overlay 或 main 窗口获取显示器列表（这些窗口已正确初始化）
        // notification 窗口在首次显示前可能没有正确初始化
        let reference_window = app_handle
            .get_webview_window("overlay")
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

                notification
                    .set_position(tauri::PhysicalPosition::new(x, y))
                    .map_err(|e| format!("设置窗口位置失败: {}", e))?;
            }
        }

        // 避免抢占焦点：学习观察依赖前台窗口 hwnd，一旦通知窗口 set_focus 会导致学习误判“失焦”。
        // 通知窗口只需可见即可。
        notification
            .show()
            .map_err(|e| format!("显示窗口失败: {}", e))?;

        Ok(())
    } else {
        Err("通知窗口不存在".to_string())
    }
}

/// 测试 LLM Provider 配置是否可用
///
/// 发送一个非常短的 Chat Completions 请求来验证：
/// - endpoint 是否可达
/// - api_key 是否有效
/// - model 是否可用
///
/// 备注：endpoint 可传 base URL 或 full URL；最终会被 normalize 为 `/chat/completions`。
#[tauri::command]
async fn test_llm_provider(
    endpoint: String,
    api_key: String,
    model: String,
) -> Result<String, String> {
    let resolved_endpoint = config::normalize_chat_completions_endpoint(&endpoint);

    if resolved_endpoint.trim().is_empty() {
        return Err("Endpoint 不能为空".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("API Key 不能为空".to_string());
    }
    if model.trim().is_empty() {
        return Err("Model 不能为空".to_string());
    }

    let client = OpenAiClient::new(OpenAiClientConfig::new(resolved_endpoint, api_key, model));
    let messages = vec![
        Message::system("You are a connectivity test. Reply with: OK"),
        Message::user("OK"),
    ];

    client
        .chat(
            &messages,
            ChatOptions {
                max_tokens: 4,
                temperature: 0.0,
                reasoning: None,
                custom_body: None,
            },
        )
        .await
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("测试请求失败: {e}"))
}

#[tauri::command]
async fn test_search_provider(provider: config::SearchProviderConfig) -> Result<u32, String> {
    search::SearchRegistry::test_provider(provider)
        .await
        .map_err(|e| format!("搜索引擎连接测试失败: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 检查是否静默启动（开机自启时）
    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.contains(&"--minimized".to_string());

    let mut context = tauri::generate_context!();
    platform::window_chrome::configure(context.config_mut());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 当第二个实例启动时，将焦点切换到已有实例的主窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(application::runtime::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| {
            platform::configure_windows(app, start_minimized);
            // 如果是静默启动，隐藏主窗口
            if start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    tracing::info!("静默启动模式：主窗口已隐藏");
                }
            }

            let initial_config = load_persisted_config().unwrap_or_else(|e| {
                tracing::warn!("创建托盘菜单时加载配置失败，使用默认值: {}", e);
                AppConfig::new()
            });

            let initial_enable_post_process = initial_config.enable_llm_post_process;
            let initial_enable_dictionary_enhancement =
                initial_config.enable_dictionary_enhancement;
            let initial_enable_web_search = initial_config.assistant_config.enable_web_search;
            let initial_active_provider =
                initial_config.asr_config.selection.active_provider.clone();

            let show_item =
                MenuItem::with_id(app, TRAY_MENU_ID_SHOW, "显示窗口", true, None::<&str>)?;
            let quit_item =
                MenuItem::with_id(app, TRAY_MENU_ID_QUIT, "退出程序", true, None::<&str>)?;

            let post_process_item = CheckMenuItem::with_id(
                app,
                TRAY_MENU_ID_TOGGLE_POST_PROCESS,
                "开启语句润色",
                true,
                initial_enable_post_process,
                None::<&str>,
            )?;
            let dictionary_enhancement_item = CheckMenuItem::with_id(
                app,
                TRAY_MENU_ID_TOGGLE_DICTIONARY_ENHANCEMENT,
                "开启词库增强",
                true,
                initial_enable_dictionary_enhancement,
                None::<&str>,
            )?;
            let web_search_item = CheckMenuItem::with_id(
                app,
                TRAY_MENU_ID_TOGGLE_WEB_SEARCH,
                "联网搜索 (Beta)",
                true,
                initial_enable_web_search,
                None::<&str>,
            )?;

            let asr_qwen_item = CheckMenuItem::with_id(
                app,
                TRAY_MENU_ID_ASR_QWEN,
                "千问",
                true,
                matches!(initial_active_provider, config::AsrProvider::Qwen),
                None::<&str>,
            )?;
            let asr_doubao_item = CheckMenuItem::with_id(
                app,
                TRAY_MENU_ID_ASR_DOUBAO,
                "豆包",
                true,
                matches!(initial_active_provider, config::AsrProvider::Doubao),
                None::<&str>,
            )?;
            let asr_doubao_ime_item = CheckMenuItem::with_id(
                app,
                TRAY_MENU_ID_ASR_DOUBAO_IME,
                "豆包输入法(免费)",
                true,
                matches!(initial_active_provider, config::AsrProvider::DoubaoIme),
                None::<&str>,
            )?;
            let asr_switch_submenu = Submenu::with_items(
                app,
                "切换语音识别引擎",
                true,
                &[&asr_qwen_item, &asr_doubao_item, &asr_doubao_ime_item],
            )?;

            let menu = Menu::with_items(
                app,
                &[
                    &show_item,
                    &post_process_item,
                    &dictionary_enhancement_item,
                    &web_search_item,
                    &asr_switch_submenu,
                    &quit_item,
                ],
            )?;

            let post_process_item_for_event = post_process_item.clone();
            let dictionary_enhancement_item_for_event = dictionary_enhancement_item.clone();
            let web_search_item_for_event = web_search_item.clone();
            let asr_qwen_item_for_event = asr_qwen_item.clone();
            let asr_doubao_item_for_event = asr_doubao_item.clone();
            let asr_doubao_ime_item_for_event = asr_doubao_ime_item.clone();

            app.manage(TrayMenuState {
                post_process_item: post_process_item.clone(),
                dictionary_enhancement_item: dictionary_enhancement_item.clone(),
                web_search_item: web_search_item.clone(),
                asr_qwen_item: asr_qwen_item.clone(),
                asr_doubao_item: asr_doubao_item.clone(),
                asr_doubao_ime_item: asr_doubao_ime_item.clone(),
            });

            // 创建系统托盘图标
            let _tray = platform::tray::builder()
                .menu(&menu)
                .tooltip("PushToTalk - AI 语音转写助手")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    TRAY_MENU_ID_SHOW => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    TRAY_MENU_ID_TOGGLE_POST_PROCESS => {
                        if let Err(e) =
                            toggle_post_process_from_tray(app, &post_process_item_for_event)
                        {
                            tracing::error!("托盘切换语句润色失败: {}", e);
                            let _ = app.emit("error", e);
                        }
                    }
                    TRAY_MENU_ID_TOGGLE_DICTIONARY_ENHANCEMENT => {
                        if let Err(e) = toggle_dictionary_enhancement_from_tray(
                            app,
                            &dictionary_enhancement_item_for_event,
                        ) {
                            tracing::error!("托盘切换词库增强失败: {}", e);
                            let _ = app.emit("error", e);
                        }
                    }
                    TRAY_MENU_ID_TOGGLE_WEB_SEARCH => {
                        if let Err(e) = toggle_web_search_from_tray(app, &web_search_item_for_event)
                        {
                            tracing::error!("托盘切换联网搜索失败: {}", e);
                            let _ = app.emit("error", e);
                        }
                    }
                    TRAY_MENU_ID_ASR_QWEN => {
                        let app_handle = app.clone();
                        let asr_qwen_item = asr_qwen_item_for_event.clone();
                        let asr_doubao_item = asr_doubao_item_for_event.clone();
                        let asr_doubao_ime_item = asr_doubao_ime_item_for_event.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = switch_asr_provider_from_tray(
                                app_handle.clone(),
                                config::AsrProvider::Qwen,
                                asr_qwen_item,
                                asr_doubao_item,
                                asr_doubao_ime_item,
                            )
                            .await
                            {
                                tracing::error!("托盘切换 ASR 到千问失败: {}", e);
                                let _ = app_handle.emit("error", e);
                            }
                        });
                    }
                    TRAY_MENU_ID_ASR_DOUBAO => {
                        let app_handle = app.clone();
                        let asr_qwen_item = asr_qwen_item_for_event.clone();
                        let asr_doubao_item = asr_doubao_item_for_event.clone();
                        let asr_doubao_ime_item = asr_doubao_ime_item_for_event.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = switch_asr_provider_from_tray(
                                app_handle.clone(),
                                config::AsrProvider::Doubao,
                                asr_qwen_item,
                                asr_doubao_item,
                                asr_doubao_ime_item,
                            )
                            .await
                            {
                                tracing::error!("托盘切换 ASR 到豆包失败: {}", e);
                                let _ = app_handle.emit("error", e);
                            }
                        });
                    }
                    TRAY_MENU_ID_ASR_DOUBAO_IME => {
                        let app_handle = app.clone();
                        let asr_qwen_item = asr_qwen_item_for_event.clone();
                        let asr_doubao_item = asr_doubao_item_for_event.clone();
                        let asr_doubao_ime_item = asr_doubao_ime_item_for_event.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = switch_asr_provider_from_tray(
                                app_handle.clone(),
                                config::AsrProvider::DoubaoIme,
                                asr_qwen_item,
                                asr_doubao_item,
                                asr_doubao_ime_item,
                            )
                            .await
                            {
                                tracing::error!("托盘切换 ASR 到豆包输入法失败: {}", e);
                                let _ = app_handle.emit("error", e);
                            }
                        });
                    }
                    TRAY_MENU_ID_QUIT => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            let state = app.state::<AppState>();
            let app_handle = app.handle().clone();
            start_builtin_dictionary_updater(
                &app_handle,
                &state.builtin_dictionary_updater_started,
                &state.builtin_hotwords_raw,
            );

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("close_requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            llm_reasoning::get_reasoning_options,
            #[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
            atdd::run,
            #[cfg(all(feature = "atdd", target_os = "macos", debug_assertions))]
            atdd::atdd_cancel,
            get_platform_status,
            request_platform_permission,
            save_config,
            patch_config_fields,
            load_config,
            get_config_snapshot,
            update_config,
            get_builtin_domains_raw,
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
            set_learning_enabled,
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
            application::assistant::get_conversation_state,
            application::assistant::paste_latest_reply,
            application::assistant::copy_latest_reply,
            application::assistant::copy_full_conversation,
            application::assistant::dismiss_conversation,
            application::assistant::cancel_assistant_generation,
            application::assistant::send_text_question,
            show_notification_window,
            test_llm_provider,
            test_search_provider,
        ])
        .build(context)
        .expect("error while building tauri application")
        .run(|app, event| platform::handle_run_event(app, &event));
}
