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

/// LongCat API 端点
const LONGCAT_ENDPOINT: &str = "https://api.longcat.chat/openai/v1/chat/completions";

/// WAV 文件头魔数
const WAV_MAGIC: &[u8; 4] = b"RIFF";

#[derive(Clone)]
pub struct OmniAsrClient {
    api_key: String,
    model: String,
    system_prompt: String,
    client: reqwest::Client,
}

impl OmniAsrClient {
    /// 创建 Omni ASR 客户端
    ///
    /// # Arguments
    /// * `api_key` - LongCat API 密钥
    /// * `model` - 模型名称（如 "LongCat-Flash-Omni-2603"）
    /// * `dictionary` - 用户词库条目（可能含 |auto 后缀）
    /// * `builtin_hotwords_raw` - 内置词库原始文本（【领域】:[词1,词2,...] 格式）
    /// * `include_builtin` - 是否在 prompt 中包含内置词库
    /// * `custom_rules` - 用户自定义转录规则
    pub fn new(
        api_key: String,
        model: String,
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
            "Omni ASR 客户端已创建: model={}, prompt_len={} 字符",
            model,
            system_prompt.len()
        );

        Self {
            api_key,
            model,
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

        let request_body = serde_json::json!({
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
            "topP": 0.1,
            "topK": 1,
            "output_modalities": ["text"]
        });

        tracing::info!("Omni ASR: 发送请求到 {}", LONGCAT_ENDPOINT);

        let response = self
            .client
            .post(LONGCAT_ENDPOINT)
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

        // 解析 choices[0].message.content
        // content 可能是 string 或 array of {type, text}
        let text = Self::extract_content_text(&result)?;

        let mut text = text.to_string();
        utils::strip_trailing_punctuation(&mut text);

        tracing::info!("Omni ASR: 转录完成: {}", text);
        Ok(text)
    }

    /// 热更新词库（重建 system prompt）
    pub fn update_dictionary(
        &mut self,
        dictionary: Vec<String>,
        builtin_hotwords_raw: &str,
        include_builtin: bool,
        custom_rules: &str,
    ) {
        self.system_prompt = prompt_builder::build(
            &dictionary,
            builtin_hotwords_raw,
            include_builtin,
            custom_rules,
        );
        tracing::info!(
            "Omni ASR: 词库已热更新, prompt_len={} 字符",
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
