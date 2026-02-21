use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::FutureExt;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::*;

pub(crate) struct TrayMenuState {
    pub(crate) post_process_item: CheckMenuItem<tauri::Wry>,
    pub(crate) dictionary_enhancement_item: CheckMenuItem<tauri::Wry>,
    pub(crate) asr_qwen_item: CheckMenuItem<tauri::Wry>,
    pub(crate) asr_doubao_item: CheckMenuItem<tauri::Wry>,
    pub(crate) asr_doubao_ime_item: CheckMenuItem<tauri::Wry>,
}

pub(crate) const TRAY_MENU_ID_SHOW: &str = "show";
pub(crate) const TRAY_MENU_ID_QUIT: &str = "quit";
pub(crate) const TRAY_MENU_ID_TOGGLE_POST_PROCESS: &str = "tray_toggle_post_process";
pub(crate) const TRAY_MENU_ID_TOGGLE_DICTIONARY_ENHANCEMENT: &str =
    "tray_toggle_dictionary_enhancement";
pub(crate) const TRAY_MENU_ID_ASR_QWEN: &str = "tray_asr_qwen";
pub(crate) const TRAY_MENU_ID_ASR_DOUBAO: &str = "tray_asr_doubao";
pub(crate) const TRAY_MENU_ID_ASR_DOUBAO_IME: &str = "tray_asr_doubao_ime";

/// 全局互斥标志：防止并发 ASR 引擎切换导致多个 restart 并行执行
static TRAY_ASR_SWITCHING: AtomicBool = AtomicBool::new(false);

pub(crate) fn setup_app(
    app: &mut tauri::App,
    start_minimized: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if start_minimized {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
            tracing::info!("静默启动模式：主窗口已隐藏");
        }
    }

    let usage_stats = UsageStats::load().unwrap_or_else(|e| {
        tracing::warn!("加载统计数据失败: {}, 使用默认值", e);
        UsageStats::default()
    });
    let initial_builtin_hotwords = builtin_dictionary_updater::load_builtin_hotwords();
    let builtin_hotwords_raw = Arc::new(Mutex::new(initial_builtin_hotwords));
    let builtin_dictionary_updater_started = Arc::new(AtomicBool::new(false));

    let app_state = AppState {
        audio_recorder: Arc::new(Mutex::new(None)),
        streaming_recorder: Arc::new(Mutex::new(None)),
        text_inserter: Arc::new(Mutex::new(None)),
        post_processor: Arc::new(Mutex::new(None)),
        assistant_processor: Arc::new(Mutex::new(None)),
        is_running: Arc::new(Mutex::new(false)),
        use_realtime_asr: Arc::new(Mutex::new(true)),
        enable_post_process: Arc::new(Mutex::new(false)),
        enable_dictionary_enhancement: Arc::new(Mutex::new(true)),
        enable_fallback: Arc::new(Mutex::new(false)),
        qwen_client: Arc::new(Mutex::new(None)),
        sensevoice_client: Arc::new(Mutex::new(None)),
        doubao_client: Arc::new(Mutex::new(None)),
        active_session: Arc::new(tokio::sync::Mutex::new(None)),
        doubao_session: Arc::new(tokio::sync::Mutex::new(None)),
        doubao_ime_session: Arc::new(tokio::sync::Mutex::new(None)),
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
        doubao_ime_credentials: Arc::new(Mutex::new(None)),
        usage_stats: Arc::new(Mutex::new(usage_stats)),
        recording_start_instant: Arc::new(Mutex::new(None)),
        builtin_hotwords_raw: Arc::clone(&builtin_hotwords_raw),
        builtin_dictionary_updater_started: Arc::clone(&builtin_dictionary_updater_started),
    };

    let initial_config = load_persisted_config().unwrap_or_else(|e| {
        tracing::warn!("创建托盘菜单时加载配置失败，使用默认值: {}", e);
        AppConfig::new()
    });

    let state_enable_post_process = *app_state.enable_post_process.lock().unwrap();
    let state_enable_dictionary_enhancement =
        *app_state.enable_dictionary_enhancement.lock().unwrap();

    let initial_enable_post_process = initial_config.enable_llm_post_process;
    let initial_enable_dictionary_enhancement = initial_config.enable_dictionary_enhancement;
    let initial_active_provider = initial_config.asr_config.selection.active_provider.clone();

    *app_state.enable_post_process.lock().unwrap() = initial_enable_post_process;
    *app_state.enable_dictionary_enhancement.lock().unwrap() =
        initial_enable_dictionary_enhancement;
    *app_state.realtime_provider.lock().unwrap() = Some(initial_active_provider.clone());

    if state_enable_post_process != initial_enable_post_process {
        tracing::info!(
            "托盘初始化语句润色状态: {} -> {}",
            state_enable_post_process,
            initial_enable_post_process
        );
    }
    if state_enable_dictionary_enhancement != initial_enable_dictionary_enhancement {
        tracing::info!(
            "托盘初始化词库增强状态: {} -> {}",
            state_enable_dictionary_enhancement,
            initial_enable_dictionary_enhancement
        );
    }

    let show_item = MenuItem::with_id(app, TRAY_MENU_ID_SHOW, "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, TRAY_MENU_ID_QUIT, "退出程序", true, None::<&str>)?;

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
            &asr_switch_submenu,
            &quit_item,
        ],
    )?;

    let post_process_item_for_event = post_process_item.clone();
    let dictionary_enhancement_item_for_event = dictionary_enhancement_item.clone();
    let asr_qwen_item_for_event = asr_qwen_item.clone();
    let asr_doubao_item_for_event = asr_doubao_item.clone();
    let asr_doubao_ime_item_for_event = asr_doubao_ime_item.clone();

    app.manage(TrayMenuState {
        post_process_item: post_process_item.clone(),
        dictionary_enhancement_item: dictionary_enhancement_item.clone(),
        asr_qwen_item: asr_qwen_item.clone(),
        asr_doubao_item: asr_doubao_item.clone(),
        asr_doubao_ime_item: asr_doubao_ime_item.clone(),
    });

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
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
                if let Err(e) = toggle_post_process_from_tray(app, &post_process_item_for_event) {
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

    app.manage(app_state);
    let state = app.state::<AppState>();
    let app_handle = app.handle().clone();
    start_builtin_dictionary_updater(
        &app_handle,
        &state.builtin_dictionary_updater_started,
        &state.builtin_hotwords_raw,
    );

    Ok(())
}

