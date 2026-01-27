// src-tauri/src/config.rs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

// 词典相关函数已移至 learning::store 模块

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
    #[serde(rename = "siliconflow")]
    SiliconFlow,
}

impl Default for AsrProvider {
    fn default() -> Self {
        AsrProvider::Qwen
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
            active_provider: AsrProvider::Qwen,
            enable_fallback: false,
            fallback_provider: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    pub credentials: AsrCredentials,
    pub selection: AsrSelection,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            credentials: AsrCredentials::default(),
            selection: AsrSelection::default(),
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
    #[serde(default)]
    pub llm_config: LlmConfig,
    /// Smart Command 独立配置（保留以便向后兼容）
    #[serde(default)]
    pub smart_command_config: SmartCommandConfig,
    /// AI 助手配置（新增）
    #[serde(default)]
    pub assistant_config: AssistantConfig,
    /// 自动词库学习配置
    #[serde(default)]
    pub learning_config: LearningConfig,
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
}

impl Default for LlmFeatureConfig {
    fn default() -> Self {
        Self {
            use_shared: true,
            provider_id: None,
            endpoint: None,
            api_key: None,
            model: None,
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
                let model = self
                    .model
                    .clone()
                    .or_else(|| match feature {
                        "polishing" => shared.polishing_model.clone(),
                        "assistant" => shared.assistant_model.clone(),
                        "learning" => shared.learning_model.clone(),
                        _ => None,
                    })
                    .unwrap_or_else(|| provider.default_model.clone());

                return ResolvedLlmClientConfig {
                    endpoint: normalize_chat_completions_endpoint(&provider.endpoint),
                    api_key: provider.api_key.clone(),
                    model,
                };
            }

            // Provider 不存在，尝试使用第一个 Provider (降级策略)
            if let Some(first_provider) = shared.providers.first() {
                let model = self
                    .model
                    .clone()
                    .or_else(|| shared.get_feature_model(feature).into())
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
    /// 解析语音润色配置
    pub fn resolve_polishing(&self) -> ResolvedLlmClientConfig {
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
        },
        LlmPreset {
            id: "translation".to_string(),
            name: "中译英".to_string(),
            system_prompt: "你是一个专业的翻译助手。请将用户的中文语音转写内容翻译成地道、流畅的英文。不要输出任何解释性文字，只输出翻译结果。".to_string(),
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
pub const DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT: &str = r#"你是一个文本处理专家。用户选中了一段文本，并给出了处理指令，你需要：
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
    /// 问答模式系统提示词（无选中文本时使用）
    #[serde(default = "default_assistant_qa_prompt")]
    pub qa_system_prompt: String,
    /// 文本处理模式系统提示词（有选中文本时使用）
    #[serde(default = "default_assistant_text_processing_prompt")]
    pub text_processing_system_prompt: String,
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
            qa_system_prompt: default_assistant_qa_prompt(),
            text_processing_system_prompt: default_assistant_text_processing_prompt(),
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
    /// 解析 LLM 配置
    pub fn resolve_llm(&self, shared: &SharedLlmConfig) -> ResolvedLlmClientConfig {
        self.llm.resolve_with_feature(shared, "assistant")
    }

    /// 检查配置是否有效（结合共享配置）
    pub fn is_valid_with_shared(&self, shared: &SharedLlmConfig) -> bool {
        self.llm.is_valid_with_shared(shared)
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

impl AppConfig {
    pub fn new() -> Self {
        Self {
            dashscope_api_key: String::new(),
            siliconflow_api_key: String::new(),
            asr_config: AsrConfig::default(),
            use_realtime_asr: default_use_realtime_asr(),
            enable_llm_post_process: false,
            llm_config: LlmConfig::default(),
            smart_command_config: SmartCommandConfig::default(),
            assistant_config: AssistantConfig::default(),
            learning_config: LearningConfig::default(),
            close_action: None,
            hotkey_config: None,
            dual_hotkey_config: DualHotkeyConfig::default(),
            transcription_mode: TranscriptionMode::default(),
            enable_mute_other_apps: false,
            dictionary: Vec::new(),
            theme: default_theme(),
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = config_dir.join("PushToTalk");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    pub fn load() -> Result<(Self, bool)> {
        let path = Self::config_path()?;
        tracing::info!("尝试从以下路径加载配置: {:?}", path);

        // 跟踪是否发生了迁移（调用者可根据此决定是否保存）
        let mut migrated = false;

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;

            // 使用 serde_json::Value 先解析，以支持结构迁移
            let v: serde_json::Value = serde_json::from_str(&content)?;

            // 尝试直接反序列化为 AppConfig
            let mut config: AppConfig = match serde_json::from_value(v.clone()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("直接解析配置失败，尝试手动迁移: {}", e);
                    let mut cfg = AppConfig::new();

                    // 尝试从原始 JSON 提取未变更的字段
                    if let Some(llm_config) = v.get("llm_config") {
                        if let Ok(llm) = serde_json::from_value(llm_config.clone()) {
                            tracing::info!("成功恢复 llm_config");
                            cfg.llm_config = llm;
                        }
                    }
                    if let Some(assistant_config) = v.get("assistant_config") {
                        if let Ok(assistant) = serde_json::from_value(assistant_config.clone()) {
                            tracing::info!("成功恢复 assistant_config");
                            cfg.assistant_config = assistant;
                        }
                    }
                    if let Some(dictionary) = v.get("dictionary") {
                        if let Ok(dict) = serde_json::from_value(dictionary.clone()) {
                            tracing::info!("成功恢复 dictionary");
                            cfg.dictionary = dict;
                        }
                    }

                    cfg
                }
            };

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
                            // 重要：设置 use_shared=false 以保留原语义
                            // 否则迁移后会优先使用 Provider Registry，导致 endpoint 被忽略
                            config.learning_config.feature_override.use_shared = false;
                            migrated = true;
                        }
                    }
                }
            }

            // 迁移 5: 旧单快捷键 → 新双快捷键 (保持原有逻辑)
            if let Some(old_hotkey) = config.hotkey_config.take() {
                let is_default = config.dual_hotkey_config.dictation.keys
                    == vec![HotkeyKey::ControlLeft, HotkeyKey::MetaLeft]
                    && config.dual_hotkey_config.assistant.keys
                        == vec![HotkeyKey::AltLeft, HotkeyKey::Space];

                if is_default {
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
                        },
                        qa_system_prompt: config.smart_command_config.system_prompt.clone(),
                        text_processing_system_prompt: default_assistant_text_processing_prompt(),
                    };
                    config.smart_command_config.enabled = false;
                }
            }

            // 迁移 7: 旧配置 → Provider Registry (自动迁移)
            if config.llm_config.shared.providers.is_empty() {
                // 检查是否有旧配置需要迁移
                let has_old_shared_config = config.llm_config.shared.endpoint.is_some()
                    && config.llm_config.shared.api_key.is_some()
                    && config.llm_config.shared.default_model.is_some();

                if has_old_shared_config {
                    tracing::info!("检测到旧版共享配置，开始迁移到 Provider Registry");

                    use sha2::{Digest, Sha256};

                    // 计算 Voice Polishing 的 effective 配置
                    let polishing_endpoint = config
                        .llm_config
                        .shared
                        .endpoint
                        .clone()
                        .unwrap_or_default();
                    let polishing_api_key =
                        config.llm_config.shared.api_key.clone().unwrap_or_default();
                    let polishing_model = config
                        .llm_config
                        .shared
                        .polishing_model
                        .clone()
                        .or_else(|| config.llm_config.shared.default_model.clone())
                        .unwrap_or_else(default_llm_model);

                    // 计算 AI Assistant 的 effective 配置
                    let assistant_endpoint = if config.assistant_config.llm.use_shared {
                        config
                            .assistant_config
                            .llm
                            .endpoint
                            .clone()
                            .unwrap_or_else(|| polishing_endpoint.clone())
                    } else {
                        config
                            .assistant_config
                            .llm
                            .endpoint
                            .clone()
                            .unwrap_or_default()
                    };
                    let assistant_api_key = if config.assistant_config.llm.use_shared {
                        config
                            .assistant_config
                            .llm
                            .api_key
                            .clone()
                            .unwrap_or_else(|| polishing_api_key.clone())
                    } else {
                        config
                            .assistant_config
                            .llm
                            .api_key
                            .clone()
                            .unwrap_or_default()
                    };
                    let assistant_model = if config.assistant_config.llm.use_shared {
                        config
                            .assistant_config
                            .llm
                            .model
                            .clone()
                            .or_else(|| config.llm_config.shared.assistant_model.clone())
                            .or_else(|| config.llm_config.shared.default_model.clone())
                            .unwrap_or_else(default_llm_model)
                    } else {
                        config
                            .assistant_config
                            .llm
                            .model
                            .clone()
                            .unwrap_or_default()
                    };

                    // 生成确定性 Provider ID
                    fn generate_provider_id(endpoint: &str, api_key: &str) -> String {
                        let mut hasher = Sha256::new();
                        hasher.update(endpoint.as_bytes());
                        hasher.update(b"|");
                        hasher.update(api_key.as_bytes());
                        let result = hasher.finalize();
                        format!("{:x}", result)[..12].to_string()
                    }

                    let polishing_id =
                        generate_provider_id(&polishing_endpoint, &polishing_api_key);
                    let assistant_id =
                        generate_provider_id(&assistant_endpoint, &assistant_api_key);

                    // 判断是否需要创建 1 个或 2 个 Provider
                    if polishing_id == assistant_id {
                        // 配置相同，创建 1 个 Provider
                        tracing::info!("语音润色和 AI 助手使用相同配置，创建单个 Provider");

                        let provider = LlmProvider {
                            id: polishing_id.clone(),
                            name: "默认提供商 (迁移)".to_string(),
                            endpoint: polishing_endpoint,
                            api_key: polishing_api_key,
                            default_model: config
                                .llm_config
                                .shared
                                .default_model
                                .clone()
                                .unwrap_or_else(default_llm_model),
                        };

                        config.llm_config.shared.providers.push(provider);
                        config.llm_config.shared.default_provider_id = polishing_id.clone();

                        // 设置功能绑定
                        config.llm_config.shared.polishing_provider_id = Some(polishing_id.clone());
                        config.llm_config.shared.assistant_provider_id = Some(polishing_id.clone());

                        // 设置模型覆盖
                        if polishing_model
                            != config
                                .llm_config
                                .shared
                                .default_model
                                .clone()
                                .unwrap_or_else(default_llm_model)
                        {
                            config.llm_config.shared.polishing_model = Some(polishing_model);
                        }
                        if assistant_model
                            != config
                                .llm_config
                                .shared
                                .default_model
                                .clone()
                                .unwrap_or_else(default_llm_model)
                        {
                            config.llm_config.shared.assistant_model = Some(assistant_model);
                        }

                        // 清空 Feature 的独立配置
                        config.llm_config.feature_override.use_shared = true;
                        config.llm_config.feature_override.endpoint = None;
                        config.llm_config.feature_override.api_key = None;
                        config.assistant_config.llm.use_shared = true;
                        config.assistant_config.llm.endpoint = None;
                        config.assistant_config.llm.api_key = None;
                    } else {
                        // 配置不同，创建 2 个 Provider
                        tracing::info!("语音润色和 AI 助手使用不同配置，创建两个 Provider");

                        let polishing_provider = LlmProvider {
                            id: polishing_id.clone(),
                            name: "语音润色提供商 (迁移)".to_string(),
                            endpoint: polishing_endpoint,
                            api_key: polishing_api_key,
                            default_model: polishing_model.clone(),
                        };

                        let assistant_provider = LlmProvider {
                            id: assistant_id.clone(),
                            name: "AI 助手提供商 (迁移)".to_string(),
                            endpoint: assistant_endpoint,
                            api_key: assistant_api_key,
                            default_model: assistant_model.clone(),
                        };

                        config.llm_config.shared.providers.push(polishing_provider);
                        config.llm_config.shared.providers.push(assistant_provider);
                        config.llm_config.shared.default_provider_id = polishing_id.clone();

                        // 设置功能绑定
                        config.llm_config.shared.polishing_provider_id = Some(polishing_id.clone());
                        config.llm_config.shared.assistant_provider_id = Some(assistant_id.clone());

                        // 清空 Feature 的独立配置
                        config.llm_config.feature_override.use_shared = true;
                        config.llm_config.feature_override.endpoint = None;
                        config.llm_config.feature_override.api_key = None;
                        config.assistant_config.llm.use_shared = true;
                        config.assistant_config.llm.endpoint = None;
                        config.assistant_config.llm.api_key = None;
                    }

                    // 标记迁移完成（不在此处保存，由调用者决定）
                    tracing::info!("Provider 迁移完成");
                    migrated = true;
                }
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
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        tracing::info!("保存配置到: {:?}", path);

        // 使用原子写入：先写临时文件，再原子替换
        let temp_path = path.with_extension("json.tmp");
        let backup_path = path.with_extension("json.bak");

        tracing::info!("写入临时文件: {:?}", temp_path);
        std::fs::write(&temp_path, &content).map_err(|e| {
            tracing::error!("写入临时文件失败: {}", e);
            e
        })?;

        // Windows 原子替换策略：
        // 1. 如果目标文件存在，先备份到 .bak
        // 2. 重命名临时文件到目标文件
        // 3. 删除备份文件
        // 这样即使在任何步骤崩溃，都能恢复：
        // - 步骤 1 崩溃：原文件完好
        // - 步骤 2 崩溃：.bak 文件可用于恢复
        // - 步骤 3 崩溃：配置已保存成功，.bak 只是残留
        tracing::info!("执行原子替换");
        if path.exists() {
            // 先备份旧文件
            if backup_path.exists() {
                let _ = std::fs::remove_file(&backup_path);
            }
            std::fs::rename(&path, &backup_path).map_err(|e| {
                tracing::error!("备份旧配置文件失败: {}", e);
                e
            })?;
        }

        // 重命名临时文件到目标文件
        match std::fs::rename(&temp_path, &path) {
            Ok(_) => {
                // 成功后删除备份文件
                let _ = std::fs::remove_file(&backup_path);
                tracing::info!("配置保存成功");
                Ok(())
            }
            Err(e) => {
                tracing::error!("重命名临时文件失败: {}", e);
                // 尝试恢复备份
                if backup_path.exists() {
                    if let Err(restore_err) = std::fs::rename(&backup_path, &path) {
                        tracing::error!("恢复备份失败: {}", restore_err);
                    } else {
                        tracing::info!("已从备份恢复配置");
                    }
                }
                Err(e.into())
            }
        }
    }
}
