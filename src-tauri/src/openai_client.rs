// src-tauri/src/openai_client.rs
//
// 通用 OpenAI 兼容 API 客户端
//
// 提供统一的 LLM 调用接口，支持所有 OpenAI 兼容的 API 服务
// （如 OpenAI、智谱 GLM、DeepSeek、通义千问等）

use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::config::LlmReasoningConfig;
use crate::llm_reasoning::merge_request_options;

// ============================================================================
// 消息类型定义
// ============================================================================

/// LLM 消息角色
#[derive(Debug, Clone)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// LLM 消息
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        }
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Some(tool_calls),
        }
    }

    pub fn tool(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
            tool_calls: None,
        }
    }

    fn to_openai_json(&self) -> Value {
        let mut value = serde_json::json!({
            "role": self.role.as_str(),
            "content": self.content
        });

        if let Some(tool_call_id) = &self.tool_call_id {
            value["tool_call_id"] = Value::String(tool_call_id.clone());
        }
        if let Some(name) = &self.name {
            value["name"] = Value::String(name.clone());
        }
        if let Some(tool_calls) = &self.tool_calls {
            value["tool_calls"] = serde_json::to_value(tool_calls).unwrap_or(Value::Null);
        }

        value
    }
}

// ============================================================================
// Tool calling + streaming 类型
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub call_type: Option<String>,
    pub function_name: Option<String>,
    pub function_arguments_delta: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamChunk {
    pub delta_content: Option<String>,
    pub delta_tool_calls: Vec<StreamToolCallDelta>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatStreamResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
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
    pub reasoning: Option<LlmReasoningConfig>,
    pub custom_body: Option<Value>,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: 0.3,
            reasoning: None,
            custom_body: None,
        }
    }
}

impl ChatOptions {
    /// 用于文本润色的参数（低温度，高确定性）
    pub fn for_polishing() -> Self {
        Self {
            max_tokens: 2048, // 使用与 Smart Command 相同的值，避免 API 兼容性问题
            temperature: 0.7,
            reasoning: None,
            custom_body: None,
        }
    }

