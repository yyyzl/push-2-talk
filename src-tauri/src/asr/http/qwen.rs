use std::time::Duration;
use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use crate::asr::utils;

const QWEN_API_URL: &str = "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
const MODEL: &str = "qwen3-asr-flash";
const MAX_RETRIES: u32 = 2;

#[derive(Clone)]
pub struct QwenASRClient {
    api_key: String,
    client: reqwest::Client,
    max_retries: u32,
    dictionary: Vec<String>,
}

impl QwenASRClient {
    pub fn new(api_key: String, dictionary: Vec<String>) -> Self {
        Self {
            api_key,
            client: utils::create_http_client(),
            max_retries: MAX_RETRIES,
            dictionary,
        }
    }

    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tracing::warn!("第 {} 次重试转录...", attempt);
            }

            match self.transcribe_from_memory(audio_data).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    tracing::error!("转录失败 (尝试 {}/{}): {}", attempt + 1, self.max_retries + 1, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("转录失败，未知错误")))
    }

    pub(crate) async fn transcribe_from_memory(&self, audio_data: &[u8]) -> Result<String> {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        tracing::info!("音频数据大小: {} bytes", audio_data.len());

        // 词库用顿号分隔
        let corpus_text = self.dictionary.join("、");
        if !corpus_text.is_empty() {
            tracing::info!("Qwen HTTP ASR 词库: {} 个词, corpus={}", self.dictionary.len(), corpus_text);
        } else {
            tracing::info!("Qwen HTTP ASR 词库: 未配置");
        }

        let request_body = serde_json::json!({
            "model": MODEL,
            "input": {
                "messages": [
                    {
                        "role": "system",
                        "content": [{"text": corpus_text}]
                    },
                    {
                        "role": "user",
                        "content": [{"audio": format!("data:audio/wav;base64,{}", audio_base64)}]
                    }
                ]
            },
            "parameters": {
                "result_format": "message",
                "enable_itn": false,
                "disfluency_removal": true,
                "language": "zh"
            }
        });

        tracing::info!("发送请求到: {}", QWEN_API_URL);

        let response = self
            .client
            .post(QWEN_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("API 响应状态: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            tracing::error!("API 错误响应: {}", error_text);
            anyhow::bail!("API 请求失败 ({}): {}", status, error_text);
        }

        let result: serde_json::Value = response.json().await?;
        tracing::info!("API 响应: {}", serde_json::to_string_pretty(&result)?);

        let mut text = result["output"]["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|choice| choice["message"]["content"].as_array())
            .and_then(|content| content.first())
            .and_then(|item| item["text"].as_str())
            .ok_or_else(|| anyhow::anyhow!("无法解析转录结果，响应格式: {:?}", result))?
            .to_string();

        utils::strip_trailing_punctuation(&mut text);

        // 检测词库回显（千问特有问题：录音为空时返回词库内容）
        if !corpus_text.is_empty() && text == corpus_text {
            tracing::warn!("检测到词库回显，过滤无效结果");
            anyhow::bail!("录音无效，已跳过");
        }

        tracing::info!("转录完成: {}", text);
        Ok(text)
    }
}
