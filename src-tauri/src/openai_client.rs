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
    /// 是否优先追求低延迟（尝试关闭/最小化模型思考）
    pub disable_thinking_for_speed: bool,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: 0.3,
            disable_thinking_for_speed: false,
        }
    }
}

impl ChatOptions {
    /// 用于文本润色的参数（低温度，高确定性）
    pub fn for_polishing() -> Self {
        Self {
            max_tokens: 2048, // 使用与 Smart Command 相同的值，避免 API 兼容性问题
            temperature: 0.7,
            disable_thinking_for_speed: true,
        }
    }

    /// 用于智能指令的参数（稍高温度，更灵活）
    pub fn for_smart_command() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.5,
            disable_thinking_for_speed: false,
        }
    }
}

fn normalize_model_for_endpoint(endpoint: &str, model: &str) -> String {
    let model_trimmed = model.trim();
    if model_trimmed.is_empty() {
        return String::new();
    }

    let endpoint_lc = endpoint.to_ascii_lowercase();
    let model_lc = model_trimmed.to_ascii_lowercase();

    // DeepSeek V3.2 官方推荐通过 deepseek-chat 访问。
    if endpoint_lc.contains("api.deepseek.com")
        && matches!(
            model_lc.as_str(),
            "deepseekv3.2" | "deepseek-v3.2" | "deepseek_v3.2" | "deepseek-v3-2" | "deepseek_v3_2"
        )
    {
        return "deepseek-chat".to_string();
    }

    model_trimmed.to_string()
}

fn build_request_body(
    endpoint: &str,
    model: &str,
    messages_json: &[Value],
    options: &ChatOptions,
) -> Value {
    let resolved_model = normalize_model_for_endpoint(endpoint, model);
    let endpoint_lc = endpoint.to_ascii_lowercase();
    let resolved_model_lc = resolved_model.to_ascii_lowercase();

    let mut request_body = serde_json::json!({
        "model": resolved_model,
        "messages": messages_json,
        "max_tokens": options.max_tokens,
        "temperature": options.temperature
    });

    if options.disable_thinking_for_speed {
        // 豆包 Seed 1.8：显式关闭 thinking。
        if resolved_model_lc.starts_with("doubao-seed")
        {
            request_body["thinking"] = serde_json::json!({ "type": "disabled" });
        }

        // DeepSeek V3.2（deepseek-chat）：显式关闭 thinking，确保低延迟。
        if resolved_model_lc.starts_with("deepseek") {
            request_body["thinking"] = serde_json::json!({ "type": "disabled" });
        }

        // Gemini 3 Flash（OpenAI 兼容）：最小化推理强度。似乎并没有用
        if resolved_model_lc.starts_with("gemini-3-flash")
        {
            request_body["reasoning_effort"] = serde_json::json!("minimal");
            request_body["thinking"] = serde_json::json!({ "type": "disabled" });
            request_body["thinking"] = serde_json::json!({ "thinking_type": "disabled" });
        }
    }

    request_body
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
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
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

        let request_body = build_request_body(
            &self.config.endpoint,
            &self.config.model,
            &messages_json,
            &options,
        );
        let request_model = request_body["model"]
            .as_str()
            .unwrap_or(self.config.model.as_str());

        // 打印完整请求信息用于调试
        tracing::info!(
            "[DEBUG] OpenAI 请求: endpoint={}, model={} -> {}, api_key_len={}, max_tokens={}, temperature={}, low_latency={}",
            self.config.endpoint,
            self.config.model,
            request_model,
            self.config.api_key.len(),
            options.max_tokens,
            options.temperature,
            options.disable_thinking_for_speed
        );
        tracing::info!(
            "[DEBUG] 请求体: {}",
            serde_json::to_string_pretty(&request_body).unwrap_or_default()
        );

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
        let messages = vec![Message::system(system_prompt), Message::user(user_message)];
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
        assert!(!default.disable_thinking_for_speed);

        let polishing = ChatOptions::for_polishing();
        assert_eq!(polishing.max_tokens, 2048);
        assert_eq!(polishing.temperature, 0.7);
        assert!(polishing.disable_thinking_for_speed);

        let smart = ChatOptions::for_smart_command();
        assert_eq!(smart.max_tokens, 2048);
        assert_eq!(smart.temperature, 0.5);
        assert!(!smart.disable_thinking_for_speed);
    }

    #[test]
    fn test_config_creation() {
        let config = OpenAiClientConfig::new(
            "https://api.example.com/v1/chat/completions",
            "sk-xxx",
            "gpt-4",
        );
        assert_eq!(
            config.endpoint,
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(config.api_key, "sk-xxx");
        assert_eq!(config.model, "gpt-4");
    }

    #[test]
    fn test_normalize_deepseek_v32_alias() {
        let normalized = normalize_model_for_endpoint(
            "https://api.deepseek.com/chat/completions",
            "deepseekv3.2",
        );
        assert_eq!(normalized, "deepseek-chat");
    }

    #[test]
    fn test_polishing_adaptation_for_doubao_seed_18() {
        let messages = vec![serde_json::json!({"role": "user", "content": "ping"})];
        let options = ChatOptions::for_polishing();
        let body = build_request_body(
            "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
            "doubao-seed-1-8-251228",
            &messages,
            &options,
        );

        assert_eq!(
            body["thinking"],
            serde_json::json!({
                "type": "disabled"
            })
        );
    }

    #[test]
    fn test_polishing_adaptation_for_gemini3_flash() {
        let messages = vec![serde_json::json!({"role": "user", "content": "ping"})];
        let options = ChatOptions::for_polishing();
        let body = build_request_body(
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            "gemini-3-flash-preview",
            &messages,
            &options,
        );

        assert_eq!(body["reasoning_effort"], serde_json::json!("minimal"));
    }

    #[test]
    fn test_polishing_adaptation_for_deepseek_chat() {
        let messages = vec![serde_json::json!({"role": "user", "content": "ping"})];
        let options = ChatOptions::for_polishing();
        let body = build_request_body(
            "https://api.deepseek.com/chat/completions",
            "deepseek-chat",
            &messages,
            &options,
        );

        assert_eq!(
            body["thinking"],
            serde_json::json!({
                "type": "disabled"
            })
        );
    }
}
