// src-tauri/src/config.rs

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// 词典相关函数已移至独立的 dictionary_utils 模块

// ============================================================================
// 全局配置操作锁
// ============================================================================

lazy_static::lazy_static! {
    /// 全局配置操作锁
    ///
    /// 保护所有 config 的读写操作，防止并发 load->modify->save 导致的数据丢失
    ///
    /// 使用方式：
    /// ```ignore
    /// let _guard = CONFIG_LOCK.lock().unwrap();
    /// let (mut config, _) = AppConfig::load()?;
    /// // 修改 config...
    /// config.save()?;
    /// ```
    pub static ref CONFIG_LOCK: Mutex<()> = Mutex::new(());
}

// ============================================================================
// 热键触发模式
// ============================================================================

/// 热键触发模式
///
/// 决定如何通过热键控制录音的开始和结束
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    /// 按住模式（默认）：按住快捷键开始录音，松开结束
    #[default]
    Press,
    /// 切换模式：按一下开始录音，再按一下结束
    Toggle,
}

// ============================================================================
// 转录处理模式
// ============================================================================

/// 转录处理模式
///
/// 决定 ASR 结果如何被后续处理
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionMode {
    /// 普通模式：ASR → 可选LLM润色 → 自动插入文本
    #[default]
    Normal,
    /// AI 助手模式：语音指令 → ASR → LLM处理 → 插入结果
    Assistant,
}

// ============================================================================
// 触发模式（新增）
// ============================================================================

/// 热键触发模式
///
/// 决定用户按下哪个快捷键，从而决定处理流程
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// 听写模式：语音 → ASR → 可选润色 → 插入文本
    Dictation,
    /// AI助手模式：(可选)选中文本 + 语音指令 → ASR → LLM处理 → 插入/替换文本
    AiAssistant,
}

// ============================================================================
// 热键配置
// ============================================================================

/// 热键配置支持的按键类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyKey {
    // 修饰键
    ControlLeft,
    ControlRight,
    ShiftLeft,
    ShiftRight,
    AltLeft,
    AltRight,
    MetaLeft,  // Win/Cmd 左
    MetaRight, // Win/Cmd 右

    // 功能键
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // 常用键
    Space,
    Tab,
    CapsLock,
    Escape,

    // 字母键
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,

    // 数字键
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // 方向键
    Up,
    Down,
    Left,
    Right,

    // 编辑键
    Return,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
}

impl HotkeyKey {
    /// 判断是否为修饰键
    pub fn is_modifier(&self) -> bool {
        matches!(
            self,
            HotkeyKey::ControlLeft
                | HotkeyKey::ControlRight
                | HotkeyKey::ShiftLeft
                | HotkeyKey::ShiftRight
                | HotkeyKey::AltLeft
                | HotkeyKey::AltRight
                | HotkeyKey::MetaLeft
                | HotkeyKey::MetaRight
        )
    }

    /// 判断是否为功能键
    pub fn is_function_key(&self) -> bool {
        matches!(
            self,
            HotkeyKey::F1
                | HotkeyKey::F2
                | HotkeyKey::F3
                | HotkeyKey::F4
                | HotkeyKey::F5
                | HotkeyKey::F6
                | HotkeyKey::F7
                | HotkeyKey::F8
                | HotkeyKey::F9
                | HotkeyKey::F10
                | HotkeyKey::F11
                | HotkeyKey::F12
        )
    }

    /// 获取显示名称（用于日志和调试）
    pub fn display_name(&self) -> &'static str {
        match self {
            HotkeyKey::ControlLeft => "Ctrl(左)",
            HotkeyKey::ControlRight => "Ctrl(右)",
            HotkeyKey::ShiftLeft => "Shift(左)",
            HotkeyKey::ShiftRight => "Shift(右)",
            HotkeyKey::AltLeft => "Alt(左)",
            HotkeyKey::AltRight => "Alt(右)",
            HotkeyKey::MetaLeft => "Win(左)",
            HotkeyKey::MetaRight => "Win(右)",
            HotkeyKey::Space => "Space",
            HotkeyKey::Tab => "Tab",
            HotkeyKey::CapsLock => "CapsLock",
            HotkeyKey::Escape => "Esc",
            HotkeyKey::F1 => "F1",
            HotkeyKey::F2 => "F2",
            HotkeyKey::F3 => "F3",
            HotkeyKey::F4 => "F4",
            HotkeyKey::F5 => "F5",
            HotkeyKey::F6 => "F6",
            HotkeyKey::F7 => "F7",
            HotkeyKey::F8 => "F8",
            HotkeyKey::F9 => "F9",
            HotkeyKey::F10 => "F10",
            HotkeyKey::F11 => "F11",
            HotkeyKey::F12 => "F12",
            HotkeyKey::KeyA => "A",
            HotkeyKey::KeyB => "B",
            HotkeyKey::KeyC => "C",
            HotkeyKey::KeyD => "D",
            HotkeyKey::KeyE => "E",
            HotkeyKey::KeyF => "F",
            HotkeyKey::KeyG => "G",
            HotkeyKey::KeyH => "H",
            HotkeyKey::KeyI => "I",
            HotkeyKey::KeyJ => "J",
            HotkeyKey::KeyK => "K",
            HotkeyKey::KeyL => "L",
            HotkeyKey::KeyM => "M",
            HotkeyKey::KeyN => "N",
            HotkeyKey::KeyO => "O",
            HotkeyKey::KeyP => "P",
            HotkeyKey::KeyQ => "Q",
            HotkeyKey::KeyR => "R",
            HotkeyKey::KeyS => "S",
            HotkeyKey::KeyT => "T",
            HotkeyKey::KeyU => "U",
            HotkeyKey::KeyV => "V",
            HotkeyKey::KeyW => "W",
            HotkeyKey::KeyX => "X",
            HotkeyKey::KeyY => "Y",
            HotkeyKey::KeyZ => "Z",
            HotkeyKey::Num0 => "0",
            HotkeyKey::Num1 => "1",
            HotkeyKey::Num2 => "2",
            HotkeyKey::Num3 => "3",
            HotkeyKey::Num4 => "4",
            HotkeyKey::Num5 => "5",
            HotkeyKey::Num6 => "6",
            HotkeyKey::Num7 => "7",
            HotkeyKey::Num8 => "8",
            HotkeyKey::Num9 => "9",
            HotkeyKey::Up => "↑",
            HotkeyKey::Down => "↓",
            HotkeyKey::Left => "←",
            HotkeyKey::Right => "→",
            HotkeyKey::Return => "Enter",
            HotkeyKey::Backspace => "Backspace",
            HotkeyKey::Delete => "Delete",
            HotkeyKey::Insert => "Insert",
            HotkeyKey::Home => "Home",
            HotkeyKey::End => "End",
            HotkeyKey::PageUp => "PageUp",
            HotkeyKey::PageDown => "PageDown",
        }
    }
}

/// 热键配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// 需要同时按下的按键列表
    pub keys: Vec<HotkeyKey>,
    /// 热键触发模式（默认为按住模式）
    #[serde(default)]
    pub mode: HotkeyMode,
    /// 松手模式开关（仅听写模式生效）
    /// 已弃用：现在通过 release_mode_keys 独立配置
    #[serde(default)]
    pub enable_release_lock: bool,
    /// 松手模式独立快捷键（可选）
    /// 如果设置，则按此快捷键直接启动松手模式，无需长按
    /// 默认为 F2（仅听写模式）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_mode_keys: Option<Vec<HotkeyKey>>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        // 默认为 Ctrl+Win（向后兼容）
        Self {
            keys: vec![HotkeyKey::ControlLeft, HotkeyKey::MetaLeft],
            mode: HotkeyMode::default(),
            enable_release_lock: false,
            release_mode_keys: None, // 默认无松手模式快捷键
        }
    }
}

// ============================================================================
// 双快捷键配置（新增）
// ============================================================================

/// 双快捷键配置
///
/// 支持两个独立的快捷键，分别触发听写模式和AI助手模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualHotkeyConfig {
    /// 听写模式快捷键（默认 Ctrl+Win）
    #[serde(default = "default_dictation_hotkey")]
    pub dictation: HotkeyConfig,
    /// AI助手模式快捷键（默认 Alt+Space）
    #[serde(default = "default_assistant_hotkey")]
    pub assistant: HotkeyConfig,
}

fn default_dictation_hotkey() -> HotkeyConfig {
    HotkeyConfig {
        keys: vec![HotkeyKey::ControlLeft, HotkeyKey::MetaLeft],
        mode: HotkeyMode::Press,
        enable_release_lock: false,
        release_mode_keys: Some(vec![HotkeyKey::F2]), // 默认 F2 为松手模式快捷键
    }
}

fn default_assistant_hotkey() -> HotkeyConfig {
    HotkeyConfig {
        keys: vec![HotkeyKey::AltLeft, HotkeyKey::Space],
        mode: HotkeyMode::Press,
        enable_release_lock: false,
        release_mode_keys: None, // AI助手模式不支持松手模式
    }
}

impl Default for DualHotkeyConfig {
    fn default() -> Self {
        Self {
            dictation: default_dictation_hotkey(),
            assistant: default_assistant_hotkey(),
        }
    }
}

