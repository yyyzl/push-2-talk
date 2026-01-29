use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use crate::asr::utils;

const DOUBAO_API_URL: &str = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash";
const RESOURCE_ID: &str = "volc.bigasr.auc_turbo";

#[derive(Clone)]
pub struct DoubaoASRClient {
    app_id: String,
    access_key: String,
    client: reqwest::Client,
    dictionary: Vec<String>,
}

impl DoubaoASRClient {
    pub fn new(app_id: String, access_key: String, dictionary: Vec<String>) -> Self {
        Self {
            app_id,
            access_key,
            client: utils::create_http_client(),
            dictionary,
        }
    }

    /// 热更新词库
    pub fn update_dictionary(&mut self, dictionary: Vec<String>) {
        self.dictionary = dictionary;
    }

    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        tracing::info!("豆包 ASR: 音频数据大小 {} bytes", audio_data.len());

        // 构建词库 hotwords JSON
        let corpus = if !self.dictionary.is_empty() {
            let hotwords: Vec<serde_json::Value> = self.dictionary.iter()
                .map(|w| serde_json::json!({"word": w}))
                .collect();
            let context = serde_json::json!({"hotwords": hotwords}).to_string();
            tracing::info!("豆包 HTTP ASR 词库: {} 个词, context={}", self.dictionary.len(), context);
            Some(serde_json::json!({"context": context}))
        } else {
            tracing::info!("豆包 HTTP ASR 词库: 未配置");
            None
        };

        let mut request_obj = serde_json::json!({"model_name": "bigmodel"});
        if let Some(c) = corpus {
            request_obj["corpus"] = c;
        }
        //NOTE: 实验性功能，能提升性能
        request_obj["model_version"] = "400".into();
        request_obj["enable_ddc"] = true.into();
        

        let request_body = serde_json::json!({
            "user": {
                "uid": &self.app_id
            },
            "audio": {
                "data": audio_base64
            },
            "request": request_obj
        });

        let request_id = uuid::Uuid::new_v4().to_string();

        let response = self
            .client
            .post(DOUBAO_API_URL)
            .header("X-Api-App-Key", &self.app_id)
            .header("X-Api-Access-Key", &self.access_key)
            .header("X-Api-Resource-Id", RESOURCE_ID)
            .header("X-Api-Request-Id", &request_id)
            .header("X-Api-Sequence", "-1")
            .json(&request_body)
            .send()
            .await?;

        // 检查响应头中的状态码
        let status_code = response
            .headers()
            .get("X-Api-Status-Code")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let api_message = response
            .headers()
            .get("X-Api-Message")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        tracing::info!("豆包 ASR 响应: status_code={}, message={}", status_code, api_message);

        if status_code != "20000000" {
            anyhow::bail!("豆包 ASR 失败 ({}): {}", status_code, api_message);
        }

        let result: serde_json::Value = response.json().await?;
        tracing::debug!("豆包 ASR 响应体: {}", serde_json::to_string_pretty(&result)?);

        let mut text = result["result"]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("无法解析豆包转录结果"))?
            .to_string();

        utils::strip_trailing_punctuation(&mut text);
        tracing::info!("豆包 ASR 转录完成: {}", text);
        Ok(text)
    }
}
