// Omni 多模态 ASR 客户端
//
// 通过 OpenAI 兼容的多模态 API 进行语音转录。
// 将音频数据和结构化 prompt（含词库+转录规则）一起发送给 LLM，
// 模型在转录时同步理解语义，实现高精度术语命中。
//
// 支持任何兼容 OpenAI Chat Completions 接口的 Omni 模型，
// 如 LongCat-Flash-Omni、Qwen-Omni 等。

mod prompt_builder;

use crate::asr::utils;
use crate::config;
use crate::window_capture::FocusedWindowScreenshot;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use std::time::Duration;

/// Omni ASR 请求超时时间（秒）
const REQUEST_TIMEOUT_SECS: u64 = 90;

/// WAV 文件头魔数
const WAV_MAGIC: &[u8; 4] = b"RIFF";

#[derive(Clone)]
pub struct OmniAsrClient {
    api_key: String,
    model: String,
    endpoint: String,
    enable_thinking: bool,
    /// 当前服务商是否支持 thinking 参数（LongCat 不支持，MiMo 支持）
    thinking_supported: bool,
    system_prompt: String,
    client: reqwest::Client,
}

impl OmniAsrClient {
    /// 创建 Omni ASR 客户端
    pub fn new(
        api_key: String,
        model: String,
        endpoint: String,
        enable_thinking: bool,
        thinking_supported: bool,
        dictionary: Vec<String>,
        builtin_hotwords_raw: &str,
        selected_builtin_domains: &[String],
        include_builtin: bool,
        custom_rules: &str,
    ) -> Self {
        let endpoint = config::normalize_chat_completions_endpoint(&endpoint);
        let user_dictionary_count = dictionary.len();
        let builtin_line_count = builtin_hotwords_raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .count();
        let custom_rules_len = custom_rules.trim().chars().count();
        let system_prompt = prompt_builder::build(
            &dictionary,
            builtin_hotwords_raw,
            selected_builtin_domains,
            include_builtin,
            custom_rules,
        );

        tracing::info!(
            "Omni ASR 客户端已创建: model={}, endpoint={}, thinking={} (supported={}), prompt_len={} 字符",
            model,
            endpoint,
            enable_thinking,
            thinking_supported,
            system_prompt.len()
        );
        tracing::info!(
            "Omni ASR Prompt 构建详情: user_dict_count={}, include_builtin={}, builtin_line_count={}, custom_rules_chars={}",
            user_dictionary_count,
            include_builtin,
            builtin_line_count,
            custom_rules_len
        );
        tracing::info!("Omni ASR Prompt 全文:\n{}", system_prompt);

        Self {
            api_key,
            model,
            endpoint,
            enable_thinking,
            thinking_supported,
            system_prompt,
            client: utils::create_http_client(),
        }
    }

