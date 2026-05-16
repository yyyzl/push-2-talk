use crate::asr::utils;
use crate::config::AsrLanguageMode;
use crate::personalization::hotword_compiler::{
    compile_asr_pack_with_correction_pairs, render_qwen_corpus_text, QWEN_HTTP_MAX_HOTWORDS,
};
use crate::personalization::CorrectionPair;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use std::time::Duration;

const QWEN_API_URL: &str =
    "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
const MODEL: &str = "qwen3-asr-flash";
const MAX_RETRIES: u32 = 2;

fn asr_language_code(language_mode: AsrLanguageMode) -> &'static str {
    match language_mode {
        AsrLanguageMode::Auto => "auto",
        AsrLanguageMode::Zh => "zh",
    }
}

fn build_request_body(
    language_mode: AsrLanguageMode,
    corpus_text: &str,
    audio_base64: &str,
) -> serde_json::Value {
    serde_json::json!({
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
            // NOTE: 疑似无效参数，暂时注释掉
            // "result_format": "message",
            // "enable_itn": true,
            // "disfluency_removal": true,
            "language": asr_language_code(language_mode),
            "asr_options": {
                "enable_itn": true
            }
        }
    })
}

#[cfg(test)]
fn build_qwen_http_corpus_text(dictionary: &[String]) -> (usize, String) {
    build_qwen_http_corpus_text_with_pairs(dictionary, &[])
}

fn build_qwen_http_corpus_text_with_pairs(
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> (usize, String) {
    let hotword_pack = compile_asr_pack_with_correction_pairs(
        dictionary,
        correction_pairs,
        QWEN_HTTP_MAX_HOTWORDS,
    );
    (
        hotword_pack.words.len(),
        render_qwen_corpus_text(&hotword_pack),
    )
}

#[derive(Clone)]
pub struct QwenASRClient {
    api_key: String,
    client: reqwest::Client,
    max_retries: u32,
    dictionary: Vec<String>,
    correction_pairs: Vec<CorrectionPair>,
    language_mode: AsrLanguageMode,
}

impl QwenASRClient {
    pub fn new(api_key: String, dictionary: Vec<String>, language_mode: AsrLanguageMode) -> Self {
        Self::new_with_correction_pairs(api_key, dictionary, Vec::new(), language_mode)
    }

    pub fn new_with_correction_pairs(
        api_key: String,
        dictionary: Vec<String>,
        correction_pairs: Vec<CorrectionPair>,
        language_mode: AsrLanguageMode,
    ) -> Self {
        Self {
            api_key,
            client: utils::create_http_client(),
            max_retries: MAX_RETRIES,
            dictionary,
            correction_pairs,
            language_mode,
        }
    }

    /// 热更新词库
    pub fn update_dictionary(&mut self, dictionary: Vec<String>) {
        self.dictionary = dictionary;
    }

    pub fn update_correction_pairs(&mut self, correction_pairs: Vec<CorrectionPair>) {
        self.correction_pairs = correction_pairs;
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
                    tracing::error!(
                        "转录失败 (尝试 {}/{}): {}",
                        attempt + 1,
                        self.max_retries + 1,
                        e
                    );
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

        // 词库编译（提纯、去重、排序、截断）后用顿号分隔
        let (hotword_count, corpus_text) =
            build_qwen_http_corpus_text_with_pairs(&self.dictionary, &self.correction_pairs);
        if !corpus_text.is_empty() {
            tracing::info!(
                "Qwen HTTP ASR 词库: {} 个词（已编译）, corpus={}",
                hotword_count,
                corpus_text
            );
        } else {
            tracing::info!("Qwen HTTP ASR 词库: 未配置");
        }

        let request_body = build_request_body(self.language_mode, &corpus_text, &audio_base64);

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

#[cfg(test)]
mod tests {
    use super::{
        build_qwen_http_corpus_text, build_qwen_http_corpus_text_with_pairs, build_request_body,
    };
    use crate::config::AsrLanguageMode;
    use crate::personalization::hotword_compiler::QWEN_HTTP_MAX_HOTWORDS;
    use crate::personalization::CorrectionPair;

    #[test]
    fn build_request_body_sets_auto_language() {
        let request = build_request_body(AsrLanguageMode::Auto, "", "abc");
        assert_eq!(request["parameters"]["language"], "auto");
    }

    #[test]
    fn limits_qwen_http_corpus_with_hotword_compiler() {
        let dictionary = (0..(QWEN_HTTP_MAX_HOTWORDS + 5))
            .map(|idx| format!("词{}|manual|product", idx))
            .collect::<Vec<_>>();

        let (_, corpus) = build_qwen_http_corpus_text(&dictionary);

        assert_eq!(corpus.split('、').count(), QWEN_HTTP_MAX_HOTWORDS);
        assert!(!corpus.contains('|'));
    }

    #[test]
    fn qwen_http_corpus_includes_runtime_correction_pairs() {
        let dictionary = vec!["Rust|auto|tool".to_string()];
        let pairs = vec![CorrectionPair::new("windsurf", "winds surf", "Windsurf")];

        let (_, corpus) = build_qwen_http_corpus_text_with_pairs(&dictionary, &pairs);

        assert_eq!(corpus, "Windsurf、Rust");
        assert!(!corpus.contains("winds surf"));
        assert!(!corpus.contains('|'));
    }

    #[test]
    fn build_request_body_sets_zh_language() {
        let request = build_request_body(AsrLanguageMode::Zh, "", "abc");
        assert_eq!(request["parameters"]["language"], "zh");
    }
}