impl DualHotkeyConfig {
    /// 验证双快捷键配置
    ///
    /// 检查：
    /// 1. 两个快捷键各自有效
    /// 2. 两个快捷键不冲突（不完全相同）
    /// 3. 两个快捷键不存在子集关系（避免按键冲突）
    pub fn validate(&self) -> Result<()> {
        // 验证各自配置
        self.dictation
            .validate()
            .map_err(|e| anyhow::anyhow!("听写模式快捷键配置无效: {}", e))?;
        self.assistant
            .validate()
            .map_err(|e| anyhow::anyhow!("AI助手模式快捷键配置无效: {}", e))?;

        // 检查冲突：两个快捷键的按键集合不能完全相同
        let dictation_set: HashSet<_> = self.dictation.keys.iter().collect();
        let assistant_set: HashSet<_> = self.assistant.keys.iter().collect();

        if dictation_set == assistant_set {
            anyhow::bail!("听写模式和AI助手模式不能使用相同的快捷键");
        }

        // 检查子集关系：一组快捷键不能是另一组的子集
        // 例如：听写 Ctrl+Space，助手 Ctrl+Shift+Space 会导致冲突
        // 因为按下 Ctrl+Shift+Space 时必须先经过 Ctrl+Space 状态
        if dictation_set.is_subset(&assistant_set) || assistant_set.is_subset(&dictation_set) {
            anyhow::bail!(
                "一组快捷键不能包含另一组快捷键（这会导致按键冲突）。\n\
                 例如：Ctrl+Space 和 Ctrl+Shift+Space 会冲突，\n\
                 因为按下后者时会先触发前者。"
            );
        }

        Ok(())
    }
}

impl HotkeyConfig {
    /// 检查是否包含至少一个修饰键
    pub fn has_modifier(&self) -> bool {
        self.keys.iter().any(|k| k.is_modifier())
    }

    /// 验证热键配置是否有效
    pub fn validate(&self) -> Result<()> {
        if self.keys.is_empty() {
            anyhow::bail!("热键配置不能为空");
        }

        // 允许功能键单独使用，其他按键必须配合修饰键
        let has_function_key = self.keys.iter().any(|k| k.is_function_key());
        if !self.has_modifier() && !has_function_key {
            anyhow::bail!("热键必须包含至少一个修饰键 (Ctrl/Alt/Shift/Win) 或使用功能键 (F1-F12)");
        }

        if self.keys.len() > 4 {
            anyhow::bail!("热键最多支持4个按键组合");
        }

        // 检查是否有重复按键
        let unique_keys: HashSet<_> = self.keys.iter().collect();
        if unique_keys.len() != self.keys.len() {
            anyhow::bail!("热键配置中存在重复的按键");
        }

        // 验证松手模式快捷键（如果设置）
        if let Some(ref release_keys) = self.release_mode_keys {
            if release_keys.is_empty() {
                anyhow::bail!("松手模式快捷键配置不能为空");
            }

            let release_has_function = release_keys.iter().any(|k| k.is_function_key());
            let release_has_modifier = release_keys.iter().any(|k| k.is_modifier());
            if !release_has_modifier && !release_has_function {
                anyhow::bail!("松手模式快捷键必须包含至少一个修饰键或功能键");
            }

            if release_keys.len() > 4 {
                anyhow::bail!("松手模式快捷键最多支持4个按键组合");
            }

            // 检查松手模式快捷键是否有重复按键
            let release_unique: HashSet<_> = release_keys.iter().collect();
            if release_unique.len() != release_keys.len() {
                anyhow::bail!("松手模式快捷键配置中存在重复的按键");
            }

            // 检查与主快捷键不冲突
            let main_set: HashSet<_> = self.keys.iter().collect();
            let release_set: HashSet<_> = release_keys.iter().collect();
            if main_set == release_set {
                anyhow::bail!("松手模式快捷键不能与主快捷键相同");
            }
        }

        Ok(())
    }