    /// 用于智能指令的参数（稍高温度，更灵活）
    pub fn for_smart_command() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.5,
            reasoning: None,
            custom_body: None,
        }
    }

    /// 用于 TNL 候选仲裁的参数（短 JSON、低温度）
    pub fn for_candidate_arbitration() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.1,
            reasoning: None,
            custom_body: None,
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
    /// 请求超时秒数（默认 30 秒）
    #[serde(default)]
    pub timeout_secs: Option<u64>,
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
            timeout_secs: None,
        }
    }

    /// 设置自定义请求超时
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
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
        let timeout_secs = config.timeout_secs.unwrap_or(30);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
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
        let messages_json: Vec<Value> = messages.iter().map(Message::to_openai_json).collect();

        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "messages": messages_json,
            "max_tokens": options.max_tokens,
            "temperature": options.temperature
        });

        merge_request_options(
            &mut request_body,
            &self.config.model,
            options.reasoning.as_ref(),
            options.custom_body.as_ref(),
        );

        // 打印完整请求信息用于调试
        tracing::info!(
            "[DEBUG] OpenAI 请求: endpoint={}, model={}, api_key_len={}, max_tokens={}, temperature={}",
            self.config.endpoint,
            self.config.model,
            self.config.api_key.len(),
            options.max_tokens,
            options.temperature
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

        let body = response.text().await?;
        if body.is_empty() {
            anyhow::bail!("OpenAI API 返回空响应体");
        }
        let payload: Value = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "OpenAI API 返回非 JSON 响应: {} (前100字符: {})",
                e,
                &body[..body.len().min(100)]
            )
        })?;

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

    /// OpenAI Chat Completions SSE streaming 调用。
    ///
    /// 保留 `chat()` 的非 streaming 行为给润色和学习链路使用；AI 助手工具调用走本接口。
    /// reqwest 0.11 没有 per-read timeout，这里用 `tokio::time::timeout` 包裹每个 chunk。
    pub async fn chat_stream<F>(
        &self,
        messages: &[Message],
        options: ChatOptions,
        tools: Option<Vec<ToolDefinition>>,
        cancel_token: CancellationToken,
        mut on_chunk: F,
    ) -> Result<ChatStreamResponse>
    where
        F: FnMut(StreamChunk) + Send,
    {
        if messages.is_empty() {
            return Ok(ChatStreamResponse::default());
        }

        let messages_json: Vec<Value> = messages.iter().map(Message::to_openai_json).collect();
        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "messages": messages_json,
            "max_tokens": options.max_tokens,
            "temperature": options.temperature,
            "stream": true
        });

        if let Some(tools) = tools {
            if !tools.is_empty() {
                request_body["tools"] = serde_json::to_value(tools)?;
                request_body["tool_choice"] = Value::String("auto".to_string());
            }
        }

        merge_request_options(
            &mut request_body,
            &self.config.model,
            options.reasoning.as_ref(),
            options.custom_body.as_ref(),
        );

        tracing::info!(
            "[DEBUG] OpenAI stream 请求: endpoint={}, model={}, api_key_len={}, max_tokens={}, temperature={}",
            self.config.endpoint,
            self.config.model,
            self.config.api_key.len(),
            options.max_tokens,
            options.temperature
        );

        let response = loop {
            let request = self
                .client
                .post(&self.config.endpoint)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body);
            let response = tokio::select! {
                _ = cancel_token.cancelled() => anyhow::bail!("AI 助手生成已取消"),
                response = request.send() => response?,
            };
            let status = response.status();
            if status.is_success() {
                break response;
            }
            let text = tokio::select! {
                _ = cancel_token.cancelled() => anyhow::bail!("AI 助手生成已取消"),
                text = response.text() => text.unwrap_or_default(),
            };
            // Only retry an explicit protocol rejection before generation starts.
            // Authentication, quota, network failures and partial output never retry.
            if request_body["stream"] == true && rejects_streaming(status.as_u16(), &text) {
                request_body["stream"] = Value::Bool(false);
                continue;
            }
            anyhow::bail!("OpenAI API 请求失败 ({}): {}", status, text);
        };

        let is_json = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .eq_ignore_ascii_case("application/json")
            });
        if is_json || request_body["stream"] == false {
            let payload: Value = tokio::select! {
                _ = cancel_token.cancelled() => anyhow::bail!("AI 助手生成已取消"),
                payload = response.json() => payload?,
            };
            let chunk = completion_as_stream_chunk(&payload)?;
            on_chunk(chunk.clone());
            return Ok(accumulate_stream_chunks(&[chunk]));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut pending_utf8 = Vec::new();
        let mut chunks = Vec::new();
        let read_timeout = Duration::from_secs(self.config.timeout_secs.unwrap_or(30).max(1));

        loop {
            let next = tokio::select! {
                _ = cancel_token.cancelled() => {
                    anyhow::bail!("AI 助手生成已取消");
                }
                next = tokio::time::timeout(read_timeout, stream.next()) => next,
            };

            let Some(item) = next.map_err(|_| anyhow::anyhow!("OpenAI stream 读取超时"))?
            else {
                break;
            };

            let bytes = item?;
            // TCP/HTTP chunk boundaries can split a Chinese character or emoji.
            pending_utf8.extend_from_slice(&bytes);
            let valid_len = match std::str::from_utf8(&pending_utf8) {
                Ok(text) => text.len(),
                Err(error) if error.error_len().is_none() => error.valid_up_to(),
                Err(error) => return Err(error.into()),
            };
            buffer.push_str(std::str::from_utf8(&pending_utf8[..valid_len])?);
            pending_utf8.drain(..valid_len);

            while let Some(event) = pop_next_sse_event(&mut buffer) {
                if let Some(chunk) = parse_sse_event(&event)? {
                    on_chunk(chunk.clone());
                    chunks.push(chunk);
                }
            }
        }

        anyhow::ensure!(
            pending_utf8.is_empty(),
            "OpenAI stream 返回不完整 UTF-8 数据"
        );
        if !buffer.trim().is_empty() {
            if let Some(chunk) = parse_sse_event(&buffer)? {
                on_chunk(chunk.clone());
                chunks.push(chunk);
            }
        }

        Ok(accumulate_stream_chunks(&chunks))
    }
}

