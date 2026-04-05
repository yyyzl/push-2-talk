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
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
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
        include_builtin: bool,
        custom_rules: &str,
    ) -> Self {
        let system_prompt = prompt_builder::build(
            &dictionary,
            builtin_hotwords_raw,
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
    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        tracing::info!(
            "Omni ASR: 音频数据 {} bytes, base64 {} 字符",
            audio_data.len(),
            audio_base64.len()
        );

        // 检测音频格式
        let audio_format = if audio_data.len() >= 4 && &audio_data[..4] == WAV_MAGIC {
            "wav"
        } else {
            "wav" // 默认按 WAV 处理
        };

        let mut request_body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": [{"type": "text", "text": self.system_prompt}]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_audio",
                            "input_audio": {
                                "type": "base64",
                                "data": audio_base64,
                                "format": audio_format
                            }
                        },
                        {"type": "text", "text": "请转录这段语音。"}
                    ]
                }
            ],
            "stream": false,
            "temperature": 0.01,
            "top_p": 0.1,
            "top_k": 1,
            "output_modalities": ["text"]
        });

        // thinking 模式：仅对支持 thinking 的服务商发送参数
        if self.thinking_supported {
            request_body["chat_template_kwargs"] =
                serde_json::json!({"enable_thinking": self.enable_thinking});
            if self.enable_thinking {
                request_body["thinking"] = serde_json::json!({"type": "enabled"});
            }
            tracing::info!("Omni ASR: thinking={} (supported)", self.enable_thinking);
        } else {
            tracing::info!("Omni ASR: thinking 不支持，跳过参数");
        }

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
        tracing::info!("Omni ASR: 响应状态 {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            tracing::error!("Omni ASR: API 错误响应: {}", error_text);
            anyhow::bail!("Omni ASR 请求失败 ({}): {}", status, error_text);
        }

        let result: serde_json::Value = response.json().await?;
        tracing::info!(
            "Omni ASR: 响应: {}",
            serde_json::to_string_pretty(&result)?
        );

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

        let mut text = text.to_string();
        utils::strip_trailing_punctuation(&mut text);

        tracing::info!("Omni ASR: 转录完成: {}", text);
        Ok(text)
    }

    /// 热更新配置（重建 system prompt，同步 endpoint / thinking）
    pub fn update_config(
        &mut self,
        dictionary: Vec<String>,
        builtin_hotwords_raw: &str,
        include_builtin: bool,
        custom_rules: &str,
        endpoint: &str,
        enable_thinking: bool,
        thinking_supported: bool,
    ) {
        self.system_prompt = prompt_builder::build(
            &dictionary,
            builtin_hotwords_raw,
            include_builtin,
            custom_rules,
        );
        self.endpoint = endpoint.to_string();
        self.enable_thinking = enable_thinking;
        self.thinking_supported = thinking_supported;
        tracing::info!(
            "Omni ASR: 配置已热更新, endpoint={}, thinking={} (supported={}), prompt_len={} 字符",
            self.endpoint,
            self.enable_thinking,
            self.thinking_supported,
            self.system_prompt.len()
        );
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
}
