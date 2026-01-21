// src-tauri/src/config.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashSet;
use anyhow::Result;

use crate::learning::store::{DictionaryEntry, DictionarySource};

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
    MetaLeft,   // Win/Cmd 左
    MetaRight,  // Win/Cmd 右

    // 功能键
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // 常用键
    Space,
    Tab,
    CapsLock,
    Escape,

    // 字母键
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ,
    KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT,
    KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,

    // 数字键
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,

    // 方向键
    Up, Down, Left, Right,

    // 编辑键
    Return, Backspace, Delete, Insert, Home, End, PageUp, PageDown,
}

impl HotkeyKey {
    /// 判断是否为修饰键
    pub fn is_modifier(&self) -> bool {
        matches!(self,
            HotkeyKey::ControlLeft | HotkeyKey::ControlRight |
            HotkeyKey::ShiftLeft | HotkeyKey::ShiftRight |
            HotkeyKey::AltLeft | HotkeyKey::AltRight |
            HotkeyKey::MetaLeft | HotkeyKey::MetaRight
        )
    }

    /// 判断是否为功能键
    pub fn is_function_key(&self) -> bool {
        matches!(self,
            HotkeyKey::F1 | HotkeyKey::F2 | HotkeyKey::F3 | HotkeyKey::F4 |
            HotkeyKey::F5 | HotkeyKey::F6 | HotkeyKey::F7 | HotkeyKey::F8 |
            HotkeyKey::F9 | HotkeyKey::F10 | HotkeyKey::F11 | HotkeyKey::F12
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
            HotkeyKey::F1 => "F1", HotkeyKey::F2 => "F2", HotkeyKey::F3 => "F3",
            HotkeyKey::F4 => "F4", HotkeyKey::F5 => "F5", HotkeyKey::F6 => "F6",
            HotkeyKey::F7 => "F7", HotkeyKey::F8 => "F8", HotkeyKey::F9 => "F9",
            HotkeyKey::F10 => "F10", HotkeyKey::F11 => "F11", HotkeyKey::F12 => "F12",
            HotkeyKey::KeyA => "A", HotkeyKey::KeyB => "B", HotkeyKey::KeyC => "C",
            HotkeyKey::KeyD => "D", HotkeyKey::KeyE => "E", HotkeyKey::KeyF => "F",
            HotkeyKey::KeyG => "G", HotkeyKey::KeyH => "H", HotkeyKey::KeyI => "I",
            HotkeyKey::KeyJ => "J", HotkeyKey::KeyK => "K", HotkeyKey::KeyL => "L",
            HotkeyKey::KeyM => "M", HotkeyKey::KeyN => "N", HotkeyKey::KeyO => "O",
            HotkeyKey::KeyP => "P", HotkeyKey::KeyQ => "Q", HotkeyKey::KeyR => "R",
            HotkeyKey::KeyS => "S", HotkeyKey::KeyT => "T", HotkeyKey::KeyU => "U",
            HotkeyKey::KeyV => "V", HotkeyKey::KeyW => "W", HotkeyKey::KeyX => "X",
            HotkeyKey::KeyY => "Y", HotkeyKey::KeyZ => "Z",
            HotkeyKey::Num0 => "0", HotkeyKey::Num1 => "1", HotkeyKey::Num2 => "2",
            HotkeyKey::Num3 => "3", HotkeyKey::Num4 => "4", HotkeyKey::Num5 => "5",
            HotkeyKey::Num6 => "6", HotkeyKey::Num7 => "7", HotkeyKey::Num8 => "8",
            HotkeyKey::Num9 => "9",
            HotkeyKey::Up => "↑", HotkeyKey::Down => "↓",
            HotkeyKey::Left => "←", HotkeyKey::Right => "→",
            HotkeyKey::Return => "Enter", HotkeyKey::Backspace => "Backspace",
            HotkeyKey::Delete => "Delete", HotkeyKey::Insert => "Insert",
            HotkeyKey::Home => "Home", HotkeyKey::End => "End",
            HotkeyKey::PageUp => "PageUp", HotkeyKey::PageDown => "PageDown",
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
            release_mode_keys: None,  // 默认无松手模式快捷键
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
        release_mode_keys: Some(vec![HotkeyKey::F2]),  // 默认 F2 为松手模式快捷键
    }
}

fn default_assistant_hotkey() -> HotkeyConfig {
    HotkeyConfig {
        keys: vec![HotkeyKey::AltLeft, HotkeyKey::Space],
        mode: HotkeyMode::Press,
        enable_release_lock: false,
        release_mode_keys: None,  // AI助手模式不支持松手模式
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
        self.dictation.validate()
            .map_err(|e| anyhow::anyhow!("听写模式快捷键配置无效: {}", e))?;
        self.assistant.validate()
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
        self.keys.iter()
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
    /// 个人词典（热词列表）- 支持来源标记和词频统计
    #[serde(default, deserialize_with = "deserialize_dictionary")]
    pub dictionary: Vec<DictionaryEntry>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_endpoint: Option<String>,
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
        }
    }
}