    /// 格式化为显示字符串（用于日志）
    pub fn format_display(&self) -> String {
        self.keys
            .iter()
            .map(|k| k.display_name())
            .collect::<Vec<_>>()
            .join("+")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AsrProvider {
    Qwen,
    Doubao,
    #[serde(rename = "doubao_ime")]
    DoubaoIme,
    #[serde(rename = "siliconflow")]
    SiliconFlow,
}

impl AsrProvider {
    pub fn realtime_enabled(&self, requested: bool) -> bool {
        match self {
            Self::DoubaoIme => true,
            Self::SiliconFlow => false,
            _ => requested,
        }
    }
}

impl Default for AsrProvider {
    fn default() -> Self {
        AsrProvider::DoubaoIme
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AsrCredentials {
    #[serde(default)]
    pub qwen_api_key: String,
    #[serde(default)]
    pub sensevoice_api_key: String,
    #[serde(default)]
    pub doubao_app_id: String,
    #[serde(default)]
    pub doubao_access_token: String,
    // 豆包输入法 ASR 凭据 (自动注册获取)
    #[serde(default)]
    pub doubao_ime_device_id: String,
    #[serde(default)]
    pub doubao_ime_token: String,
    #[serde(default)]
    pub doubao_ime_cdid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrSelection {
    #[serde(default)]
    pub active_provider: AsrProvider,
    #[serde(default)]
    pub enable_fallback: bool,
    #[serde(default)]
    pub fallback_provider: Option<AsrProvider>,
}

impl Default for AsrSelection {
    fn default() -> Self {
        Self {
            active_provider: AsrProvider::DoubaoIme,
            enable_fallback: false,
            fallback_provider: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AsrLanguageMode {
    Zh,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QwenAsrProfile {
    #[default]
    #[serde(rename = "qwen_audio_3_1")]
    QwenAudio3_1,
    #[serde(rename = "qwen_audio_3", alias = "qwen_audio3")]
    QwenAudio3,
    Qwen3Legacy,
}

impl QwenAsrProfile {
    pub fn http_model(self) -> &'static str {
        match self {
            Self::QwenAudio3_1 => "qwen-audio-3.1-asr-flash",
            Self::QwenAudio3 => "qwen-audio-3.0-asr-flash",
            Self::Qwen3Legacy => "qwen3-asr-flash",
        }
    }

    pub fn realtime_model(self) -> &'static str {
        match self {
            Self::QwenAudio3_1 => "qwen-audio-3.1-asr-flash-streaming",
            Self::QwenAudio3 => "qwen-audio-3.0-asr-flash-streaming",
            Self::Qwen3Legacy => "qwen3-asr-flash-realtime",
        }
    }
}

fn legacy_qwen_profile() -> QwenAsrProfile {
    QwenAsrProfile::Qwen3Legacy
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QwenModelSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    pub credentials: AsrCredentials,
    pub selection: AsrSelection,
    #[serde(default = "legacy_qwen_profile")]
    pub qwen_profile: QwenAsrProfile,
    #[serde(default)]
    pub qwen_models: QwenModelSelection,
    #[serde(default)]
    pub language_mode: AsrLanguageMode,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            credentials: AsrCredentials::default(),
            selection: AsrSelection::default(),
            qwen_profile: QwenAsrProfile::default(),
            qwen_models: QwenModelSelection::default(),
            language_mode: AsrLanguageMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub dashscope_api_key: String,
    #[serde(default)]
    pub siliconflow_api_key: String,
    #[serde(default)]
    pub asr_config: AsrConfig,
    #[serde(default = "default_use_realtime_asr")]
    pub use_realtime_asr: bool,
    #[serde(default)]
    pub enable_llm_post_process: bool,
    /// 语句润色：是否启用“词库增强”（将个人词库注入提示词用于同音词纠错）
    #[serde(default = "default_enable_dictionary_enhancement")]
    pub enable_dictionary_enhancement: bool,
    #[serde(default)]
    pub llm_config: LlmConfig,
    /// Smart Command 独立配置（保留以便向后兼容）
    #[serde(default)]
    pub smart_command_config: SmartCommandConfig,
    /// AI 助手配置（新增）
    #[serde(default = "legacy_assistant_config")]
    pub assistant_config: AssistantConfig,
    /// 联网搜索配置
    #[serde(default)]
    pub search_config: SearchConfig,
    /// 自动词库学习配置
    #[serde(default)]
    pub learning_config: LearningConfig,
    /// TNL 技术规范化层配置
    #[serde(default = "legacy_tnl_config")]
    pub tnl_config: TnlConfig,
    /// 关闭行为: "close" = 直接关闭, "minimize" = 最小化到托盘, None = 每次询问
    #[serde(default)]
    pub close_action: Option<String>,
    /// 热键配置（旧版，保留以便迁移）
    #[serde(default, skip_serializing)]
    pub hotkey_config: Option<HotkeyConfig>,
    /// 双快捷键配置（新版）
    #[serde(default)]
    pub dual_hotkey_config: DualHotkeyConfig,
    /// 转录处理模式（默认普通模式）
    #[serde(default)]
    pub transcription_mode: TranscriptionMode,
    /// 录音时自动静音其他应用
    #[serde(default)]
    pub enable_mute_other_apps: bool,
    /// 个人词典（热词列表）- 简化格式："word" 或 "word|auto"
    #[serde(default)]
    pub dictionary: Vec<String>,
    /// 内置词库领域（用于组合请求词库）
    #[serde(default)]
    pub builtin_dictionary_domains: Vec<String>,
    /// 悬浮窗主题 ("light" | "dark")
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "light".to_string()
}

// ============================================================================
// 自动词库学习配置
// ============================================================================

/// 自动词库学习配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    /// 是否启用自动学习
    #[serde(default)]
    pub enabled: bool,
    /// 观察期时长（秒），默认 15 秒
    #[serde(default = "default_observation_duration_secs")]
    pub observation_duration_secs: u64,
    /// 独立的 LLM 端点（如果为 None，则使用通用 LLM 配置）
    /// 保留用于向后兼容
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_endpoint: Option<String>,
    /// LLM 配置（使用共享或独立）
    #[serde(default)]
    pub feature_override: LlmFeatureConfig,
}

fn default_observation_duration_secs() -> u64 {
    15
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            observation_duration_secs: default_observation_duration_secs(),
            llm_endpoint: None,
            feature_override: LlmFeatureConfig::default(),
        }
    }
}

// ============================================================================
// TNL 技术规范化层配置
// ============================================================================

/// TNL 技术规范化层配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TnlConfig {
    /// 是否启用 TNL（默认启用）
    #[serde(default = "default_enable_tnl")]
    pub enabled: bool,
    /// 口语流畅化清洗模式
    #[serde(default = "legacy_disfluency_mode")]
    pub disfluency_mode: crate::tnl::DisfluencyMode,
    /// 个性化纠错对精确文本 Pass 开关
    #[serde(default)]
    pub enable_personalization_exact_text_pass: bool,
    /// 个性化音节格/alias Pass 开关
    #[serde(default)]
    pub enable_personalization_syllable_match_pass: bool,
    /// 将学习到的纠错对作为 ASR 热词；旧配置必须主动开启。
    #[serde(default)]
    pub enable_personalization_hotwords: bool,
    /// 将当前输入窗口与近期历史作为 ASR 上下文热词；旧配置必须主动开启。
    #[serde(default)]
    pub enable_context_hotwords: bool,
    /// 个性化窗口最大 token 数
    #[serde(default = "default_personalization_max_window_tokens")]
    pub personalization_max_window_tokens: usize,
    /// 个性化本地自动应用阈值
    #[serde(default = "default_personalization_apply_threshold")]
    pub personalization_apply_threshold: f32,
}

// Deserialization restores old behavior; Default is reserved for a new installation.
fn legacy_tnl_config() -> TnlConfig {
    TnlConfig {
        disfluency_mode: legacy_disfluency_mode(),
        enable_personalization_exact_text_pass: false,
        enable_personalization_syllable_match_pass: false,
        enable_personalization_hotwords: false,
        enable_context_hotwords: false,
        ..TnlConfig::default()
    }
}

fn legacy_disfluency_mode() -> crate::tnl::DisfluencyMode {
    crate::tnl::DisfluencyMode::Off
}

fn default_enable_tnl() -> bool {
    true
}

fn default_enable_personalization_exact_text_pass() -> bool {
    true
}

fn default_enable_personalization_syllable_match_pass() -> bool {
    true
}

fn default_personalization_max_window_tokens() -> usize {
    5
}

fn default_personalization_apply_threshold() -> f32 {
    0.88
}

impl Default for TnlConfig {
    fn default() -> Self {
        let enable_personalization_exact_text_pass =
            default_enable_personalization_exact_text_pass();
        let enable_personalization_syllable_match_pass =
            default_enable_personalization_syllable_match_pass();

        Self {
            enabled: default_enable_tnl(),
            disfluency_mode: crate::tnl::DisfluencyMode::default(),
            enable_personalization_exact_text_pass,
            enable_personalization_syllable_match_pass,
            enable_personalization_hotwords: true,
            enable_context_hotwords: false,
            personalization_max_window_tokens: default_personalization_max_window_tokens(),
            personalization_apply_threshold: default_personalization_apply_threshold(),
        }
    }
}

impl LearningConfig {
    /// 解析 LLM 配置（兼容旧的 llm_endpoint 字段）
    pub fn resolve_llm(&self, shared: &SharedLlmConfig) -> ResolvedLlmClientConfig {
        // 向后兼容：如果 feature_override 没有设置 endpoint，但 llm_endpoint 有值，则使用 llm_endpoint
        let mut cfg = self.feature_override.clone();
        if cfg.endpoint.is_none() && self.llm_endpoint.is_some() {
            cfg.endpoint = self.llm_endpoint.clone();
        }
        cfg.resolve_with_feature(shared, "learning")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPreset {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    /// Per-preset provider override (optional).
    /// When `Some`, this preset uses the given provider instead of the polishing default.
    /// When `None`, falls back to feature-level / shared default chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// Per-preset model override (optional).
    /// Invariant: `model.is_some()` requires `provider_id.is_some()` (state ④ banned).
    /// Migration 9 cleans up violations on load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<LlmReasoningConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    #[default]
    Default,
    None,
    Auto,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LlmReasoningConfig {
    #[serde(default)]
    pub effort: ReasoningEffort,
}

// ============================================================================
// 共享 LLM 配置（新增）
// ============================================================================

/// LLM 提供商
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvider {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub api_key: String,
    pub default_model: String,
}

/// 共享 LLM 配置
///
/// 此配置将被语音润色、AI 助手、自学习词库共享使用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedLlmConfig {
    /// Provider 列表
    #[serde(default)]
    pub providers: Vec<LlmProvider>,
    /// 默认 Provider ID
    #[serde(default)]
    pub default_provider_id: String,

    /// 功能默认绑定 (可选,留空则使用 default_provider_id)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polishing_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_provider_id: Option<String>,

    /// 向后兼容字段 (迁移后可删除)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// 语句润色专用模型（可选，留空则使用 default_model）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polishing_model: Option<String>,
    /// AI 助手专用模型（可选，留空则使用 default_model）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_model: Option<String>,
    /// 自动词库学习专用模型（可选，留空则使用 default_model）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_model: Option<String>,
}

impl Default for SharedLlmConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            default_provider_id: String::new(),
            polishing_provider_id: None,
            assistant_provider_id: None,
            learning_provider_id: None,
            endpoint: None,
            api_key: None,
            default_model: None,
            polishing_model: None,
            assistant_model: None,
            learning_model: None,
        }
    }
}

impl SharedLlmConfig {
    /// 获取指定 Provider
    pub fn get_provider(&self, provider_id: &str) -> Option<&LlmProvider> {
        self.providers.iter().find(|p| p.id == provider_id)
    }

    /// 获取指定功能的模型（如果功能模型未设置，则返回默认模型）
    /// 注意：此方法用于向后兼容，新代码应使用 resolve_with_feature
    pub fn get_feature_model(&self, feature: &str) -> String {
        match feature {
            "polishing" => self
                .polishing_model
                .clone()
                .unwrap_or_else(|| self.default_model.clone().unwrap_or_else(default_llm_model)),
            "assistant" => self
                .assistant_model
                .clone()
                .unwrap_or_else(|| self.default_model.clone().unwrap_or_else(default_llm_model)),
            "learning" => self
                .learning_model
                .clone()
                .unwrap_or_else(|| self.default_model.clone().unwrap_or_else(default_llm_model)),
            _ => self.default_model.clone().unwrap_or_else(default_llm_model),
        }
    }

    /// 获取指定功能的专用模型（不 fallback 到默认模型）
    pub fn get_feature_model_option(&self, feature: &str) -> Option<String> {
        match feature {
            "polishing" => self.polishing_model.clone(),
            "assistant" => self.assistant_model.clone(),
            "learning" => self.learning_model.clone(),
            _ => None,
        }
    }
}

fn default_use_shared_llm() -> bool {
    true
}

/// 功能特定 LLM 配置
///
/// 每个功能可以选择使用共享配置或独立配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmFeatureConfig {
    /// 是否使用共享配置
    #[serde(default = "default_use_shared_llm")]
    pub use_shared: bool,
    /// 共享模式: 指定 Provider ID (可选,留空则使用功能默认或全局默认)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// 独立端点（如果 use_shared=false）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// 独立 API Key（如果 use_shared=false）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// 模型覆盖（共享模式或独立模式都可用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<LlmReasoningConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_body: Option<serde_json::Value>,
}

impl Default for LlmFeatureConfig {
    fn default() -> Self {
        Self {
            use_shared: true,
            provider_id: None,
            endpoint: None,
            api_key: None,
            model: None,
            reasoning: None,
            custom_body: None,
        }
    }
}

/// 解析后的 LLM 客户端配置
///
/// 用于实际调用 LLM API
#[derive(Debug, Clone)]
pub struct ResolvedLlmClientConfig {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

impl LlmFeatureConfig {
    fn has_connection_override(&self) -> bool {
        !self.use_shared
            || self.provider_id.is_some()
            || self.endpoint.is_some()
            || self.api_key.is_some()
            || self.model.is_some()
    }

    /// 解析配置：根据 use_shared 决定使用共享配置还是独立配置
    pub fn resolve(&self, shared: &SharedLlmConfig) -> ResolvedLlmClientConfig {
        self.resolve_with_feature(shared, "")
    }