pub(crate) fn sync_tray_menu_from_config(app_handle: &AppHandle, config: &AppConfig) {
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

    sync_asr_provider_checks(
        &tray_state.asr_qwen_item,
        &tray_state.asr_doubao_item,
        &tray_state.asr_doubao_ime_item,
        &config.asr_config.selection.active_provider,
    );
}

pub(crate) fn load_persisted_config() -> Result<AppConfig, String> {
    match AppConfig::load() {
        Ok((config, migrated)) => {
            if migrated {
                config
                    .save()
                    .map_err(|e| format!("保存迁移后的配置失败: {}", e))?;
            }
            Ok(config)
        }
        Err(e) => Err(format!("加载配置失败: {}", e)),
    }
}

pub(crate) fn save_persisted_config_without_emit(config: &AppConfig) -> Result<(), String> {
    config.save().map_err(|e| format!("保存配置失败: {}", e))?;
    Ok(())
}

pub(crate) fn mutate_persisted_config_with_result<R, F>(
    mutator: F,
) -> Result<(AppConfig, R), String>
where
    F: FnOnce(&mut AppConfig) -> Result<R, String>,
{
    let _guard = CONFIG_LOCK
        .lock()
        .map_err(|e| format!("获取配置锁失败: {}", e))?;

    let mut config = load_persisted_config()?;
    let result = mutator(&mut config)?;
    save_persisted_config_without_emit(&config)?;

    Ok((config, result))
}

pub(crate) fn mutate_persisted_config<F>(mutator: F) -> Result<AppConfig, String>
where
    F: FnOnce(&mut AppConfig) -> Result<(), String>,
{
    mutate_persisted_config_with_result(|config| {
        mutator(config)?;
        Ok(())
    })
    .map(|(config, _)| config)
}

pub(crate) fn emit_config_updated(app: &AppHandle, config: &AppConfig) {
    sync_tray_menu_from_config(app, config);
    let _ = app.emit("config_updated", config);
}

fn hotwords_content_changed(current: &str, next: &str) -> bool {
    current.trim() != next.trim()
}

pub(crate) fn lock_hotwords_or_recover<'a>(
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

pub(crate) fn start_builtin_dictionary_updater(
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

#[cfg(target_os = "macos")]
pub(crate) fn check_microphone_access() -> bool {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        return false;
    };
    device.supported_input_configs().is_ok()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn check_microphone_access() -> bool {
    true
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
}

#[cfg(target_os = "macos")]
pub(crate) fn check_input_monitoring() -> bool {
    // 与 Accessibility 不同，输入监控有独立授权位。
    // CGPreflightListenEventAccess 可直接查询当前进程是否具备监听权限。
    unsafe { CGPreflightListenEventAccess() }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn check_input_monitoring() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub(crate) fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn check_accessibility() -> bool {
    true
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

async fn restart_service_with_config(
    app_handle: AppHandle,
    config: AppConfig,
) -> Result<(), String> {
    if let Err(e) = commands::lifecycle::stop_app(app_handle.clone()).await {
        tracing::warn!("切换 ASR 引擎时停止服务失败: {}", e);
    }

    let dictionary_words = learning::store::entries_to_words(&config.dictionary);

    commands::lifecycle::start_app(
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
        Some(dictionary_words),
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

pub(crate) fn toggle_post_process_from_tray(
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

pub(crate) fn toggle_dictionary_enhancement_from_tray(
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

pub(crate) async fn switch_asr_provider_from_tray(
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
    let config = {
        let _guard = CONFIG_LOCK
            .lock()
            .map_err(|e| format!("获取配置锁失败: {}", e))?;

        let mut config = load_persisted_config()?;

        if !is_asr_provider_configured(&config, &target_provider) {
            sync_asr_provider_checks(
                qwen_item,
                doubao_item,
                doubao_ime_item,
                &config.asr_config.selection.active_provider,
            );
            return Err(format!(
                "{} 未配置凭证，无法切换",
                asr_provider_name(&target_provider)
            ));
        }

        if config.asr_config.selection.active_provider == target_provider {
            sync_asr_provider_checks(qwen_item, doubao_item, doubao_ime_item, &target_provider);
            return Ok(());
        }

        config.asr_config.selection.active_provider = target_provider.clone();
        save_persisted_config_without_emit(&config)?;
        config
    };

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
