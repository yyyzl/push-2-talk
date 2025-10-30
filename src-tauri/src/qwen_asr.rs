// Qwen ASR 客户端模块
use std::path::Path;
use std::time::Duration;
use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};

pub struct QwenASRClient {
    api_key: String,
    client: reqwest::Client,
    max_retries: u32,
}

impl QwenASRClient {
    pub fn new(api_key: String) -> Self {
        // 创建带超时配置的HTTP客户端
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))  // 10秒总超时
            .connect_timeout(Duration::from_secs(5))  // 5秒连接超时
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            api_key,
            client,
            max_retries: 2,  // 最多重试2次
        }
    }

    pub async fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let mut last_error = None;

        // 尝试转录，包含重试逻辑
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tracing::warn!("第 {} 次重试转录...", attempt);
            }

            match self.transcribe_internal(audio_path).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    tracing::error!("转录失败 (尝试 {}/{}): {}", attempt + 1, self.max_retries + 1, e);
                    last_error = Some(e);

                    // 如果不是最后一次尝试，等待一小段时间再重试
                    if attempt < self.max_retries {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }

        // 所有尝试都失败
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("转录失败，未知错误")))
    }

    async fn transcribe_internal(&self, audio_path: &Path) -> Result<String> {
        tracing::info!("开始转录音频文件: {:?}", audio_path);

        // 读取音频文件并转换为 base64
        let audio_data = tokio::fs::read(audio_path).await?;
        let audio_base64 = general_purpose::STANDARD.encode(&audio_data);

        tracing::info!("音频文件大小: {} bytes", audio_data.len());

        // 构建请求体 - 使用 qwen3-asr-flash 的多模态对话 API
        let request_body = serde_json::json!({
            "model": "qwen3-asr-flash",
            "input": {
                "messages": [
                    {
                        "role": "system",
                        "content": [
                            {"text": ""}
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "audio": format!("data:audio/wav;base64,{}", audio_base64)
                            }
                        ]
                    }
                ]
            },
            "parameters": {
                "result_format": "message",
                "enable_itn": true
            }
        });

        // 正确的 qwen3-asr-flash API endpoint
        let url = "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";

        tracing::info!("发送请求到: {}", url);

        // 发送请求到 DashScope API
        let response = self
            .client
            .post(url)
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

        // 解析响应 - qwen3-asr-flash 的响应格式
        let mut text = result["output"]["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|choice| choice["message"]["content"].as_array())
            .and_then(|content| content.first())
            .and_then(|item| item["text"].as_str())
            .ok_or_else(|| anyhow::anyhow!("无法解析转录结果，响应格式: {:?}", result))?
            .to_string();

        // 去除末尾的标点符号
        let punctuation = ['。', '，', '！', '？', '、', '；', '：', '"', '"', '\'', '\'', '.', ',', '!', '?', ';', ':'];
        while let Some(last_char) = text.chars().last() {
            if punctuation.contains(&last_char) {
                text.pop();
            } else {
                break;
            }
        }

        tracing::info!("转录完成: {}", text);
        Ok(text)
    }
}