    /// 解析配置（带功能名称）：根据 use_shared 决定使用共享配置还是独立配置
    /// feature: "polishing" | "assistant" | "learning" | ""
    ///
    /// 共享模式优先级：
    /// 1. Feature 的 provider_id > 功能默认绑定 > 全局 default_provider_id
    /// 2. Feature 的 model > 功能默认 model > Provider 的 default_model
    pub fn resolve_with_feature(
        &self,
        shared: &SharedLlmConfig,
        feature: &str,
    ) -> ResolvedLlmClientConfig {
        if !self.use_shared {
            // 独立模式：使用独立配置
            return ResolvedLlmClientConfig {
                endpoint: normalize_chat_completions_endpoint(
                    &self.endpoint.clone().unwrap_or_default(),
                ),
                api_key: self.api_key.clone().unwrap_or_default(),
                model: self.model.clone().unwrap_or_default(),
            };
        }

        // 共享模式：检查是否有 Provider 配置
        if !shared.providers.is_empty() {
            // 新模式：使用 Provider Registry

            // 确定使用哪个 Provider
            let provider_id = self
                .provider_id
                .as_deref()
                .or_else(|| match feature {
                    "polishing" => shared.polishing_provider_id.as_deref(),
                    "assistant" => shared.assistant_provider_id.as_deref(),
                    "learning" => shared.learning_provider_id.as_deref(),
                    _ => None,
                })
                .unwrap_or(&shared.default_provider_id);

            // 查找 Provider
            if let Some(provider) = shared.get_provider(provider_id) {
                // 确定使用哪个模型
                // Provider Registry 模式下不使用 self.model（该字段仅用于独立模式）
                // 优先级：shared 的功能专用模型 > provider.default_model
                let model = match feature {
                    "polishing" => shared.polishing_model.clone(),
                    "assistant" => shared.assistant_model.clone(),
                    "learning" => shared.learning_model.clone(),
                    _ => None,
                }
                .unwrap_or_else(|| provider.default_model.clone());

                return ResolvedLlmClientConfig {
                    endpoint: normalize_chat_completions_endpoint(&provider.endpoint),
                    api_key: provider.api_key.clone(),
                    model,
                };
            }

            // Provider 不存在，尝试使用第一个 Provider (降级策略)
            if let Some(first_provider) = shared.providers.first() {
                let model = shared
                    .get_feature_model_option(feature)
                    .unwrap_or_else(|| first_provider.default_model.clone());

                return ResolvedLlmClientConfig {
                    endpoint: normalize_chat_completions_endpoint(&first_provider.endpoint),
                    api_key: first_provider.api_key.clone(),
                    model,
                };
            }
        }

        // 旧模式（向后兼容）：使用旧字段
        let default_model = if !feature.is_empty() {
            shared.get_feature_model(feature)
        } else {
            shared
                .default_model
                .clone()
                .unwrap_or_else(default_llm_model)
        };

        ResolvedLlmClientConfig {
            endpoint: normalize_chat_completions_endpoint(
                &self.endpoint.clone().unwrap_or_else(|| {
                    shared.endpoint.clone().unwrap_or_else(default_llm_endpoint)
                }),
            ),
            api_key: self
                .api_key
                .clone()
                .unwrap_or_else(|| shared.api_key.clone().unwrap_or_default()),
            model: self.model.clone().unwrap_or(default_model),
        }
    }

    /// 检查配置是否有效（结合共享配置）
    pub fn is_valid_with_shared(&self, shared: &SharedLlmConfig) -> bool {
        let resolved = self.resolve(shared);
        !resolved.endpoint.trim().is_empty()
            && !resolved.model.trim().is_empty()
            && !resolved.api_key.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// 共享 LLM 配置
    #[serde(default)]
    pub shared: SharedLlmConfig,
    /// 语音润色特定配置
    #[serde(default)]
    pub feature_override: LlmFeatureConfig,
    /// 预设列表
    #[serde(default = "default_presets")]
    pub presets: Vec<LlmPreset>,
    /// 当前选中的预设ID
    #[serde(default = "default_active_preset_id")]
    pub active_preset_id: String,
}

impl LlmConfig {
    /// Migration 9 helper: cleanup preset state ④ violations
    /// (`model.is_some() && provider_id.is_none()`).
    ///
    /// Returns `true` if any preset was cleaned (caller should set `migrated = true`).
    /// Extracted as a public helper so it can be unit-tested directly without
    /// going through `AppConfig::load` (which reads from disk).
    pub fn cleanup_preset_state_invariant(&mut self) -> bool {
        let mut cleaned = false;
        for preset in self.presets.iter_mut() {
            if preset.model.is_some() && preset.provider_id.is_none() {
                tracing::warn!(
                    "preset {} 违反不变量（model={:?} 但 provider_id=None），清理 model 字段",
                    preset.id,
                    preset.model
                );
                preset.model = None;
                cleaned = true;
            }
        }
        cleaned
    }

    /// 解析语音润色配置
    ///
    /// Priority chain:
    /// 1. Active preset's `provider_id` (if set and points to an existing provider) → preset.model
    ///    or provider.default_model. **Skips `shared.polishing_model`** (avoids badge-vs-behavior mismatch).
    /// 2. Preset.provider_id absent or dangling → fallthrough to original chain
    ///    `feature_override.resolve_with_feature(&shared, "polishing")`.
    ///
    /// Note: `resolve_with_feature` is shared by polishing/assistant/learning, so preset awareness
    /// is contained here in `LlmConfig` rather than leaking the preset concept to the generic method.
    pub fn resolve_polishing(&self) -> ResolvedLlmClientConfig {
        if let Some(preset) = self.presets.iter().find(|p| p.id == self.active_preset_id) {
            if let Some(provider_id) = preset.provider_id.as_deref() {
                if let Some(provider) = self.shared.get_provider(provider_id) {
                    let model = preset
                        .model
                        .clone()
                        .unwrap_or_else(|| provider.default_model.clone());
                    return ResolvedLlmClientConfig {
                        endpoint: normalize_chat_completions_endpoint(&provider.endpoint),
                        api_key: provider.api_key.clone(),
                        model,
                    };
                }
                tracing::warn!(
                    "preset {} 指向不存在的 provider {}，降级到默认链",
                    preset.id,
                    provider_id
                );
            }
        }

        self.feature_override
            .resolve_with_feature(&self.shared, "polishing")
    }
}

fn default_llm_endpoint() -> String {
    "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
}

/// Normalize OpenAI-compatible chat completions endpoint.
///
/// Users/providers may provide either:
/// - Base URL (e.g. https://api.openai.com/v1)
/// - Full endpoint (e.g. https://api.openai.com/v1/chat/completions)
///
/// This helper ensures we always end up with a usable `/chat/completions` endpoint.
pub fn normalize_chat_completions_endpoint(endpoint: &str) -> String {
    let mut e = endpoint.trim().to_string();
    if e.is_empty() {
        return e;
    }

    // Strip trailing slashes
    while e.ends_with('/') {
        e.pop();
    }

    // Already looks like a completions endpoint
    if e.ends_with("/chat/completions") {
        return e;
    }

    // Tolerate the common typo: /chat.completions
    if e.ends_with("/chat.completions") {
        return e.replace("/chat.completions", "/chat/completions");
    }

    format!("{}/chat/completions", e)
}

fn default_llm_model() -> String {
    "glm-4-flash-250414".to_string()
}

// 默认预设生成逻辑
fn default_presets() -> Vec<LlmPreset> {
    vec![
        LlmPreset {
            id: "polishing".to_string(),
            name: "文本润色".to_string(),
            system_prompt: "你是一个语音转写润色助手。请在不改变原意的前提下：1）删除重复或意义相近的句子；2）合并同一主题的内容；3）去除「嗯」「啊」等口头禅；4）保留数字与关键信息；5）相关数字和时间不要使用中文；6）整理成自然的段落。输出纯文本即可。".to_string(),
            provider_id: None,
            model: None,
            reasoning: None,
            custom_body: None,
        },
        LlmPreset {
            id: "translation".to_string(),
            name: "中译英".to_string(),
            system_prompt: "你是一个专业的翻译助手。请将用户的中文语音转写内容翻译成地道、流畅的英文。不要输出任何解释性文字，只输出翻译结果。".to_string(),
            provider_id: None,
            model: None,
            reasoning: None,
            custom_body: None,
        }
    ]
}

fn default_active_preset_id() -> String {
    "polishing".to_string()
}

// ============================================================================
// Smart Command 配置
// ============================================================================

/// Smart Command 默认系统提示词（问答模式）
pub const DEFAULT_SMART_COMMAND_PROMPT: &str = r#"你是一个智能语音助手。用户会通过语音向你提问，你需要：
1. 理解用户的问题
2. 给出简洁、准确、有用的回答
3. 如果问题不够明确，给出最可能的解答

注意：
- 回答要简洁明了，适合直接粘贴使用
- 避免过多的解释和废话
- 如果是代码相关问题，直接给出代码"#;

/// AI 助手默认系统提示词 - 问答模式（无选中文本）
pub const DEFAULT_ASSISTANT_QA_PROMPT: &str = r#"你是一个智能语音助手。用户会通过语音向你提问，你需要：
1. 理解用户的问题
2. 给出简洁、准确、有用的回答
3. 如果问题不够明确，给出最可能的解答

注意：
- 回答要简洁明了，适合直接粘贴使用
- 避免过多的解释和废话
- 如果是代码相关问题，直接给出代码"#;

/// AI 助手默认系统提示词 - 文本处理模式（有选中文本）
pub const DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT: &str = r#"你是一个选区上下文助手。用户会选中一段文本，然后用语音或文字提出问题/指令。你需要：
1. 始终先阅读【本轮选中文本（主要上下文）】，把它作为本轮回答的主要依据。
2. 判断用户意图：是编辑类任务，还是基于选中文本回答问题、解释、分析、提建议。
3. 编辑类任务（润色、翻译、总结、改写、扩写、修复语法等）：直接输出处理后的文本，不要添加“这是修改后的版本”等前缀。
4. 问答/解释/分析类任务：围绕选中文本给出清晰回答，可以引用关键点，但不要脱离选区泛泛回答。
5. 保持原文格式和结构，除非用户明确要求改变。

如果指令不明确，优先根据选中文本给出最可能有用的处理结果；确实无法判断时，只提出一个简短澄清问题。"#;

const LEGACY_ASSISTANT_TEXT_PROCESSING_PROMPT: &str = r#"你是一个文本处理专家。用户选中了一段文本，并给出了处理指令，你需要：
1. 根据用户的指令对文本进行相应处理（润色、翻译、解释、修改等）
2. 直接输出处理后的结果，不要添加多余的解释
3. 保持原文的格式和结构（除非用户要求改变）

常见任务示例：
- "润色" / "改得更专业" → 优化表达，提升文笔
- "翻译成英文" → 输出英文翻译结果
- "解释这段代码" → 用简洁的语言说明代码功能
- "修复语法错误" → 纠正错别字和语法问题
- "总结" → 提炼核心要点

注意：直接输出处理结果，不要添加"这是修改后的版本"之类的前缀。"#;

#[cfg(test)]
const LEGACY_FRONTEND_ASSISTANT_TEXT_PROCESSING_PROMPT: &str = r#"你是一个文本处理助手。用户会选中一段文本，然后通过语音告诉你要如何处理这段文本。
你的任务：
1. 理解用户的语音指令
2. 对选中的文本执行相应操作（润色、翻译、总结、改写等）
3. 直接输出处理后的文本
注意：
- 只输出处理后的结果，不要输出任何解释
- 保持原文的格式和结构（除非用户要求改变）
- 如果指令不明确，按最合理的方式处理"#;

/// Smart Command 独立配置（保留向后兼容）
///
/// 与 LLM 润色模块完全独立，拥有自己的 API 配置和系统提示词
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCommandConfig {
    /// 是否启用 Smart Command 模式
    #[serde(default)]
    pub enabled: bool,
    /// API 端点
    #[serde(default = "default_smart_command_endpoint")]
    pub endpoint: String,
    /// 模型名称
    #[serde(default = "default_smart_command_model")]
    pub model: String,
    /// API Key
    #[serde(default)]
    pub api_key: String,
    /// 系统提示词
    #[serde(default = "default_smart_command_prompt")]
    pub system_prompt: String,
}

/// AI 助手配置（新增，取代 SmartCommandConfig）
///
/// 支持双系统提示词：问答模式和文本处理模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantConfig {
    /// 是否启用 AI 助手模式
    #[serde(default)]
    pub enabled: bool,
    /// LLM 配置（使用共享或独立）
    #[serde(default)]
    pub llm: LlmFeatureConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qa_llm: Option<LlmFeatureConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_processing_llm: Option<LlmFeatureConfig>,
    /// 问答模式系统提示词（无选中文本时使用）
    #[serde(default = "default_assistant_qa_prompt")]
    pub qa_system_prompt: String,
    /// 文本处理模式系统提示词（有选中文本时使用）
    #[serde(default = "legacy_assistant_text_processing_prompt")]
    pub text_processing_system_prompt: String,
    /// AI 助手是否允许联网搜索
    #[serde(default)]
    pub enable_web_search: bool,
    /// function calling 最多工具循环次数
    #[serde(default = "default_web_search_max_loops")]
    pub web_search_max_loops: u32,
    /// 文本处理模式是否也允许联网搜索
    #[serde(default)]
    pub web_search_in_text_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchProviderType {
    Tavily,
    Bocha,
    Serper,
    Searxng,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProviderConfig {
    pub id: String,
    pub provider_type: SearchProviderType,
    pub display_name: String,
    #[serde(default = "default_search_provider_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_auth_username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_auth_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serper_gl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serper_hl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serper_tbs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searxng_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searxng_time_range: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default)]
    pub providers: Vec<SearchProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider_id: Option<String>,
    #[serde(default = "default_search_max_results")]
    pub max_results: u32,
    #[serde(default = "default_search_timeout_secs")]
    pub timeout_secs: u32,
    #[serde(default = "default_search_enable_fallback")]
    pub enable_fallback: bool,
}

fn default_smart_command_endpoint() -> String {
    "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
}

fn default_smart_command_model() -> String {
    "glm-4-flash-250414".to_string()
}

fn default_smart_command_prompt() -> String {
    DEFAULT_SMART_COMMAND_PROMPT.to_string()
}

fn default_assistant_qa_prompt() -> String {
    DEFAULT_ASSISTANT_QA_PROMPT.to_string()
}

fn default_assistant_text_processing_prompt() -> String {
    DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT.to_string()
}

fn legacy_assistant_text_processing_prompt() -> String {
    LEGACY_ASSISTANT_TEXT_PROCESSING_PROMPT.to_string()
}

fn legacy_assistant_config() -> AssistantConfig {
    AssistantConfig {
        text_processing_system_prompt: legacy_assistant_text_processing_prompt(),
        ..AssistantConfig::default()
    }
}

fn default_web_search_max_loops() -> u32 {
    3
}

fn default_search_provider_enabled() -> bool {
    true
}

fn default_search_max_results() -> u32 {
    5
}

fn default_search_timeout_secs() -> u32 {
    6
}

fn default_search_enable_fallback() -> bool {
    true
}

impl Default for SmartCommandConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_smart_command_endpoint(),
            model: default_smart_command_model(),
            api_key: String::new(),
            system_prompt: default_smart_command_prompt(),
        }
    }
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            llm: LlmFeatureConfig::default(),
            qa_llm: None,
            text_processing_llm: None,
            qa_system_prompt: default_assistant_qa_prompt(),
            text_processing_system_prompt: default_assistant_text_processing_prompt(),
            enable_web_search: false,
            web_search_max_loops: default_web_search_max_loops(),
            web_search_in_text_mode: false,
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            default_provider_id: None,
            max_results: default_search_max_results(),
            timeout_secs: default_search_timeout_secs(),
            enable_fallback: default_search_enable_fallback(),
        }
    }
}