fn rejects_streaming(status: u16, text: &str) -> bool {
    if !matches!(status, 400 | 422 | 501) {
        return false;
    }
    let text = text.to_lowercase();
    text.contains("stream")
        && [
            "not supported",
            "unsupported",
            "not implemented",
            "does not support",
            "不支持",
        ]
        .iter()
        .any(|phrase| text.contains(phrase))
}

fn completion_as_stream_chunk(payload: &Value) -> Result<StreamChunk> {
    let choice = payload["choices"]
        .as_array()
        .and_then(|choices| choices.first())
        .ok_or_else(|| anyhow::anyhow!("OpenAI API 返回缺少 choices 的响应"))?;
    let message = &choice["message"];
    let tool_calls: Vec<ToolCall> = match message.get("tool_calls").filter(|value| !value.is_null())
    {
        Some(calls) => serde_json::from_value(calls.clone())?,
        None => Vec::new(),
    };
    Ok(StreamChunk {
        delta_content: message["content"].as_str().map(str::to_string),
        finish_reason: choice["finish_reason"].as_str().map(str::to_string),
        delta_tool_calls: tool_calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| StreamToolCallDelta {
                index,
                id: Some(call.id),
                call_type: Some(call.call_type),
                function_name: Some(call.function.name),
                function_arguments_delta: Some(call.function.arguments),
            })
            .collect(),
    })
}

#[cfg(test)]
pub(crate) fn parse_sse_events(input: &str) -> Result<Vec<StreamChunk>> {
    let normalized = input.replace("\r\n", "\n");
    let mut chunks = Vec::new();

    for event in normalized.split("\n\n") {
        if let Some(chunk) = parse_sse_event(event)? {
            chunks.push(chunk);
        }
    }

    Ok(chunks)
}

fn pop_next_sse_event(buffer: &mut String) -> Option<String> {
    let lf = buffer.find("\n\n").map(|pos| (pos, 2));
    let crlf = buffer.find("\r\n\r\n").map(|pos| (pos, 4));
    let (pos, delimiter_len) = match (lf, crlf) {
        (Some(lf), Some(crlf)) => {
            if lf.0 < crlf.0 {
                lf
            } else {
                crlf
            }
        }
        (Some(lf), None) => lf,
        (None, Some(crlf)) => crlf,
        (None, None) => return None,
    };

    let event = buffer[..pos].to_string();
    let next = buffer[pos + delimiter_len..].to_string();
    *buffer = next;
    Some(event)
}

fn parse_sse_event(event: &str) -> Result<Option<StreamChunk>> {
    let mut data_lines = Vec::new();
    for line in event.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }

    if data_lines.is_empty() {
        return Ok(None);
    }

    let data = data_lines.join("\n");
    if data.trim() == "[DONE]" {
        return Ok(None);
    }

    let payload: Value = serde_json::from_str(&data).map_err(|e| {
        anyhow::anyhow!(
            "OpenAI stream 返回非 JSON 数据: {} (片段: {})",
            e,
            data.chars().take(120).collect::<String>()
        )
    })?;

    let Some(choice) = payload["choices"].as_array().and_then(|arr| arr.first()) else {
        return Ok(None);
    };

    let delta = &choice["delta"];
    let delta_content = delta["content"].as_str().map(ToString::to_string);
    let finish_reason = choice["finish_reason"].as_str().map(ToString::to_string);
    let mut delta_tool_calls = Vec::new();

    if let Some(calls) = delta["tool_calls"].as_array() {
        for call in calls {
            let index = call["index"].as_u64().unwrap_or(0) as usize;
            delta_tool_calls.push(StreamToolCallDelta {
                index,
                id: call["id"].as_str().map(ToString::to_string),
                call_type: call["type"].as_str().map(ToString::to_string),
                function_name: call["function"]["name"].as_str().map(ToString::to_string),
                function_arguments_delta: call["function"]["arguments"]
                    .as_str()
                    .map(ToString::to_string),
            });
        }
    }

    Ok(Some(StreamChunk {
        delta_content,
        delta_tool_calls,
        finish_reason,
    }))
}

