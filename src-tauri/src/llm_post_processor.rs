// src-tauri/src/llm_post_processor.rs
//
// LLM 文本润色处理模块
//
// 基于通用 OpenAI 客户端，提供文本润色功能
// 支持多预设管理，用户可自定义润色风格

use anyhow::Result;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use crate::config::LlmConfig;
use crate::dictionary_utils::entries_to_words;
use crate::openai_client::{ChatOptions, OpenAiClient, OpenAiClientConfig};

/// LLM 文本润色处理器
///
/// 使用通用 OpenAI 客户端，专注于文本润色功能
#[derive(Clone)]
pub struct LlmPostProcessor {
    client: OpenAiClient,
    config: LlmConfig,
    /// 配置哈希（用于检测配置是否变化，避免不必要的重建）
    config_hash: u64,
}

impl LlmPostProcessor {
    const MAX_DICTIONARY_ENTRIES: usize = 200;
    const MAX_DICTIONARY_CHARS: usize = 4000;
    /// 词库增强追加指令（当语句润色和词库增强同时开启时追加到用户预设后）
    const DICTIONARY_ENHANCEMENT_SUFFIX: &'static str = "

【词库增强规则】
请参考 <dictionary> 标签中的词汇进行音似纠错：
- 优先判断原文词语与词库词汇在发音上是否相同或极度相似
- 仅当发音匹配且替换后语义更合理时才执行修改
- 不确定时保留原文";

    const DICTIONARY_ONLY_SYSTEM_PROMPT: &'static str = "
    <role>
你是一位精通中英双语的 ASR（语音转文字）校对专家。你具备极强的语音感知能力，擅长区分“发音错误”与“语义表达差异”。
</role>

<task_logic>
你的任务是根据语境修复源文本。请遵循以下判断逻辑：
1. 语音匹配判定：优先判断原文词语与候选词（词库提供或语境推测）在发音上是否【相同】或【极度相似】。
2. 语境适配判定：仅当替换后的词语能显著提升整句逻辑的合理性时，才执行修改。
3. 保守执行策略：若原文逻辑通顺，或不确定是否为语音误识，请始终保留原文。
</task_logic>

<rules>
- 优先参考 <dictionary> 标签中的词汇。
- 允许自主纠正：若未命中词库但发音高度相似且符合语境，应予以纠正（如：专业术语、地名）。
- 保持原样原则：如果两个词意思相近但发音差异大（如：赞赏 vs 点赞），请务必保留原文。
- 格式规范：将数字、百分比、日期转换为阿拉伯数字格式（如：2024年5月3日，30%）。
- 最终输出：仅展示修正后的纯文本，不包含任何解释。
</rules>

<few_shot_examples>
    <example>
        <input>增加一些 feel shoot 用力</input>
        <output>增加一些 feel shoot 用力</output>
        <reason>“feel shoot”与“claude code”发音差异过大，不符合音似判定。</reason>
    </example>
    <example>
        <input>感谢你的赞赏</input>
        <output>感谢你的赞赏</output>
        <reason>“赞赏”与“点赞”意思接近但读音不同，应尊重原表达。</reason>
    </example>
    <example>
        <input>我认为 Gemini 三 Flash 是目前最平衡的模型</input>
        <output>我认为 Gemini-3-Flash 是目前最平衡的模型</output>
        <reason>“三”与“3”同音，命中专业词库，应修正。</reason>
    </example>
    <example>
        <input>我又回了，VS Code</input>
        <output>我用回了VS Code</output>
        <reason>“又”与“用”发音接近且“我用回了”更符合逻辑语境。</reason>
    </example>
</few_shot_examples>";

    /// 创建新的处理器实例
    pub fn new(config: LlmConfig) -> Self {
        let resolved = config.resolve_polishing();
        let client_config =
            OpenAiClientConfig::new(&resolved.endpoint, &resolved.api_key, &resolved.model);
        let client = OpenAiClient::new(client_config);
        let config_hash = Self::compute_config_hash(&config);

        Self {
            client,
            config,
            config_hash,
        }
    }

