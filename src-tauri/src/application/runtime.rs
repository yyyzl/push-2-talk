//! Application-owned runtime state, composed from capability and workflow owners.
use super::{recording::RecordingSession, recording_resources::RecordingResources};
use crate::{
    asr::{DoubaoASRClient, DoubaoImeCredentials, QwenASRClient, SenseVoiceClient},
    config,
    llm_post_processor::LlmPostProcessor,
    personalization::CorrectionPair,
    platform::HotkeyService,
    text_inserter::TextInserter,
    usage_stats::UsageStats,
};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tauri::Manager;

/// Tauri 先初始化插件，再创建配置中的 WebView，最后才执行应用 setup。
/// 在单实例插件之后注册状态，既避免提前到达的 IPC panic，也不让第二实例迁移配置。
pub(crate) fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("runtime-state")
        .setup(|app, _| {
            let initial_config =
                super::configuration::load_persisted_config().unwrap_or_else(|e| {
                    tracing::warn!("初始化运行状态时加载配置失败，使用默认值: {}", e);
                    config::AppConfig::new()
                });
            let usage_stats = UsageStats::load().unwrap_or_else(|e| {
                tracing::warn!("加载统计数据失败: {}, 使用默认值", e);
                UsageStats::default()
            });
            let builtin_hotwords = crate::builtin_dictionary_updater::load_builtin_hotwords();

            app.manage(AppState {
                assistant: super::assistant::AssistantState::default(),
                recording_session: Arc::default(),
                recording: RecordingResources::default(),
                text_inserter: Arc::new(Mutex::new(None)),
                post_processor: Arc::new(Mutex::new(None)),
                is_running: Arc::new(Mutex::new(false)),
                use_realtime_asr: Arc::new(Mutex::new(true)),
                enable_post_process: Arc::new(Mutex::new(initial_config.enable_llm_post_process)),
                enable_dictionary_enhancement: Arc::new(Mutex::new(
                    initial_config.enable_dictionary_enhancement,
                )),
                enable_fallback: Arc::new(Mutex::new(false)),
                qwen_client: Arc::new(Mutex::new(None)),
                sensevoice_client: Arc::new(Mutex::new(None)),
                doubao_client: Arc::new(Mutex::new(None)),
                realtime_provider: Arc::new(Mutex::new(Some(
                    initial_config.asr_config.selection.active_provider,
                ))),
                fallback_provider: Arc::new(Mutex::new(None)),
                hotkey_service: Arc::new(HotkeyService::new()),
                dictionary: Arc::new(Mutex::new(Vec::new())),
                asr_correction_pairs: Arc::new(Mutex::new(Vec::new())),
                doubao_ime_credentials: Arc::new(Mutex::new(None)),
                usage_stats: Arc::new(Mutex::new(usage_stats)),
                builtin_hotwords_raw: Arc::new(Mutex::new(builtin_hotwords)),
                builtin_dictionary_updater_started: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .build()
}

pub(crate) struct AppState {
    pub assistant: super::assistant::AssistantState,
    pub recording_session: Arc<RecordingSession>,
    pub recording: RecordingResources,
    pub text_inserter: Arc<Mutex<Option<TextInserter>>>,
    pub post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    pub is_running: Arc<Mutex<bool>>,
    pub use_realtime_asr: Arc<Mutex<bool>>,
    pub enable_post_process: Arc<Mutex<bool>>,
    /// 语句润色：是否启用“词库增强”（将个人词库注入提示词）
    pub enable_dictionary_enhancement: Arc<Mutex<bool>>,
    pub enable_fallback: Arc<Mutex<bool>>,
    pub qwen_client: Arc<Mutex<Option<QwenASRClient>>>,
    pub sensevoice_client: Arc<Mutex<Option<SenseVoiceClient>>>,
    pub doubao_client: Arc<Mutex<Option<DoubaoASRClient>>>,
    pub realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    pub fallback_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    // 单例热键服务
    pub hotkey_service: Arc<HotkeyService>,
    /// 词库（用于 Realtime 模式热更新）
    pub dictionary: Arc<Mutex<Vec<String>>>,
    /// 个性化纠错对（用于 ASR 热词编译，录音开始前读取快照）
    pub asr_correction_pairs: Arc<Mutex<Vec<CorrectionPair>>>,
    /// 豆包输入法凭据（自动注册获取，跨会话复用）
    pub doubao_ime_credentials: Arc<Mutex<Option<DoubaoImeCredentials>>>,
    /// 使用统计数据
    pub usage_stats: Arc<Mutex<UsageStats>>,
    /// 内置词库原始内容（用于前端动态解析）
    pub builtin_hotwords_raw: Arc<Mutex<String>>,
    /// 内置词库后台更新任务是否已启动（进程级单例）
    pub builtin_dictionary_updater_started: Arc<AtomicBool>,
}