pub(crate) fn accumulate_stream_chunks(chunks: &[StreamChunk]) -> ChatStreamResponse {
    #[derive(Default)]
    struct PartialToolCall {
        id: Option<String>,
        call_type: Option<String>,
        function_name: Option<String>,
        arguments: String,
    }

    let mut content = String::new();
    let mut finish_reason = None;
    let mut partials: Vec<PartialToolCall> = Vec::new();

    for chunk in chunks {
        if let Some(delta) = &chunk.delta_content {
            content.push_str(delta);
        }
        if let Some(reason) = &chunk.finish_reason {
            finish_reason = Some(reason.clone());
        }

        for delta in &chunk.delta_tool_calls {
            if partials.len() <= delta.index {
                partials.resize_with(delta.index + 1, PartialToolCall::default);
            }
            let partial = &mut partials[delta.index];
            if let Some(id) = &delta.id {
                partial.id = Some(id.clone());
            }
            if let Some(call_type) = &delta.call_type {
                partial.call_type = Some(call_type.clone());
            }
            if let Some(name) = &delta.function_name {
                partial.function_name = Some(name.clone());
            }
            if let Some(arguments) = &delta.function_arguments_delta {
                partial.arguments.push_str(arguments);
            }
        }
    }

    let tool_calls = partials
        .into_iter()
        .filter_map(|partial| {
            let id = partial.id?;
            let name = partial.function_name?;
            Some(ToolCall {
                id,
                call_type: partial.call_type.unwrap_or_else(|| "function".to_string()),
                function: ToolFunctionCall {
                    name,
                    arguments: partial.arguments,
                },
            })
        })
        .collect();

    ChatStreamResponse {
        content,
        tool_calls,
        finish_reason,
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
        assert_eq!(polishing.temperature, 0.7);

        let smart = ChatOptions::for_smart_command();
        assert_eq!(smart.max_tokens, 2048);
        assert_eq!(smart.temperature, 0.5);

        let arbitration = ChatOptions::for_candidate_arbitration();
        assert_eq!(arbitration.max_tokens, 256);
        assert_eq!(arbitration.temperature, 0.1);
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
    fn test_parse_sse_content_stream() {
        let fixture = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );

        let chunks = parse_sse_events(fixture).expect("SSE fixture 必须可解析");

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].delta_content.as_deref(), Some("你"));
        assert_eq!(chunks[1].delta_content.as_deref(), Some("好"));
        assert_eq!(chunks[1].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn test_parse_sse_tool_call_stream() {
        let fixture = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"search_web\",\"arguments\":\"{\\\"query\\\":\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"OpenAI news\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );

        let chunks = parse_sse_events(fixture).expect("SSE fixture 必须可解析");
        let response = accumulate_stream_chunks(&chunks);

        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id, "call_1");
        assert_eq!(response.tool_calls[0].function.name, "search_web");
        assert_eq!(
            response.tool_calls[0].function.arguments,
            "{\"query\":\"OpenAI news\"}"
        );
    }

    #[test]
    fn test_pop_next_sse_event_supports_crlf_and_lf_boundaries() {
        let mut buffer = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"},\"finish_reason\":null}]}\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\r\n\r\n",
        )
        .to_string();
        let mut chunks = Vec::new();

        while let Some(event) = pop_next_sse_event(&mut buffer) {
            if let Some(chunk) = parse_sse_event(&event).expect("SSE event 必须可解析") {
                chunks.push(chunk);
            }
        }

        assert!(buffer.is_empty());
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].delta_content.as_deref(), Some("你"));
        assert_eq!(chunks[1].delta_content.as_deref(), Some("好"));
        assert_eq!(chunks[1].finish_reason.as_deref(), Some("stop"));
    }
}

#[cfg(test)]
#[path = "openai_client_compatibility_tests.rs"]
mod compatibility_tests;