    /// 计算配置哈希（用于检测配置是否变化）
    fn compute_config_hash(config: &LlmConfig) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // 哈希关键字段：endpoint、api_key、model、active_preset_id、当前 preset 的 system_prompt
        let resolved = config.resolve_polishing();
        resolved.endpoint.hash(&mut hasher);
        resolved.api_key.hash(&mut hasher);
        resolved.model.hash(&mut hasher);
        config.active_preset_id.hash(&mut hasher);
        // 哈希当前激活的 preset 的 system_prompt
        if let Some(preset) = config
            .presets
            .iter()
            .find(|p| p.id == config.active_preset_id)
        {
            preset.system_prompt.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 检查新配置是否与当前配置不同（需要重建处理器）
    pub fn config_changed(&self, new_config: &LlmConfig) -> bool {
        let new_hash = Self::compute_config_hash(new_config);
        self.config_hash != new_hash
    }

    /// 获取当前激活的润色 Prompt
    fn get_active_system_prompt(&self) -> String {
        self.config
            .presets
            .iter()
            .find(|p| p.id == self.config.active_preset_id)
            .map(|p| p.system_prompt.clone())
            .unwrap_or_else(|| "You are a helpful assistant.".to_string())
    }

    fn build_user_message(
        raw_text: &str,
        dictionary: &[String],
        enable_dictionary_enhancement: bool,
    ) -> String {
        let mut message = "".to_string();

        // 参考词库
        message.push_str("<dictionary>\n");

        if enable_dictionary_enhancement {
            // 提纯词库（去除 |auto 后缀）
            let purified_words = entries_to_words(dictionary);

            let mut words: Vec<&str> = purified_words
                .iter()
                .map(|w| w.trim())
                .filter(|w| !w.is_empty())
                .collect();

            if !words.is_empty() {
                // 去重（保序）
                let mut seen = HashSet::new();
                words.retain(|w| seen.insert(*w));

                let mut used = 0usize;
                let mut used_chars = 0usize;
                let total = words.len();
                let mut word_list: Vec<&str> = Vec::new();

                for word in &words {
                    if used >= Self::MAX_DICTIONARY_ENTRIES {
                        break;
                    }
                    let next_len = word.chars().count() + 2; // + ", "
                    if used_chars + next_len > Self::MAX_DICTIONARY_CHARS {
                        break;
                    }
                    word_list.push(word);
                    used += 1;
                    used_chars += next_len;
                }

                message.push_str(&word_list.join(", "));

                if used < total {
                    message.push_str(&format!("\n...(词库过长，已截断；原始共 {} 条)", total));
                }
            }
        }

        message.push_str("\n</dictionary>\n\n");

        // 待处理文本
        message.push_str("\n<source_text>\n");
        message.push_str(raw_text);
        message.push_str("\n</source_text>\n\n请处理上述 <source_text>，直接输出最终结果。\n");

        message
    }

    /// 文本润色
    ///
    /// 使用当前激活的预设对 ASR 转写文本进行润色
    ///
    /// # Arguments
    /// * `raw_text` - ASR 转写的原始文本
    ///
    /// # Returns
    /// * 润色后的文本
    pub async fn polish_transcript(
        &self,
        raw_text: &str,
        dictionary: &[String],
        enable_post_process: bool,
        enable_dictionary_enhancement: bool,
    ) -> Result<String> {
        if raw_text.trim().is_empty() {
            return Ok(String::new());
        }

        let system_prompt = if enable_post_process {
            let base_prompt = self.get_active_system_prompt();
            if enable_dictionary_enhancement {
                // 两者都开：追加词库增强指令到用户预设后
                tracing::info!(
                    "LLM 后处理使用预设 ID: {} + 词库增强",
                    self.config.active_preset_id
                );
                format!("{}{}", base_prompt, Self::DICTIONARY_ENHANCEMENT_SUFFIX)
            } else {
                tracing::info!("LLM 后处理使用预设 ID: {}", self.config.active_preset_id);
                base_prompt
            }
        } else {
            tracing::info!("LLM 后处理: 仅词库增强（未启用语句润色）");
            Self::DICTIONARY_ONLY_SYSTEM_PROMPT.to_string()
        };

        let user_message =
            Self::build_user_message(raw_text, dictionary, enable_dictionary_enhancement);

        self.client
            .chat_simple(&system_prompt, &user_message, ChatOptions::for_polishing())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmFeatureConfig, LlmPreset, SharedLlmConfig};

    fn create_test_config() -> LlmConfig {
        LlmConfig {
            shared: SharedLlmConfig {
                providers: Vec::new(),
                default_provider_id: String::new(),
                polishing_provider_id: None,
                assistant_provider_id: None,
                learning_provider_id: None,
                endpoint: Some("https://api.example.com/v1/chat/completions".to_string()),
                api_key: Some("test-key".to_string()),
                default_model: Some("test-model".to_string()),
                polishing_model: None,
                assistant_model: None,
                learning_model: None,
            },
            feature_override: LlmFeatureConfig::default(),
            presets: vec![LlmPreset {
                id: "test".to_string(),
                name: "Test Preset".to_string(),
                system_prompt: "You are a test assistant.".to_string(),
            }],
            active_preset_id: "test".to_string(),
        }
    }

    #[test]
    fn test_get_active_system_prompt() {
        let config = create_test_config();
        let processor = LlmPostProcessor::new(config);
        let prompt = processor.get_active_system_prompt();
        assert_eq!(prompt, "You are a test assistant.");
    }

    #[test]
    fn test_get_active_system_prompt_fallback() {
        let mut config = create_test_config();
        config.active_preset_id = "non-existent".to_string();
        let processor = LlmPostProcessor::new(config);
        let prompt = processor.get_active_system_prompt();
        assert_eq!(prompt, "You are a helpful assistant.");
    }

    #[test]
    fn test_build_user_message_without_dictionary() {
        let msg = LlmPostProcessor::build_user_message("hello", &[], true);
        assert!(msg.contains("<source_text>"));
        assert!(!msg.contains("<user_dictionary>"));
    }

    #[test]
    fn test_build_user_message_with_dictionary_enabled() {
        let dict = vec![
            "张三".to_string(),
            "  北京  ".to_string(),
            "张三".to_string(),
        ];
        let msg = LlmPostProcessor::build_user_message("你好", &dict, true);
        assert!(msg.contains("<dictionary>"));
        assert!(msg.contains("张三"));
        assert!(msg.contains("北京"));
        assert!(msg.contains("<source_text>"));
    }

    #[test]
    fn test_build_user_message_with_dictionary_disabled() {
        let dict = vec!["张三".to_string()];
        let msg = LlmPostProcessor::build_user_message("你好", &dict, false);
        assert!(!msg.contains("<user_dictionary>"));
    }
}
