use crate::asr::utils;
use anyhow::Result;

const SENSEVOICE_API_URL: &str = "https://api.siliconflow.cn/v1/audio/transcriptions";
const MODEL: &str = "FunAudioLLM/SenseVoiceSmall";

#[derive(Clone)]
pub struct SenseVoiceClient {
    api_key: String,
    client: reqwest::Client,
}

impl SenseVoiceClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: utils::create_http_client(),
        }
    }

    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        tracing::info!(
            "开始使用 SenseVoice 转录音频数据: {} bytes",
            audio_data.len()
        );

        let form = reqwest::multipart::Form::new().text("model", MODEL).part(
            "file",
            reqwest::multipart::Part::bytes(audio_data.to_vec())
                .file_name("audio.wav")
                .mime_str("audio/wav")?,
        );

        tracing::info!("发送请求到 SenseVoice: {}", SENSEVOICE_API_URL);

        let response = self
            .client
            .post(SENSEVOICE_API_URL)
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
        tracing::info!(
            "SenseVoice API 响应: {}",
            serde_json::to_string_pretty(&result)?
        );

        let mut text = result["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("无法解析 SenseVoice 转录结果"))?
            .to_string();

        utils::strip_trailing_punctuation(&mut text);
        tracing::info!("SenseVoice 转录完成: {}", text);
        Ok(text)
    }
}