impl SmartCommandConfig {
    /// 检查配置是否有效（API Key 已填写）
    pub fn is_valid(&self) -> bool {
        !self.api_key.is_empty() && !self.endpoint.is_empty() && !self.model.is_empty()
    }
}

impl AssistantConfig {
    fn merge_mode_llm_config(&self, mode_config: Option<&LlmFeatureConfig>) -> LlmFeatureConfig {
        let Some(mode_config) = mode_config else {
            return self.llm.clone();
        };

        if mode_config.has_connection_override() {
            return mode_config.clone();
        }

        let mut merged = self.llm.clone();
        merged.reasoning = mode_config.reasoning.clone();
        merged.custom_body = mode_config.custom_body.clone();
        merged
    }

    pub fn qa_feature_config(&self) -> LlmFeatureConfig {
        self.merge_mode_llm_config(self.qa_llm.as_ref())
    }

    pub fn text_processing_feature_config(&self) -> LlmFeatureConfig {
        self.merge_mode_llm_config(self.text_processing_llm.as_ref())
    }

    pub fn resolve_qa_llm(&self, shared: &SharedLlmConfig) -> ResolvedLlmClientConfig {
        self.qa_feature_config()
            .resolve_with_feature(shared, "assistant")
    }

    pub fn resolve_text_processing_llm(&self, shared: &SharedLlmConfig) -> ResolvedLlmClientConfig {
        self.text_processing_feature_config()
            .resolve_with_feature(shared, "assistant")
    }

    /// 检查配置是否有效（结合共享配置）
    pub fn is_valid_with_shared(&self, shared: &SharedLlmConfig) -> bool {
        self.qa_feature_config().is_valid_with_shared(shared)
            && self
                .text_processing_feature_config()
                .is_valid_with_shared(shared)
    }
}

// 为了兼容旧版本配置，如果反序列化时 presets 为空，手动填充默认值
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            shared: SharedLlmConfig::default(),
            feature_override: LlmFeatureConfig::default(),
            presets: default_presets(),
            active_preset_id: default_active_preset_id(),
        }
    }
}

fn default_use_realtime_asr() -> bool {
    false
}

fn default_enable_dictionary_enhancement() -> bool {
    false
}