/// 词典反序列化函数（支持 Vec<String> 和 Vec<DictionaryEntry> 两种格式）
fn deserialize_dictionary<'de, D>(deserializer: D) -> std::result::Result<Vec<DictionaryEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;

    match value {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(Vec::new());
            }

            // 检查第一个元素的类型
            if arr[0].is_string() {
                // 旧格式：Vec<String> -> 迁移为 Vec<DictionaryEntry>
                let words: Vec<String> = arr
                    .into_iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|w| !w.trim().is_empty())
                    .collect();

                Ok(words
                    .into_iter()
                    .map(|word| DictionaryEntry::new(word, DictionarySource::Manual))
                    .collect())
            } else {
                // 新格式：Vec<DictionaryEntry>
                serde_json::from_value(serde_json::Value::Array(arr))
                    .map_err(|e| D::Error::custom(format!("解析词典失败: {}", e)))
            }
        }
        _ => Ok(Vec::new()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPreset {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    
    // 新增：预设列表和当前选中的预设ID
    #[serde(default = "default_presets")]
    pub presets: Vec<LlmPreset>,
    #[serde(default = "default_active_preset_id")]
    pub active_preset_id: String,
}

fn default_llm_endpoint() -> String {
    "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
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
    /// API 端点
    #[serde(default = "default_assistant_endpoint")]
    pub endpoint: String,
    /// 模型名称
    #[serde(default = "default_assistant_model")]
    pub model: String,
    /// API Key
    #[serde(default)]
    pub api_key: String,
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

fn default_assistant_endpoint() -> String {
    "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
}

fn default_assistant_model() -> String {
    "glm-4-flash-250414".to_string()
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
            endpoint: default_assistant_endpoint(),
            model: default_assistant_model(),
            api_key: String::new(),
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
    /// 检查配置是否有效（API Key 已填写）
    pub fn is_valid(&self) -> bool {
        !self.api_key.is_empty() && !self.endpoint.is_empty() && !self.model.is_empty()
    }
}

// 为了兼容旧版本配置，如果反序列化时 presets 为空，手动填充默认值
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: default_llm_endpoint(),
            model: default_llm_model(),
            api_key: String::new(),
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
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = config_dir.join("PushToTalk");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        tracing::info!("尝试从以下路径加载配置: {:?}", path);
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
            if config.asr_config.credentials.qwen_api_key.is_empty() && !config.dashscope_api_key.is_empty() {
                tracing::info!("从根配置迁移 Qwen API Key");
                config.asr_config.credentials.qwen_api_key = config.dashscope_api_key.clone();
            }
            if config.asr_config.credentials.sensevoice_api_key.is_empty() && !config.siliconflow_api_key.is_empty() {
                tracing::info!("从根配置迁移 SiliconFlow API Key");
                config.asr_config.credentials.sensevoice_api_key = config.siliconflow_api_key.clone();
            }

            // 迁移 3: 旧单快捷键 → 新双快捷键 (保持原有逻辑)
            if let Some(old_hotkey) = config.hotkey_config.take() {
                let is_default = config.dual_hotkey_config.dictation.keys == vec![HotkeyKey::ControlLeft, HotkeyKey::MetaLeft]
                    && config.dual_hotkey_config.assistant.keys == vec![HotkeyKey::AltLeft, HotkeyKey::Space];

                if is_default {
                    tracing::info!("迁移旧快捷键配置 {} 到听写模式", old_hotkey.format_display());
                    config.dual_hotkey_config.dictation = old_hotkey;
                }
            }

            // 迁移 4: SmartCommandConfig → AssistantConfig (保持原有逻辑)
            if config.smart_command_config.enabled && config.smart_command_config.is_valid() {
                if !config.assistant_config.is_valid() {
                    tracing::info!("迁移 Smart Command 配置到 AI 助手配置");
                    config.assistant_config = AssistantConfig {
                        enabled: config.smart_command_config.enabled,
                        endpoint: config.smart_command_config.endpoint.clone(),
                        model: config.smart_command_config.model.clone(),
                        api_key: config.smart_command_config.api_key.clone(),
                        qa_system_prompt: config.smart_command_config.system_prompt.clone(),
                        text_processing_system_prompt: default_assistant_text_processing_prompt(),
                    };
                    config.smart_command_config.enabled = false;
                }
            }

            if config.llm_config.presets.is_empty() {
                 tracing::info!("检测到预设列表为空，用户可能删除了所有预设");
            }

            tracing::info!("配置加载成功");
            Ok(config)
        } else {
            tracing::warn!("配置文件不存在，创建并返回默认配置");
            Ok(Self::new())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        tracing::info!("保存配置到: {:?}", path);

        // 使用原子写入：先写临时文件，再重命名（避免文件锁定和损坏）
        let temp_path = path.with_extension("json.tmp");

        tracing::info!("写入临时文件: {:?}", temp_path);
        std::fs::write(&temp_path, &content).map_err(|e| {
            tracing::error!("写入临时文件失败: {}", e);
            e
        })?;

        tracing::info!("重命名临时文件到目标文件");
        // Windows 上如果目标文件存在，先删除
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                tracing::error!("删除旧配置文件失败: {}", e);
                e
            })?;
        }

        std::fs::rename(&temp_path, &path).map_err(|e| {
            tracing::error!("重命名临时文件失败: {}", e);
            e
        })?;

        tracing::info!("配置保存成功");
        Ok(())
    }
}
