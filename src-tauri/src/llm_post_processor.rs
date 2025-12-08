// src-tauri/src/llm_post_processor.rs

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
            .pool_idle_timeout(Duration::from_secs(30))  // 30秒空闲超时
            .pool_max_idle_per_host(10)  // 增加连接池大小
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { config, client }
    }

    // 辅助函数：获取当前激活的 Prompt
    fn get_active_system_prompt(&self) -> String {
        self.config.presets
            .iter()
            .find(|p| p.id == self.config.active_preset_id)
            .map(|p| p.system_prompt.clone())
            .unwrap_or_else(|| "You are a helpful assistant.".to_string())
    }

    pub async fn polish_transcript(&self, raw_text: &str) -> Result<String> {
        if raw_text.trim().is_empty() {
            return Ok(String::new());
        }

        let system_prompt = self.get_active_system_prompt();
        tracing::info!("LLM 使用预设 ID: {}", self.config.active_preset_id);

        // 使用 OpenAI 兼容格式
        let request_body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": format!("<ASR转写的文本>\n{}\n</ASR转写的文本>", raw_text)
                }
            ],
            "max_tokens": 1024, // 稍微调大一点以防万一
            "temperature": 0.3
        });

        // ... (其余代码保持不变)
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