fn migrate_legacy_llm_registry(config: &mut AppConfig) -> bool {
    let old = config.llm_config.shared.clone();
    if !old.providers.is_empty()
        || old.endpoint.is_none()
        || old.api_key.is_none()
        || old.default_model.is_none()
    {
        return false;
    }

    fn provider_id(shared: &mut SharedLlmConfig, resolved: &ResolvedLlmClientConfig) -> String {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(resolved.endpoint.as_bytes());
        hash.update(b"|");
        hash.update(resolved.api_key.as_bytes());
        let id = format!("{:x}", hash.finalize())[..12].to_string();
        if shared.get_provider(&id).is_none() {
            shared.providers.push(LlmProvider {
                id: id.clone(),
                name: format!("旧配置提供商 {}", shared.providers.len() + 1),
                endpoint: resolved.endpoint.clone(),
                api_key: resolved.api_key.clone(),
                default_model: resolved.model.clone(),
            });
        }
        id
    }

    let shared = &mut config.llm_config.shared;
    let base = LlmFeatureConfig::default().resolve(&old);
    shared.default_provider_id = provider_id(shared, &base);
    for (name, feature) in [
        ("polishing", &mut config.llm_config.feature_override),
        ("assistant", &mut config.assistant_config.llm),
        ("learning", &mut config.learning_config.feature_override),
    ] {
        // Independent connections already have complete semantics; leave them untouched.
        if !feature.use_shared {
            continue;
        }
        let resolved = feature.resolve_with_feature(&old, name);
        let id = provider_id(shared, &resolved);
        match name {
            "polishing" => {
                shared.polishing_provider_id = Some(id);
                shared.polishing_model = Some(resolved.model);
            }
            "assistant" => {
                shared.assistant_provider_id = Some(id);
                shared.assistant_model = Some(resolved.model);
            }
            _ => {
                shared.learning_provider_id = Some(id);
                shared.learning_model = Some(resolved.model);
            }
        }
        feature.provider_id = None;
        feature.endpoint = None;
        feature.api_key = None;
        feature.model = None;
    }

    // Mode-specific connections are optional. Reasoning-only overrides still inherit
    // the migrated assistant connection; explicit old overrides retain their full tuple.
    for mode in [
        &mut config.assistant_config.qa_llm,
        &mut config.assistant_config.text_processing_llm,
    ] {
        if let Some(feature) = mode {
            if feature.use_shared && feature.has_connection_override() {
                let resolved = feature.resolve_with_feature(&old, "assistant");
                feature.use_shared = false;
                feature.provider_id = None;
                feature.endpoint = Some(resolved.endpoint);
                feature.api_key = Some(resolved.api_key);
                feature.model = Some(resolved.model);
            }
        }
    }
    true
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            dashscope_api_key: String::new(),
            siliconflow_api_key: String::new(),
            asr_config: AsrConfig::default(),
            use_realtime_asr: default_use_realtime_asr(),
            enable_llm_post_process: false,
            enable_dictionary_enhancement: default_enable_dictionary_enhancement(),
            llm_config: LlmConfig::default(),
            smart_command_config: SmartCommandConfig::default(),
            assistant_config: AssistantConfig::default(),
            search_config: SearchConfig::default(),
            learning_config: LearningConfig::default(),
            tnl_config: TnlConfig::default(),
            close_action: None,
            hotkey_config: None,
            dual_hotkey_config: DualHotkeyConfig::default(),
            transcription_mode: TranscriptionMode::default(),
            enable_mute_other_apps: false,
            dictionary: Vec::new(),
            builtin_dictionary_domains: Vec::new(),
            theme: default_theme(),
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = config_dir.join("PushToTalk");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    pub fn backfill_dictionary_categories(&mut self) -> bool {
        crate::dictionary_utils::backfill_inferred_categories(&mut self.dictionary)
    }

    pub fn load() -> Result<(Self, bool)> {
        Self::load_from_path(&Self::config_path()?)
    }

    pub(crate) fn load_from_path(path: &Path) -> Result<(Self, bool)> {
        tracing::info!("尝试从以下路径加载配置: {:?}", path);

        // 跟踪是否发生了迁移（调用者可根据此决定是否保存）
        // The old writer moved the canonical file to .bak before replacing it.
        // Recover only if the canonical file is absent; never hide its parse errors.
        let backup_path = path.with_extension("json.bak");
        let recovered_backup = !path.try_exists()? && backup_path.try_exists()?;
        let source = if recovered_backup {
            backup_path.as_path()
        } else {
            path
        };
        let mut migrated = recovered_backup;

        if source.try_exists()? {
            let content = std::fs::read_to_string(source)?;

            // 使用 serde_json::Value 先解析，以支持结构迁移
            let v: serde_json::Value = serde_json::from_str(&content)?;

            // Unknown/invalid fields must not silently reset unrelated settings or keys.
            // Known historical fields are still accepted and migrated below.
            let mut config: AppConfig = serde_json::from_value(v.clone())
                .context("配置格式不受支持；原配置已保留，请检查配置字段")?;

            if v.get("asr_config")
                .and_then(|asr| asr.get("qwen_profile"))
                .is_none()
            {
                config.asr_config.qwen_profile = legacy_qwen_profile();
                migrated = true;
            }
            if v.get("tnl_config")
                .and_then(|tnl| tnl.get("disfluency_mode"))
                .is_none()
            {
                config.tnl_config.disfluency_mode = legacy_disfluency_mode();
                migrated = true;
            }
            // Before provider selection existed, DashScope was the primary ASR.
            if v.get("asr_config").is_none() {
                if !config.dashscope_api_key.trim().is_empty() {
                    config.asr_config.selection.active_provider = AsrProvider::Qwen;
                } else if !config.siliconflow_api_key.trim().is_empty() {
                    config.asr_config.selection.active_provider = AsrProvider::SiliconFlow;
                }
            }

            // ========== 迁移逻辑 ==========

            // 1. 兼容更早的根目录 Key (dashscope_api_key / siliconflow_api_key)
            if config.asr_config.credentials.qwen_api_key.is_empty()
                && !config.dashscope_api_key.is_empty()
            {
                tracing::info!("从根配置迁移 Qwen API Key");
                config.asr_config.credentials.qwen_api_key = config.dashscope_api_key.clone();
                migrated = true;
            }
            if config.asr_config.credentials.sensevoice_api_key.is_empty()
                && !config.siliconflow_api_key.is_empty()
            {
                tracing::info!("从根配置迁移 SiliconFlow API Key");
                config.asr_config.credentials.sensevoice_api_key =
                    config.siliconflow_api_key.clone();
                migrated = true;
            }

            // 迁移 2: LLM 配置统一化（旧的扁平结构 → 新的 shared + feature_override 结构）
            if let Some(llm_cfg) = v.get("llm_config") {
                let has_legacy_fields = llm_cfg.get("shared").is_none()
                    && (llm_cfg.get("endpoint").is_some()
                        || llm_cfg.get("api_key").is_some()
                        || llm_cfg.get("model").is_some());

                if has_legacy_fields {
                    tracing::info!("检测到旧版 LLM 配置格式，开始迁移");
                    migrated = true;

                    // 迁移到 shared 配置
                    if let Some(endpoint) = llm_cfg.get("endpoint").and_then(|v| v.as_str()) {
                        if !endpoint.trim().is_empty() {
                            tracing::info!(
                                "迁移 llm_config.endpoint -> llm_config.shared.endpoint"
                            );
                            config.llm_config.shared.endpoint = Some(endpoint.to_string());
                        }
                    }
                    if let Some(model) = llm_cfg.get("model").and_then(|v| v.as_str()) {
                        if !model.trim().is_empty() {
                            tracing::info!(
                                "迁移 llm_config.model -> llm_config.shared.default_model"
                            );
                            config.llm_config.shared.default_model = Some(model.to_string());
                        }
                    }
                    if let Some(api_key) = llm_cfg.get("api_key").and_then(|v| v.as_str()) {
                        if !api_key.trim().is_empty() {
                            tracing::info!("迁移 llm_config.api_key -> llm_config.shared.api_key");
                            config.llm_config.shared.api_key = Some(api_key.to_string());
                        }
                    }
                }
            }

            // 迁移 3: AssistantConfig 配置统一化
            if let Some(assistant_cfg) = v.get("assistant_config") {
                let has_legacy_fields = assistant_cfg.get("llm").is_none()
                    && (assistant_cfg.get("endpoint").is_some()
                        || assistant_cfg.get("api_key").is_some()
                        || assistant_cfg.get("model").is_some());

                if has_legacy_fields {
                    tracing::info!("检测到旧版 AI 助手配置格式，开始迁移");
                    migrated = true;

                    let endpoint = assistant_cfg
                        .get("endpoint")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let model = assistant_cfg
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let api_key = assistant_cfg
                        .get("api_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !endpoint.trim().is_empty()
                        || !model.trim().is_empty()
                        || !api_key.trim().is_empty()
                    {
                        let shared = &config.llm_config.shared;
                        let matches_shared = shared
                            .endpoint
                            .as_ref()
                            .map(|e| e == &endpoint)
                            .unwrap_or(false)
                            && shared
                                .api_key
                                .as_ref()
                                .map(|k| k == &api_key)
                                .unwrap_or(false)
                            && shared
                                .default_model
                                .as_ref()
                                .map(|m| m == &model)
                                .unwrap_or(false);

                        if matches_shared {
                            tracing::info!("AI 助手配置与共享配置相同，使用共享配置");
                            config.assistant_config.llm.use_shared = true;
                        } else {
                            tracing::info!("AI 助手配置与共享配置不同，保留独立配置");
                            config.assistant_config.llm.use_shared = false;
                            if !endpoint.trim().is_empty() {
                                config.assistant_config.llm.endpoint = Some(endpoint);
                            }
                            if !model.trim().is_empty() {
                                config.assistant_config.llm.model = Some(model);
                            }
                            if !api_key.trim().is_empty() {
                                config.assistant_config.llm.api_key = Some(api_key);
                            }
                        }
                    }
                }
            }

            // 迁移 4: LearningConfig 配置统一化
            if let Some(learning_cfg) = v.get("learning_config") {
                let has_legacy_endpoint = learning_cfg.get("feature_override").is_none()
                    && learning_cfg.get("llm_endpoint").is_some();

                if has_legacy_endpoint && config.learning_config.feature_override.endpoint.is_none()
                {
                    if let Some(endpoint) =
                        learning_cfg.get("llm_endpoint").and_then(|v| v.as_str())
                    {
                        if !endpoint.trim().is_empty() {
                            tracing::info!("迁移 learning_config.llm_endpoint -> learning_config.feature_override.endpoint");
                            config.learning_config.feature_override.endpoint =
                                Some(endpoint.to_string());
                            let resolved = config
                                .learning_config
                                .resolve_llm(&config.llm_config.shared);
                            // 重要：设置 use_shared=false 以保留原语义
                            // 否则迁移后会优先使用 Provider Registry，导致 endpoint 被忽略
                            config.learning_config.feature_override.use_shared = false;
                            config.learning_config.feature_override.api_key =
                                Some(resolved.api_key);
                            config.learning_config.feature_override.model = Some(resolved.model);
                            migrated = true;
                        }
                    }
                }
            }

            // 迁移 5: 旧单快捷键 → 新双快捷键 (保持原有逻辑)
            if let Some(old_hotkey) = config.hotkey_config.take() {
                // A saved modern choice wins even when it happens to equal the defaults.
                if v.get("dual_hotkey_config").is_none() {
                    tracing::info!(
                        "迁移旧快捷键配置 {} 到听写模式",
                        old_hotkey.format_display()
                    );
                    config.dual_hotkey_config.dictation = old_hotkey;
                    migrated = true;
                }
            }

            // 迁移 6: SmartCommandConfig → AssistantConfig (保持原有逻辑)
            if config.smart_command_config.enabled && config.smart_command_config.is_valid() {
                if !config
                    .assistant_config
                    .is_valid_with_shared(&config.llm_config.shared)
                {
                    tracing::info!("迁移 Smart Command 配置到 AI 助手配置");
                    migrated = true;
                    config.assistant_config = AssistantConfig {
                        enabled: config.smart_command_config.enabled,
                        llm: LlmFeatureConfig {
                            use_shared: false,
                            provider_id: None,
                            endpoint: Some(config.smart_command_config.endpoint.clone()),
                            model: Some(config.smart_command_config.model.clone()),
                            api_key: Some(config.smart_command_config.api_key.clone()),
                            reasoning: None,
                            custom_body: None,
                        },
                        qa_llm: None,
                        text_processing_llm: None,
                        qa_system_prompt: config.smart_command_config.system_prompt.clone(),
                        text_processing_system_prompt: legacy_assistant_text_processing_prompt(),
                        enable_web_search: false,
                        web_search_max_loops: default_web_search_max_loops(),
                        web_search_in_text_mode: false,
                    };
                    config.smart_command_config.enabled = false;
                }
            }

            // 迁移 7: 先按旧规则解析每个功能，再写入 Registry，不能只迁移共享默认值。
            if migrate_legacy_llm_registry(&mut config) {
                migrated = true;
            }

            // 迁移 8: 清理 Provider Registry 模式下遗留的 feature model 覆盖
            // 迁移 7 遗漏了清理 model 字段，导致旧的 model 值覆盖 provider.default_model
            if !config.llm_config.shared.providers.is_empty() {
                let mut cleaned = false;
                if config.llm_config.feature_override.use_shared
                    && config.llm_config.feature_override.model.is_some()
                {
                    tracing::info!(
                        "清理 feature_override 遗留的 model 值: {:?}",
                        config.llm_config.feature_override.model
                    );
                    config.llm_config.feature_override.model = None;
                    cleaned = true;
                }
                if config.assistant_config.llm.use_shared
                    && config.assistant_config.llm.model.is_some()
                {
                    tracing::info!(
                        "清理 assistant_config.llm 遗留的 model 值: {:?}",
                        config.assistant_config.llm.model
                    );
                    config.assistant_config.llm.model = None;
                    cleaned = true;
                }
                if cleaned {
                    migrated = true;
                }
            }

            // 迁移 9: 清理 LlmPreset state ④（model.is_some() 且 provider_id.is_none()）
            // 该状态违反不变量：preset.model 必须依附于 preset.provider_id
            // 仅在手工编辑 config.json 后可能出现；前端 popover 守护正常路径不会产生
            if config.llm_config.cleanup_preset_state_invariant() {
                migrated = true;
            }

            if config.backfill_dictionary_categories() {
                tracing::info!("迁移个人词典 category metadata");
                migrated = true;
            }

            if config.llm_config.presets.is_empty() {
                tracing::info!("检测到预设列表为空，用户可能删除了所有预设");
            }

            if migrated {
                tracing::info!("配置加载成功（发生迁移，建议保存）");
            } else {
                tracing::info!("配置加载成功");
            }
            Ok((config, migrated))
        } else {
            tracing::warn!("配置文件不存在，创建并返回默认配置");
            Ok((Self::new(), false))
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to_path(&Self::config_path()?)
    }

    pub(crate) fn save_to_path(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        tracing::info!("保存配置到: {:?}", path);

        // Keep the old file in place until the native atomic replacement succeeds.
        // Unique siblings also prevent independent writers from sharing a temp file.
        anyhow::ensure!(!path.is_dir(), "配置路径是目录，无法保存");
        let temp_path = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            drop(file);
            crate::platform::replace_file(&temp_path, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, AsrConfig, AsrLanguageMode, AssistantConfig, LlmConfig, LlmFeatureConfig,
        LlmPreset, LlmReasoningConfig, QwenAsrProfile, ReasoningEffort, SearchConfig,
        SharedLlmConfig, TnlConfig, DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT,
    };

    #[test]
    fn asr_config_defaults_to_auto_language_mode() {
        assert_eq!(AsrConfig::default().language_mode, AsrLanguageMode::Auto);
    }

    #[test]
    fn asr_config_preserves_legacy_qwen3_when_profile_is_absent() {
        let config: AsrConfig = serde_json::from_value(serde_json::json!({
            "credentials": {},
            "selection": {}
        }))
        .expect("旧版 ASR 配置应能迁移");

        assert_eq!(
            serde_json::to_value(config.qwen_profile).unwrap(),
            "qwen3_legacy"
        );
        assert_eq!(config.qwen_profile, QwenAsrProfile::Qwen3Legacy);
    }

    #[test]
    fn asr_config_round_trips_all_three_qwen_profiles() {
        for wire in ["qwen_audio_3_1", "qwen_audio_3", "qwen3_legacy"] {
            let config: AsrConfig = serde_json::from_value(serde_json::json!({
                "credentials": {}, "selection": {}, "qwen_profile": wire
            }))
            .expect("每种模型都必须能通过 save_config IPC 反序列化");
            assert_eq!(serde_json::to_value(config).unwrap()["qwen_profile"], wire);
        }
    }

    #[test]
    fn asr_config_round_trips_qwen_audio_3_ipc_value() {
        let config: AsrConfig = serde_json::from_value(serde_json::json!({
            "credentials": {},
            "selection": {},
            "qwen_profile": "qwen_audio_3"
        }))
        .expect("前端 qwen_audio_3 应能通过 Tauri IPC 反序列化");

        assert_eq!(config.qwen_profile, QwenAsrProfile::QwenAudio3);

        let value = serde_json::to_value(config).expect("ASR 配置应能序列化");
        assert_eq!(value["qwen_profile"], "qwen_audio_3");
    }

    #[test]
    fn asr_config_migrates_previous_qwen_audio3_wire_value() {
        let profile: QwenAsrProfile = serde_json::from_str("\"qwen_audio3\"")
            .expect("修复前可能落盘的 qwen_audio3 应继续可读");

        assert_eq!(profile, QwenAsrProfile::QwenAudio3);
        assert_eq!(
            serde_json::to_string(&profile).expect("Qwen profile 应能序列化"),
            "\"qwen_audio_3\""
        );
    }

    #[test]
    fn asr_config_serializes_legacy_qwen_profile_explicitly() {
        let config = AsrConfig {
            qwen_profile: QwenAsrProfile::Qwen3Legacy,
            ..AsrConfig::default()
        };

        let value = serde_json::to_value(config).expect("ASR 配置应能序列化");
        assert_eq!(value["qwen_profile"], "qwen3_legacy");
    }

    #[test]
    fn search_config_defaults_are_safe_for_legacy_configs() {
        let cfg = SearchConfig::default();

        assert!(cfg.providers.is_empty());
        assert_eq!(cfg.default_provider_id, None);
        assert_eq!(cfg.max_results, 5);
        assert_eq!(cfg.timeout_secs, 6);
        assert!(cfg.enable_fallback);
    }

    #[test]
    fn assistant_web_search_defaults_to_off() {
        let cfg = AssistantConfig::default();

        assert!(!cfg.enable_web_search);
        assert_eq!(cfg.web_search_max_loops, 3);
        assert!(!cfg.web_search_in_text_mode);
    }

    #[test]
    fn assistant_reasoning_override_preserves_base_independent_connection() {
        let mut cfg = AssistantConfig::default();
        cfg.llm = LlmFeatureConfig {
            use_shared: false,
            provider_id: None,
            endpoint: Some("https://assistant.example.com/v1".to_string()),
            api_key: Some("assistant-key".to_string()),
            model: Some("assistant-model".to_string()),
            reasoning: None,
            custom_body: None,
        };
        cfg.qa_llm = Some(LlmFeatureConfig {
            reasoning: Some(LlmReasoningConfig {
                effort: ReasoningEffort::High,
            }),
            ..LlmFeatureConfig::default()
        });

        let resolved = cfg.resolve_qa_llm(&SharedLlmConfig::default());
        let qa_config = cfg.qa_feature_config();

        assert_eq!(
            resolved.endpoint,
            "https://assistant.example.com/v1/chat/completions"
        );
        assert_eq!(resolved.api_key, "assistant-key");
        assert_eq!(resolved.model, "assistant-model");
        assert_eq!(
            qa_config.reasoning.expect("reasoning override").effort,
            ReasoningEffort::High
        );
        assert!(cfg.is_valid_with_shared(&SharedLlmConfig::default()));
    }

    #[test]
    fn assistant_mode_connection_override_replaces_base_connection_when_explicit() {
        let mut cfg = AssistantConfig::default();
        cfg.llm = LlmFeatureConfig {
            use_shared: false,
            provider_id: None,
            endpoint: Some("https://base.example.com/v1".to_string()),
            api_key: Some("base-key".to_string()),
            model: Some("base-model".to_string()),
            reasoning: None,
            custom_body: None,
        };
        cfg.text_processing_llm = Some(LlmFeatureConfig {
            use_shared: false,
            provider_id: None,
            endpoint: Some("https://text.example.com/v1".to_string()),
            api_key: Some("text-key".to_string()),
            model: Some("text-model".to_string()),
            reasoning: Some(LlmReasoningConfig {
                effort: ReasoningEffort::None,
            }),
            custom_body: None,
        });

        let resolved = cfg.resolve_text_processing_llm(&SharedLlmConfig::default());
        let text_config = cfg.text_processing_feature_config();

        assert_eq!(
            resolved.endpoint,
            "https://text.example.com/v1/chat/completions"
        );
        assert_eq!(resolved.api_key, "text-key");
        assert_eq!(resolved.model, "text-model");
        assert_eq!(
            text_config.reasoning.expect("reasoning override").effort,
            ReasoningEffort::None
        );
    }

    #[test]
    fn app_config_legacy_json_backfills_search_fields() {
        let json = r#"{
            "assistant_config": {
                "enabled": true,
                "llm": {"use_shared": true},
                "qa_system_prompt": "qa",
                "text_processing_system_prompt": "tp"
            }
        }"#;

        let cfg: AppConfig = serde_json::from_str(json).expect("旧配置必须能反序列化");

        assert!(!cfg.assistant_config.enable_web_search);
        assert_eq!(cfg.assistant_config.web_search_max_loops, 3);
        assert_eq!(cfg.search_config.max_results, 5);
    }

    #[test]
    fn new_install_uses_context_prompt() {
        assert_eq!(
            AppConfig::new()
                .assistant_config
                .text_processing_system_prompt,
            DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT
        );
    }

    #[test]
    fn tnl_config_defaults_enable_personalization_passes() {
        let cfg = TnlConfig::default();

        assert!(cfg.enabled);
        assert_eq!(
            cfg.disfluency_mode,
            crate::tnl::DisfluencyMode::Conservative
        );
        assert!(cfg.enable_personalization_exact_text_pass);
        assert!(cfg.enable_personalization_syllable_match_pass);
        assert_eq!(cfg.personalization_max_window_tokens, 5);
        assert!((cfg.personalization_apply_threshold - 0.88).abs() < f32::EPSILON);
    }

    #[test]
    fn experimental_context_hotwords_require_opt_in_without_resetting_saved_choice() {
        assert!(!TnlConfig::default().enable_context_hotwords);
        let old: TnlConfig = serde_json::from_str("{}").unwrap();
        assert!(!old.enable_context_hotwords);
        let configured: TnlConfig =
            serde_json::from_str(r#"{"enable_context_hotwords":true}"#).unwrap();
        assert!(configured.enable_context_hotwords);
        let reloaded: TnlConfig =
            serde_json::from_str(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert!(reloaded.enable_context_hotwords);
    }

    #[test]
    fn app_config_legacy_json_backfills_tnl_personalization_fields() {
        let json = r#"{
            "tnl_config": {
                "enabled": true
            }
        }"#;

        let cfg: AppConfig = serde_json::from_str(json).expect("旧配置必须能反序列化");

        assert_eq!(
            cfg.tnl_config.disfluency_mode,
            crate::tnl::DisfluencyMode::Off
        );
        assert!(!cfg.tnl_config.enable_personalization_exact_text_pass);
        assert!(!cfg.tnl_config.enable_personalization_syllable_match_pass);
        assert_eq!(cfg.tnl_config.personalization_max_window_tokens, 5);
        assert!((cfg.tnl_config.personalization_apply_threshold - 0.88).abs() < f32::EPSILON);
    }

    #[test]
    fn app_config_loads_explicit_tnl_disfluency_mode() {
        let json = r#"{
            "tnl_config": {
                "enabled": true,
                "disfluency_mode": "off"
            }
        }"#;

        let cfg: AppConfig = serde_json::from_str(json).expect("配置必须能反序列化");

        assert_eq!(
            cfg.tnl_config.disfluency_mode,
            crate::tnl::DisfluencyMode::Off
        );
    }

    #[test]
    fn app_config_dictionary_backfill_marks_migrated_when_storage_changes() {
        let mut cfg = AppConfig::new();
        cfg.dictionary = vec!["useState|auto".to_string(), "rust|auto".to_string()];

        assert!(cfg.backfill_dictionary_categories());
        assert_eq!(
            cfg.dictionary,
            vec!["useState|auto|code_symbol", "rust|auto"]
        );
    }

    #[test]
    fn app_config_dictionary_backfill_skips_canonical_storage() {
        let mut cfg = AppConfig::new();
        cfg.dictionary = vec![
            "useState|auto|code_symbol".to_string(),
            "rust|auto".to_string(),
        ];

        assert!(!cfg.backfill_dictionary_categories());
        assert_eq!(
            cfg.dictionary,
            vec!["useState|auto|code_symbol", "rust|auto"]
        );
    }

    // ============================================================================
    // PRD per-preset-llm-override — T7-T10 (migration 9 + serde compat)
    // ============================================================================

    /// T7: Migration 9 cleans state ④ violation
    /// (`model.is_some() && provider_id.is_none()` is invalid; cleanup zeroes `model`)
    #[test]
    fn test_load_migration_9_cleans_state_invariant_violation() {
        let mut llm = LlmConfig::default();
        llm.presets = vec![
            LlmPreset {
                id: "p1".to_string(),
                name: "Valid".to_string(),
                system_prompt: String::new(),
                provider_id: Some("some-provider".to_string()),
                model: Some("some-model".to_string()),
                reasoning: None,
                custom_body: None,
            },
            LlmPreset {
                id: "p2".to_string(),
                name: "Invariant Violation".to_string(),
                system_prompt: String::new(),
                provider_id: None, // ← violates invariant
                model: Some("orphan-model".to_string()),
                reasoning: None,
                custom_body: None,
            },
            LlmPreset {
                id: "p3".to_string(),
                name: "Default".to_string(),
                system_prompt: String::new(),
                provider_id: None,
                model: None,
                reasoning: None,
                custom_body: None,
            },
        ];

        let cleaned = llm.cleanup_preset_state_invariant();
        assert!(cleaned, "迁移 9 必须报告做了清理");

        // p1: 不变（合法）
        assert_eq!(llm.presets[0].provider_id.as_deref(), Some("some-provider"));
        assert_eq!(llm.presets[0].model.as_deref(), Some("some-model"));
        // p2: model 被清空
        assert_eq!(llm.presets[1].provider_id, None);
        assert_eq!(llm.presets[1].model, None, "state ④ 的 model 必须被清空");
        // p3: 不变（默认态）
        assert_eq!(llm.presets[2].provider_id, None);
        assert_eq!(llm.presets[2].model, None);
    }

    /// T8: Migration 9 returns false (no migration) when all presets are valid
    /// 用于验证 `migrated=true` 触发条件正确
    #[test]
    fn test_load_migration_9_skips_when_no_violation() {
        let mut llm = LlmConfig::default();
        llm.presets = vec![LlmPreset {
            id: "p1".to_string(),
            name: "Default".to_string(),
            system_prompt: String::new(),
            provider_id: None,
            model: None,
            reasoning: None,
            custom_body: None,
        }];

        let cleaned = llm.cleanup_preset_state_invariant();
        assert!(!cleaned, "无违反时不应触发清理（migrated=false 路径）");
    }

    /// T9: Legacy config (no provider_id/model fields) loads with all presets defaulted to None
    #[test]
    fn test_load_legacy_config_without_preset_fields_unchanged() {
        // Inline JSON literal mimics an old config saved before this feature
        let legacy_json = r#"{
            "id": "polishing",
            "name": "文本润色",
            "system_prompt": "Old prompt"
        }"#;

        let preset: LlmPreset = serde_json::from_str(legacy_json).expect("旧 JSON 必须能反序列化");

        assert_eq!(preset.id, "polishing");
        assert_eq!(preset.name, "文本润色");
        assert_eq!(preset.system_prompt, "Old prompt");
        assert_eq!(
            preset.provider_id, None,
            "旧 JSON 加载后 provider_id 必须为 None"
        );
        assert_eq!(preset.model, None, "旧 JSON 加载后 model 必须为 None");
    }

    /// T10: Serializing a preset with None fields skips them entirely
    /// (skip_serializing_if 生效 → 与旧版 JSON 字节级兼容)
    #[test]
    fn test_save_preset_skips_serializing_none_fields() {
        let preset = LlmPreset {
            id: "p1".to_string(),
            name: "Test".to_string(),
            system_prompt: "prompt".to_string(),
            provider_id: None,
            model: None,
            reasoning: None,
            custom_body: None,
        };

        let json = serde_json::to_string(&preset).expect("序列化必须成功");

        assert!(
            !json.contains("provider_id"),
            "None 时 provider_id 不应出现在 JSON 中: {}",
            json
        );
        assert!(
            !json.contains("\"model\""),
            "None 时 model 不应出现在 JSON 中: {}",
            json
        );
        // 正向：必有字段都在
        assert!(json.contains("\"id\":\"p1\""));
        assert!(json.contains("\"name\":\"Test\""));
    }

    /// 补充 T10b: 有覆盖时序列化必须包含字段
    #[test]
    fn test_save_preset_with_override_serializes_fields() {
        let preset = LlmPreset {
            id: "p1".to_string(),
            name: "Test".to_string(),
            system_prompt: "prompt".to_string(),
            provider_id: Some("prov-1".to_string()),
            model: Some("m1".to_string()),
            reasoning: None,
            custom_body: None,
        };

        let json = serde_json::to_string(&preset).expect("序列化必须成功");

        assert!(
            json.contains("\"provider_id\":\"prov-1\""),
            "Some 时 provider_id 必须出现: {}",
            json
        );
        assert!(
            json.contains("\"model\":\"m1\""),
            "Some 时 model 必须出现: {}",
            json
        );
    }
}

#[cfg(test)]
mod compatibility_tests;

#[cfg(test)]
mod release_upgrade_tests;