    /// 转录音频数据
    ///
    /// 将音频 Base64 编码后，与 system prompt 一起发送到 Omni 模型进行转录。
    pub async fn transcribe_bytes(
        &self,
        audio_data: &[u8],
        screenshot: Option<FocusedWindowScreenshot>,
    ) -> Result<String> {
        let audio_base64_len = general_purpose::STANDARD.encode(audio_data).len();
        tracing::info!(
            "Omni ASR: 音频数据 {} bytes, base64 {} 字符, screenshot={}",
            audio_data.len(),
            audio_base64_len,
            screenshot.is_some()
        );

        let request_body = Self::build_request_body(
            &self.model,
            &self.system_prompt,
            self.thinking_supported,
            self.enable_thinking,
            audio_data,
            screenshot.as_ref(),
        );

        // thinking 模式：仅对支持 thinking 的服务商发送参数
        if self.thinking_supported {
            tracing::info!("Omni ASR: thinking={} (supported)", self.enable_thinking);
        } else {
            tracing::info!("Omni ASR: thinking 不支持，跳过参数");
        }

        tracing::info!(
            "Omni ASR: 请求摘要 endpoint={}, model={}, audio_format={}, audio_bytes={}, audio_base64_chars={}, prompt_chars={}, thinking_enabled={}, thinking_supported={}, screenshot={}, timeout_secs={}",
            self.endpoint,
            self.model,
            "wav",
            audio_data.len(),
            audio_base64_len,
            self.system_prompt.chars().count(),
            self.enable_thinking,
            self.thinking_supported,
            screenshot.is_some(),
            REQUEST_TIMEOUT_SECS
        );
        tracing::info!("Omni ASR: 本次 system prompt 全文:\n{}", self.system_prompt);
        let request_preview = Self::redact_request_body_for_logging(
            &request_body,
            audio_data.len(),
            audio_base64_len,
        );
        tracing::info!(
            "Omni ASR: 请求体预览（音频已脱敏）:\n{}",
            serde_json::to_string_pretty(&request_preview)?
        );
        tracing::info!("Omni ASR: 发送请求到 {}", self.endpoint);

        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        tracing::info!("Omni ASR: 响应状态 {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            tracing::error!("Omni ASR: API 错误响应: {}", error_text);
            anyhow::bail!("Omni ASR 请求失败 ({}): {}", status, error_text);
        }

        let response_text = response.text().await?;
        tracing::info!(
            "Omni ASR: 原始响应预览 (content-type={}): {}",
            content_type,
            Self::preview_text(&response_text)
        );
        let result = Self::parse_response_json(&response_text)?;
        tracing::info!("Omni ASR: 响应: {}", serde_json::to_string_pretty(&result)?);

        // 如有 reasoning_content，记录日志（不影响转录结果）
        if let Some(reasoning) = result
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("reasoning_content"))
            .and_then(|r| r.as_str())
        {
            let preview: String = reasoning.chars().take(100).collect();
            tracing::info!(
                "Omni ASR: thinking 推理过程 ({} 字符): {}",
                reasoning.chars().count(),
                preview
            );
        }

        // 解析 choices[0].message.content
        // content 可能是 string 或 array of {type, text}
        let text = Self::extract_content_text(&result)?;
        let text = Self::finalize_transcript_text(&text);

