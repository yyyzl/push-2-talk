use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

use crate::config::LlmConfig;

#[derive(Clone)]
pub struct LlmPostProcessor {
    config: LlmConfig,
    client: Client,
}

impl LlmPostProcessor {
    pub fn new(config: LlmConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(600))
            .pool_max_idle_per_host(2)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { config, client }
    }

    pub async fn polish_transcript(&self, raw_text: &str) -> Result<String> {
        if raw_text.trim().is_empty() {
            return Ok(String::new());
        }

        // 使用 OpenAI 兼容格式（大多数 API 都支持）
        let request_body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": self.config.system_prompt
                },
                {
                    "role": "user",
                    "content": format!("<ASR转写的文本>\n{}\n</ASR转写的文本>", raw_text)
                }
            ],
            "max_tokens": 800,
            "temperature": 0.3
        });

        tracing::debug!("LLM 请求: endpoint={}, model={}", self.config.endpoint, self.config.model);

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
            anyhow::bail!("LLM 处理失败 ({}): {}", status, text);
        }

        let payload: Value = response.json().await?;

        // 尝试解析 OpenAI 格式的响应
        let refined = payload["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|choice| choice["message"]["content"].as_str())
            .ok_or_else(|| anyhow::anyhow!("LLM 返回格式不可解析: {:?}", payload))?;

        Ok(refined.trim().to_string())
    }
}
