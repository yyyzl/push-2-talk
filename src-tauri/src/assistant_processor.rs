// src-tauri/src/assistant_processor.rs
//
// AI 助手处理器
//
// 支持双系统提示词：问答模式和文本处理模式

use anyhow::Result;

use crate::config::{AssistantConfig, SharedLlmConfig};
use crate::openai_client::{ChatOptions, OpenAiClient, OpenAiClientConfig};

/// AI 助手处理器
///
/// 根据是否有上下文（选中文本）使用不同的系统提示词
#[derive(Clone)]
pub struct AssistantProcessor {
    client: OpenAiClient,
    /// 问答模式系统提示词（无选中文本时使用）
    qa_system_prompt: String,
    /// 文本处理模式系统提示词（有选中文本时使用）
    text_processing_system_prompt: String,
}

impl AssistantProcessor {
    /// 创建新的 AI 助手处理器实例
    pub fn new(config: AssistantConfig, shared: &SharedLlmConfig) -> Self {
        let resolved = config.resolve_llm(shared);
        let client_config = OpenAiClientConfig::new(
            &resolved.endpoint,
            &resolved.api_key,
            &resolved.model,
        );
        let client = OpenAiClient::new(client_config);

        Self {
            client,
            qa_system_prompt: config.qa_system_prompt,
            text_processing_system_prompt: config.text_processing_system_prompt,
        }
    }

    /// 处理用户指令（无上下文 - 问答模式）
    ///
    /// # Arguments
    /// * `user_input` - 用户的语音转写文本（问题/指令）
    ///
    /// # Returns
    /// * LLM 的回答
    pub async fn process(&self, user_input: &str) -> Result<String> {
        if user_input.trim().is_empty() {
            return Ok(String::new());
        }

        tracing::info!("AssistantProcessor: 问答模式处理指令: {}", user_input);

        self.client
            .chat_simple(
                &self.qa_system_prompt,
                user_input,
                ChatOptions::for_smart_command(),
            )
            .await
    }

    /// 带上下文的指令处理（文本处理模式）
    ///
    /// # Arguments
    /// * `user_instruction` - 用户的语音指令
    /// * `selected_text` - 选中的文本
    ///
    /// # Returns
    /// * LLM 处理后的结果
    pub async fn process_with_context(
        &self,
        user_instruction: &str,
        selected_text: &str,
    ) -> Result<String> {
        if user_instruction.trim().is_empty() {
            return Ok(String::new());
        }

        tracing::info!(
            "AssistantProcessor: 文本处理模式 (指令: {}, 上下文长度: {} 字符)",
            user_instruction,
            selected_text.len()
        );

        // 构建包含上下文的用户消息
        let user_message = format!(
            "【选中的文本】\n{}\n\n【用户指令】\n{}",
            selected_text, user_instruction
        );

        self.client
            .chat_simple(
                &self.text_processing_system_prompt,
                &user_message,
                ChatOptions::for_smart_command(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DEFAULT_ASSISTANT_QA_PROMPT, DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT, LlmFeatureConfig, SharedLlmConfig};

    fn create_test_config() -> AssistantConfig {
        AssistantConfig {
            enabled: true,
            llm: LlmFeatureConfig {
                use_shared: false,
                endpoint: Some("https://api.example.com/v1/chat/completions".to_string()),
                model: Some("test-model".to_string()),
                api_key: Some("test-key".to_string()),
            },
            qa_system_prompt: DEFAULT_ASSISTANT_QA_PROMPT.to_string(),
            text_processing_system_prompt: DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT.to_string(),
        }
    }

    #[test]
    fn test_processor_creation() {
        let config = create_test_config();
        let shared = SharedLlmConfig::default();
        let processor = AssistantProcessor::new(config, &shared);
        assert!(!processor.qa_system_prompt.is_empty());
        assert!(!processor.text_processing_system_prompt.is_empty());
    }
}