        tracing::info!("Omni ASR: 转录完成: {}", text);
        Ok(text)
    }

    /// 热更新配置（重建 system prompt，同步 endpoint / thinking）
    pub fn update_config(
        &mut self,
        dictionary: Vec<String>,
        builtin_hotwords_raw: &str,
        selected_builtin_domains: &[String],
        include_builtin: bool,
        custom_rules: &str,
        endpoint: &str,
        enable_thinking: bool,
        thinking_supported: bool,
    ) {
        let user_dictionary_count = dictionary.len();
        let builtin_line_count = builtin_hotwords_raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .count();
        let custom_rules_len = custom_rules.trim().chars().count();
        self.system_prompt = prompt_builder::build(
            &dictionary,
            builtin_hotwords_raw,
            selected_builtin_domains,
            include_builtin,
            custom_rules,
        );
        self.endpoint = config::normalize_chat_completions_endpoint(endpoint);
        self.enable_thinking = enable_thinking;
        self.thinking_supported = thinking_supported;
        tracing::info!(
            "Omni ASR: 配置已热更新, endpoint={}, thinking={} (supported={}), prompt_len={} 字符",
            self.endpoint,
            self.enable_thinking,
            self.thinking_supported,
            self.system_prompt.len()
        );
        tracing::info!(
            "Omni ASR Prompt 热更新详情: user_dict_count={}, include_builtin={}, builtin_line_count={}, custom_rules_chars={}",
            user_dictionary_count,
            include_builtin,
            builtin_line_count,
            custom_rules_len
        );
        tracing::info!("Omni ASR Prompt 热更新全文:\n{}", self.system_prompt);
    }

    /// 从 API 响应中提取转录文本
    ///
    /// 兼容两种 content 格式：
    /// 1. content 为字符串: `"content": "转录文本"`
    /// 2. content 为数组: `"content": [{"type": "text", "text": "转录文本"}]`
    fn extract_content_text(result: &serde_json::Value) -> Result<String> {
        let content = result
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Omni ASR: 无法解析响应格式，缺少 choices[0].message.content: {:?}",
                    result
                )
            })?;

        // 尝试作为字符串解析
        if let Some(text) = content.as_str() {
            return Ok(text.to_string());
        }

        // 尝试作为数组解析
        if let Some(arr) = content.as_array() {
            for item in arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        return Ok(text.to_string());
                    }
                }
            }
            // 如果数组中没有 type=text 的项，尝试取第一个有 text 字段的
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        anyhow::bail!(
            "Omni ASR: 无法从 content 中提取文本，content 格式: {:?}",
            content
        )
    }

    fn parse_response_json(response_text: &str) -> Result<Value> {
        let trimmed = response_text.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Omni ASR: 服务端返回空响应体");
        }

        if let Ok(result) = serde_json::from_str::<Value>(trimmed) {
            return Ok(result);
        }

        if let Some(fenced) = Self::extract_fenced_json(trimmed) {
            if let Ok(result) = serde_json::from_str::<Value>(&fenced) {
                return Ok(result);
            }
        }

        if let Some(sse_payload) = Self::extract_sse_json(trimmed) {
            if let Ok(result) = serde_json::from_str::<Value>(&sse_payload) {
                return Ok(result);
            }
        }

        anyhow::bail!(
            "Omni ASR: 服务端返回的不是可解析 JSON，原始响应预览: {}",
            Self::preview_text(trimmed)
        )
    }

    fn extract_fenced_json(text: &str) -> Option<String> {
        let stripped = text
            .strip_prefix("```json")
            .or_else(|| text.strip_prefix("```"))?;
        let stripped = stripped.trim_start_matches(['\r', '\n']);
        let stripped = stripped.strip_suffix("```")?;
        Some(stripped.trim().to_string())
    }

    fn extract_sse_json(text: &str) -> Option<String> {
        text.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("data:"))
            .map(|line| line.trim_start_matches("data:").trim())
            .find(|payload| !payload.is_empty() && *payload != "[DONE]")
            .map(ToString::to_string)
    }

    fn preview_text(text: &str) -> String {
        const LIMIT: usize = 240;
        let normalized = text.replace('\r', "\\r").replace('\n', "\\n");
        let char_count = normalized.chars().count();
        if char_count <= LIMIT {
            normalized
        } else {
            let preview: String = normalized.chars().take(LIMIT).collect();
            format!("{}... ({} chars)", preview, char_count)
        }
    }

    fn redact_request_body_for_logging(
        request_body: &Value,
        audio_bytes_len: usize,
        audio_base64_len: usize,
    ) -> Value {
        let mut redacted = request_body.clone();
        if let Some(value) = redacted.pointer_mut("/messages/1/content/0/input_audio/data") {
            *value = Value::String(format!(
                "<omitted: {} base64 chars, {} raw bytes>",
                audio_base64_len, audio_bytes_len
            ));
        }
        if let Some(value) = redacted.pointer_mut("/messages/1/content/1/image_url/url") {
            *value = Value::String("<omitted: focused window screenshot data url>".to_string());
        }
        redacted
    }

    fn finalize_transcript_text(text: &str) -> String {
        text.strip_suffix('。')
            .or_else(|| text.strip_suffix('.'))
            .unwrap_or(text)
            .to_string()
    }

    /// LongCat 系列模型不支持在音频请求中附带 image_url 截图
    /// （官方 API 文档中无 audio+image 混合请求示例，实测会报错）
    fn is_longcat_model(model: &str) -> bool {
        model.trim().to_ascii_lowercase().contains("longcat")
    }

    fn build_request_body(
        model: &str,
        system_prompt: &str,
        thinking_supported: bool,
        enable_thinking: bool,
        audio_data: &[u8],
        screenshot: Option<&FocusedWindowScreenshot>,
    ) -> Value {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        let audio_format = if audio_data.len() >= 4 && &audio_data[..4] == WAV_MAGIC {
            "wav"
        } else {
            "wav"
        };

        // LongCat 不支持 audio+image 混合请求，截图对它无效
        let effective_screenshot = if Self::is_longcat_model(model) && screenshot.is_some() {
            tracing::info!("Omni ASR: LongCat 模型不支持 audio+image 混合请求，跳过截图");
            None
        } else {
            screenshot
        };

        let mut content = vec![serde_json::json!({
            "type": "input_audio",
            "input_audio": {
                "type": "base64",
                "data": audio_base64,
                "format": audio_format
            }
        })];

        if let Some(screenshot) = effective_screenshot {
            content.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": format!(
                        "data:{};base64,{}",
                        screenshot.mime_type, screenshot.data_base64
                    )
                }
            }));
        }

        content.push(serde_json::json!({
            "type": "text",
            "text": Self::build_user_instruction_text(effective_screenshot.is_some())
        }));

        let mut request_body = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": [{"type": "text", "text": system_prompt}]
                },
                {
                    "role": "user",
                    "content": content
                }
            ],
            "stream": false,
            "temperature": 0.01,
            "top_p": 0.1,
            "top_k": 1,
            "output_modalities": ["text"]
        });

        if let Some(reasoning_effort) = Self::gemini_min_reasoning_effort(model) {
            tracing::info!(
                "Omni ASR: Gemini reasoning_effort injected: model={}, reasoning_effort={}",
                model,
                reasoning_effort
            );
            request_body["reasoning_effort"] = serde_json::json!(reasoning_effort);
        }

        if thinking_supported {
            request_body["chat_template_kwargs"] =
                serde_json::json!({"enable_thinking": enable_thinking});
            if enable_thinking {
                request_body["thinking"] = serde_json::json!({"type": "enabled"});
            }
        }

        request_body
    }

    fn build_user_instruction_text(has_screenshot: bool) -> String {
        if has_screenshot {
            "请转录这段语音。截图仅作为辅助线索：优先忠实转录音频；仅在音频不清晰或术语可疑时结合截图纠错；不要凭截图臆造用户没说出的整句。".to_string()
        } else {
            "请转录这段语音。".to_string()
        }
    }

    fn gemini_min_reasoning_effort(model: &str) -> Option<&'static str> {
        let model = model.trim().to_ascii_lowercase();
        if !model.starts_with("gemini-") {
            return None;
        }

        if model.starts_with("gemini-2.5-pro") {
            return Some("low");
        }

        if model.starts_with("gemini-2.5") {
            return Some("none");
        }

        if model.contains("pro") {
            return Some("low");
        }

        Some("minimal")
    }
}

