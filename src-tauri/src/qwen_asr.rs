// ASR 客户端模块（支持千问和 SenseVoice）
use std::path::Path;
use std::sync::{Arc, Mutex};
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
            .timeout(Duration::from_secs(6))  // 6秒总超时
            .connect_timeout(Duration::from_secs(5))  // 5秒连接超时
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            api_key,
            client,
            max_retries: 2,  // 最多重试2次
        }
    }

    // 带重试逻辑的转录（用于单独使用千问时）- 文件版本
    pub async fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let audio_data = tokio::fs::read(audio_path).await?;
        self.transcribe_bytes(&audio_data).await
    }

    // 带重试逻辑的转录（用于单独使用千问时）- 内存版本
    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        let mut last_error = None;

        // 尝试转录，包含重试逻辑
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tracing::warn!("第 {} 次重试转录...", attempt);
            }

            match self.transcribe_from_memory(audio_data).await {
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

    // 单次请求，不带重试（用于主备并行时）
    pub async fn transcribe_once(&self, audio_path: &Path) -> Result<String> {
        tracing::info!("开始转录音频文件: {:?}", audio_path);

        // 读取音频文件并转换为 base64
        let audio_data = tokio::fs::read(audio_path).await?;
        self.transcribe_from_memory(&audio_data).await
    }

    /// 从内存中的 WAV 数据直接转录（跳过文件 I/O）
    pub async fn transcribe_from_memory(&self, audio_data: &[u8]) -> Result<String> {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);

        tracing::info!("音频数据大小: {} bytes", audio_data.len());

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

// SenseVoice 客户端（硅基流动）
pub struct SenseVoiceClient {
    api_key: String,
    client: reqwest::Client,
}

impl SenseVoiceClient {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))  // 6秒总超时
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { api_key, client }
    }

    pub async fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let audio_data = tokio::fs::read(audio_path).await?;
        self.transcribe_bytes(&audio_data).await
    }

    /// 从内存中的 WAV 数据直接转录
    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        tracing::info!("开始使用 SenseVoice 转录音频数据: {} bytes", audio_data.len());

        // 构建 multipart/form-data 请求
        let form = reqwest::multipart::Form::new()
            .text("model", "FunAudioLLM/SenseVoiceSmall")
            .part(
                "file",
                reqwest::multipart::Part::bytes(audio_data.to_vec())
                    .file_name("audio.wav")
                    .mime_str("audio/wav")?,
            );

        let url = "https://api.siliconflow.cn/v1/audio/transcriptions";
        tracing::info!("发送请求到 SenseVoice: {}", url);

        // 发送请求
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("SenseVoice API 响应状态: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            tracing::error!("SenseVoice API 错误响应: {}", error_text);
            anyhow::bail!("SenseVoice API 请求失败 ({}): {}", status, error_text);
        }

        let result: serde_json::Value = response.json().await?;
        tracing::info!("SenseVoice API 响应: {}", serde_json::to_string_pretty(&result)?);

        // 解析响应
        let mut text = result["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("无法解析 SenseVoice 转录结果"))?
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

        tracing::info!("SenseVoice 转录完成: {}", text);
        Ok(text)
    }
}

// 主备并行调用：优先使用千问，在重试前检查 SenseVoice 结果（文件版本）
pub async fn transcribe_with_fallback(
    qwen_api_key: String,
    sensevoice_api_key: String,
    audio_path: &Path,
) -> Result<String> {
    let audio_data = tokio::fs::read(audio_path).await?;
    transcribe_with_fallback_bytes(qwen_api_key, sensevoice_api_key, audio_data).await
}

// 主备并行调用：优先使用千问，在重试前检查 SenseVoice 结果（内存版本）
pub async fn transcribe_with_fallback_bytes(
    qwen_api_key: String,
    sensevoice_api_key: String,
    audio_data: Vec<u8>,
) -> Result<String> {
    tracing::info!("启动主备并行转录 (内存模式), 音频大小: {} bytes", audio_data.len());

    // 创建两个客户端
    let qwen_client = QwenASRClient::new(qwen_api_key);
    let sensevoice_client = SenseVoiceClient::new(sensevoice_api_key);

    // 克隆音频数据用于并行任务
    let audio_data_sensevoice = audio_data.clone();

    // 使用共享状态存储 SenseVoice 结果
    let sensevoice_result: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));
    let sensevoice_result_clone = Arc::clone(&sensevoice_result);

    // 启动 SenseVoice 异步任务
    let sensevoice_handle = tokio::spawn(async move {
        tracing::info!("🚀 SenseVoice 任务启动");
        let result = sensevoice_client.transcribe_bytes(&audio_data_sensevoice).await;
        match &result {
            Ok(text) => tracing::info!("✅ SenseVoice 转录成功: {}", text),
            Err(e) => tracing::error!("❌ SenseVoice 转录失败: {}", e),
        }
        *sensevoice_result_clone.lock().unwrap() = Some(result);
    });

    // 千问重试逻辑（最多3次尝试）
    let max_retries = 2;
    let mut qwen_last_error = None;

    for attempt in 0..=max_retries {
        // 如果是重试，先检查 SenseVoice 是否已经完成
        if attempt > 0 {
            tracing::warn!("⏳ 千问第 {} 次重试前，检查 SenseVoice 结果...", attempt);

            // 检查 SenseVoice 是否已有结果
            if let Some(sv_result) = sensevoice_result.lock().unwrap().as_ref() {
                match sv_result {
                    Ok(text) => {
                        tracing::info!("✅ 千问重试前发现 SenseVoice 已成功，立即使用: {}", text);
                        return Ok(text.clone());
                    }
                    Err(e) => {
                        tracing::warn!("⚠️ SenseVoice 也失败了: {}，继续千问重试", e);
                    }
                }
            }

            // 等待一小段时间再重试
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // 尝试千问单次请求
        tracing::info!("🔄 千问第 {} 次尝试 (共 {} 次)", attempt + 1, max_retries + 1);
        match qwen_client.transcribe_from_memory(&audio_data).await {
            Ok(text) => {
                tracing::info!("✅ 千问转录成功: {}", text);
                return Ok(text);
            }
            Err(e) => {
                tracing::error!("❌ 千问第 {} 次尝试失败: {}", attempt + 1, e);
                qwen_last_error = Some(e);
            }
        }
    }

    // 千问全部失败，等待 SenseVoice 最终结果
    tracing::warn!("⚠️ 千问全部失败，等待 SenseVoice 最终结果...");
    let _ = sensevoice_handle.await;

    // 获取 SenseVoice 的最终结果
    if let Some(result) = sensevoice_result.lock().unwrap().take() {
        match result {
            Ok(text) => {
                tracing::info!("✅ 使用 SenseVoice 备用结果: {}", text);
                return Ok(text);
            }
            Err(sensevoice_error) => {
                tracing::error!("❌ 两个 API 都失败了");
                tracing::error!("   千问错误: {:?}", qwen_last_error);
                tracing::error!("   SenseVoice 错误: {:?}", sensevoice_error);
                return Err(anyhow::anyhow!(
                    "两个 API 都失败 - 千问: {:?}, SenseVoice: {}",
                    qwen_last_error,
                    sensevoice_error
                ));
            }
        }
    }

    // 兜底错误
    Err(anyhow::anyhow!("所有 API 都失败"))
}
