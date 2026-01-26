// src-tauri/src/openai_client.rs
//
// 通用 OpenAI 兼容 API 客户端
//
// 提供统一的 LLM 调用接口，支持所有 OpenAI 兼容的 API 服务
// （如 OpenAI、智谱 GLM、DeepSeek、通义千问等）

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

// ============================================================================
// 消息类型定义
// ============================================================================

/// LLM 消息角色
#[derive(Debug, Clone)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// LLM 消息
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    #[allow(dead_code)]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

// ============================================================================
// 聊天选项
// ============================================================================

/// 聊天请求参数
#[derive(Debug, Clone)]
pub struct ChatOptions {
    /// 最大生成 token 数
    pub max_tokens: u32,
    /// 温度参数（0.0-1.0，越低越确定）
    /// 使用 f64 避免浮点精度问题（f32 的 0.3 会变成 0.30000001192092896）
    pub temperature: f64,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: 0.3,
        }
    }
}

impl ChatOptions {
    /// 用于文本润色的参数（低温度，高确定性）
    pub fn for_polishing() -> Self {
        Self {
            max_tokens: 2048,  // 使用与 Smart Command 相同的值，避免 API 兼容性问题
            temperature: 0.3,
        }
    }

    /// 用于智能指令的参数（稍高温度，更灵活）
    pub fn for_smart_command() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.5,
        }
    }
}

// ============================================================================
// 客户端配置
// ============================================================================

/// OpenAI 兼容 API 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiClientConfig {
    /// API 端点 (如 https://api.openai.com/v1/chat/completions)
    pub endpoint: String,
    /// API Key
    pub api_key: String,
    /// 模型名称 (如 gpt-4, glm-4-flash)
    pub model: String,
}

impl OpenAiClientConfig {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

// ============================================================================
// OpenAI 客户端
// ============================================================================

/// 通用 OpenAI 兼容 API 客户端
///
/// 支持所有 OpenAI 兼容的 API 服务，提供统一的聊天接口
#[derive(Clone)]
pub struct OpenAiClient {
    config: OpenAiClientConfig,
    client: Client,
}

impl OpenAiClient {
    /// 创建新的客户端实例
    pub fn new(config: OpenAiClientConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .no_proxy()
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { config, client }
    }

    /// 通用聊天方法
    ///
    /// 支持自定义 system prompt 和用户消息
    ///
    /// # Arguments
    /// * `messages` - 消息列表（通常是 system + user）
    /// * `options` - 聊天参数
    ///
    /// # Example
    /// ```ignore
    /// let messages = vec![
    ///     Message::system("你是一个有帮助的助手"),
    ///     Message::user("你好"),
    /// ];
    /// let response = client.chat(&messages, ChatOptions::default()).await?;
    /// ```
    pub async fn chat(&self, messages: &[Message], options: ChatOptions) -> Result<String> {
        if messages.is_empty() {
            return Ok(String::new());
        }

        // 构建 OpenAI 兼容格式的消息
        let messages_json: Vec<Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.as_str(),
                    "content": m.content
                })
            })
            .collect();

        let request_body = serde_json::json!({
            "model": self.config.model,
            "messages": messages_json,
            "max_tokens": options.max_tokens,
            "temperature": options.temperature
        });

        // 打印完整请求信息用于调试
        tracing::info!(
            "[DEBUG] OpenAI 请求: endpoint={}, model={}, api_key_len={}, max_tokens={}, temperature={}",
            self.config.endpoint,
            self.config.model,
            self.config.api_key.len(),
            options.max_tokens,
            options.temperature
        );
        tracing::info!("[DEBUG] 请求体: {}", serde_json::to_string_pretty(&request_body).unwrap_or_default());

        let response = self
            .client
            .post(&self.config.endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API 请求失败 ({}): {}", status, text);
        }

        let payload: Value = response.json().await?;

        // 解析 OpenAI 格式的响应
        let content = payload["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|choice| choice["message"]["content"].as_str())
            .ok_or_else(|| anyhow::anyhow!("OpenAI API 返回格式不可解析: {:?}", payload))?;

        Ok(content.trim().to_string())
    }

    /// 简化的单轮对话方法
    ///
    /// 适用于简单的问答场景
    pub async fn chat_simple(
        &self,
        system_prompt: &str,
        user_message: &str,
        options: ChatOptions,
    ) -> Result<String> {
        let messages = vec![
            Message::system(system_prompt),
            Message::user(user_message),
        ];
        self.chat(&messages, options).await
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let sys = Message::system("test system");
        assert!(matches!(sys.role, Role::System));
        assert_eq!(sys.content, "test system");

        let user = Message::user("test user");
        assert!(matches!(user.role, Role::User));
        assert_eq!(user.content, "test user");
    }

    #[test]
    fn test_chat_options() {
        let default = ChatOptions::default();
        assert_eq!(default.max_tokens, 1024);
        assert_eq!(default.temperature, 0.3);

        let polishing = ChatOptions::for_polishing();
        assert_eq!(polishing.max_tokens, 2048);
        assert_eq!(polishing.temperature, 0.3);

        let smart = ChatOptions::for_smart_command();
        assert_eq!(smart.max_tokens, 2048);
        assert_eq!(smart.temperature, 0.5);
    }

    #[test]
    fn test_config_creation() {
        let config = OpenAiClientConfig::new(
            "https://api.example.com/v1/chat/completions",
            "sk-xxx",
            "gpt-4",
        );
        assert_eq!(config.endpoint, "https://api.example.com/v1/chat/completions");
        assert_eq!(config.api_key, "sk-xxx");
        assert_eq!(config.model, "gpt-4");
    }
}