#[cfg(test)]
mod tests {
    use super::OmniAsrClient;
    use crate::window_capture::FocusedWindowScreenshot;
    use serde_json::Value;

    #[test]
    fn parses_plain_json_response() {
        let parsed =
            OmniAsrClient::parse_response_json(r#"{"choices":[{"message":{"content":"ok"}}]}"#)
                .expect("plain json should parse");
        assert_eq!(parsed["choices"][0]["message"]["content"], "ok");
    }

    #[test]
    fn parses_fenced_json_response() {
        let parsed = OmniAsrClient::parse_response_json(
            "```json\n{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}\n```",
        )
        .expect("fenced json should parse");
        assert_eq!(parsed["choices"][0]["message"]["content"], "ok");
    }

    #[test]
    fn parses_sse_json_response() {
        let parsed = OmniAsrClient::parse_response_json(
            "data: {\"choices\":[{\"message\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n",
        )
        .expect("sse json should parse");
        assert_eq!(parsed["choices"][0]["message"]["content"], "ok");
    }

    #[test]
    fn strips_trailing_period_in_transcript() {
        assert_eq!(OmniAsrClient::finalize_transcript_text("你好。"), "你好");
    }

    #[test]
    fn keeps_non_period_trailing_punctuation_in_transcript() {
        assert_eq!(OmniAsrClient::finalize_transcript_text("你好！"), "你好！");
        assert_eq!(OmniAsrClient::finalize_transcript_text("你好？"), "你好？");
    }

    #[test]
    fn build_request_body_without_screenshot_keeps_audio_only_payload() {
        let request = OmniAsrClient::build_request_body(
            "demo-model",
            "system prompt",
            false,
            false,
            b"RIFFdemo",
            None,
        );

        let content = request["messages"][1]["content"]
            .as_array()
            .expect("user content should be an array");

        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], Value::String("input_audio".to_string()));
        assert_eq!(content[1]["type"], Value::String("text".to_string()));
        assert!(content[1]["text"]
            .as_str()
            .expect("text prompt")
            .contains("请转录这段语音"));
    }

    #[test]
    fn build_request_body_with_screenshot_adds_image_context_prompt() {
        let screenshot = FocusedWindowScreenshot {
            mime_type: "image/png".to_string(),
            data_base64: "ZmFrZS1pbWFnZQ==".to_string(),
        };
        let request = OmniAsrClient::build_request_body(
            "demo-model",
            "system prompt",
            false,
            false,
            b"RIFFdemo",
            Some(&screenshot),
        );

        let content = request["messages"][1]["content"]
            .as_array()
            .expect("user content should be an array");

        assert_eq!(content.len(), 3);
        assert_eq!(content[1]["type"], Value::String("image_url".to_string()));
        assert_eq!(
            content[1]["image_url"]["url"],
            Value::String("data:image/png;base64,ZmFrZS1pbWFnZQ==".to_string())
        );
        let prompt = content[2]["text"].as_str().expect("text prompt");
        assert!(prompt.contains("截图仅作为辅助线索"));
        assert!(prompt.contains("不要凭截图臆造"));
    }

    #[test]
    fn build_request_body_for_gemini_3_flash_injects_minimal_reasoning() {
        let request = OmniAsrClient::build_request_body(
            "gemini-3-flash",
            "system prompt",
            false,
            false,
            b"RIFFdemo",
            None,
        );

        assert_eq!(request["reasoning_effort"], Value::String("minimal".to_string()));
        assert!(request.get("chat_template_kwargs").is_none());
        assert!(request.get("thinking").is_none());
    }

    #[test]
    fn build_request_body_for_gemini_25_flash_injects_none_reasoning() {
        let request = OmniAsrClient::build_request_body(
            "gemini-2.5-flash",
            "system prompt",
            false,
            false,
            b"RIFFdemo",
            None,
        );

        assert_eq!(request["reasoning_effort"], Value::String("none".to_string()));
    }

    #[test]
    fn build_request_body_for_non_gemini_omits_reasoning_effort() {
        let request = OmniAsrClient::build_request_body(
            "LongCat-Flash-Omni-2603",
            "system prompt",
            false,
            false,
            b"RIFFdemo",
            None,
        );

        assert!(request.get("reasoning_effort").is_none());
    }

    #[test]
    fn build_request_body_for_longcat_skips_screenshot() {
        let screenshot = FocusedWindowScreenshot {
            mime_type: "image/png".to_string(),
            data_base64: "ZmFrZS1pbWFnZQ==".to_string(),
        };
        let request = OmniAsrClient::build_request_body(
            "LongCat-Flash-Omni-2603",
            "system prompt",
            false,
            false,
            b"RIFFdemo",
            Some(&screenshot),
        );

        let content = request["messages"][1]["content"]
            .as_array()
            .expect("user content should be an array");

        // LongCat：即使传入截图，也应退化为仅 audio + text（2 项，无 image_url）
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], Value::String("input_audio".to_string()));
        assert_eq!(content[1]["type"], Value::String("text".to_string()));
        // 文本提示应是无截图版本
        let prompt = content[1]["text"].as_str().expect("text prompt");
        assert!(prompt.contains("请转录这段语音"));
        assert!(!prompt.contains("截图"));
    }

    #[test]
    fn is_longcat_model_detects_various_names() {
        assert!(OmniAsrClient::is_longcat_model("LongCat-Flash-Omni-2603"));
        assert!(OmniAsrClient::is_longcat_model("longcat-flash-omni"));
        assert!(OmniAsrClient::is_longcat_model("LONGCAT-NEXT"));
        assert!(!OmniAsrClient::is_longcat_model("gemini-2.5-flash"));
        assert!(!OmniAsrClient::is_longcat_model("qwen-omni"));
        assert!(!OmniAsrClient::is_longcat_model("grok-2-audio"));
    }
}
