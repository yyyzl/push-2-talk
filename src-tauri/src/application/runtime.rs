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
