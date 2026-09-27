use crate::asr::utils;
use crate::config::{AsrLanguageMode, QwenAsrProfile};
use crate::personalization::hotword_compiler::{
    compile_asr_pack_with_correction_pairs, render_qwen_corpus_text, QWEN_HTTP_MAX_HOTWORDS,
};
use crate::personalization::CorrectionPair;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use std::time::Duration;

const QWEN_API_URL: &str =
    "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
const MAX_RETRIES: u32 = 2;

fn asr_language_code(language_mode: AsrLanguageMode) -> &'static str {
    match language_mode {
        AsrLanguageMode::Auto => "auto",
        AsrLanguageMode::Zh => "zh",
    }
}

fn build_request_body(
    profile: QwenAsrProfile,
    language_mode: AsrLanguageMode,
    hotwords: &[String],
    audio_base64: &str,
) -> serde_json::Value {
    match profile {
        QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
            let mut parameters = serde_json::json!({
                "format": "wav",
                "sample_rate": "16000"
            });

            if !hotwords.is_empty() {
                let vocabulary = hotwords
                    .iter()
                    .map(|word| (word.clone(), serde_json::json!(4)))
                    .collect::<serde_json::Map<String, serde_json::Value>>();
                parameters["vocabulary"] = serde_json::Value::Object(vocabulary);
            }

            if language_mode == AsrLanguageMode::Zh {
                parameters["language_hints"] = serde_json::json!(["zh"]);
            }

            serde_json::json!({
                "model": profile.http_model(),
                "input": {
                    "messages": [{
                        "role": "user",
                        "content": [{
                            "type": "input_audio",
                            "input_audio": {
                                "data": format!("data:audio/wav;base64,{}", audio_base64)
                            }
                        }]
                    }]
                },
                "parameters": parameters
            })
        }
        QwenAsrProfile::Qwen3Legacy => {
            let corpus_text = hotwords.join("、");
            serde_json::json!({
                "model": profile.http_model(),
                "input": {
                    "messages": [
                        {
                            "role": "system",
                            "content": [{"text": corpus_text}]
                        },
                        {
                            "role": "user",
                            "content": [{
                                "audio": format!("data:audio/wav;base64,{}", audio_base64)
                            }]
                        }
                    ]
                },
                "parameters": {
                    "language": asr_language_code(language_mode),
                    "asr_options": {
                        "enable_itn": true
                    }
                }
            })
        }
    }
}

fn parse_transcription_text(profile: QwenAsrProfile, result: &serde_json::Value) -> Result<String> {
    let text = match profile {
        QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => result["output"]["text"]
            .as_str()
            .or_else(|| result["output"]["output"]["sentence"]["text"].as_str()),
        QwenAsrProfile::Qwen3Legacy => result["output"]["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|choice| choice["message"]["content"].as_array())
            .and_then(|content| content.first())
            .and_then(|item| item["text"].as_str()),
    };

    text.map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("无法解析转录结果，响应格式: {:?}", result))
}

#[cfg(test)]
fn build_qwen_http_corpus_text(dictionary: &[String]) -> (usize, String) {
    build_qwen_http_corpus_text_with_pairs(dictionary, &[])
}

#[cfg(test)]
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
    profile: QwenAsrProfile,
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
        Self::new_with_profile_and_correction_pairs(
            api_key,
            dictionary,
            correction_pairs,
            language_mode,
            QwenAsrProfile::default(),
        )
    }

    pub fn new_with_profile_and_correction_pairs(
        api_key: String,
        dictionary: Vec<String>,
        correction_pairs: Vec<CorrectionPair>,
        language_mode: AsrLanguageMode,
        profile: QwenAsrProfile,
    ) -> Self {
        Self {
            api_key,
            client: utils::create_http_client(),
            max_retries: MAX_RETRIES,
            dictionary,
            correction_pairs,
            language_mode,
            profile,
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

    /// 单次识别，不进行自动重试；用于竞速策略和显式 API 验证。
    pub async fn transcribe_from_memory(&self, audio_data: &[u8]) -> Result<String> {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        tracing::info!("音频数据大小: {} bytes", audio_data.len());

        let hotword_pack = compile_asr_pack_with_correction_pairs(
            &self.dictionary,
            &self.correction_pairs,
            QWEN_HTTP_MAX_HOTWORDS,
        );
        let hotwords = hotword_pack
            .words
            .iter()
            .map(|hotword| hotword.text.clone())
            .collect::<Vec<_>>();
        let hotword_count = hotwords.len();
        let corpus_text = render_qwen_corpus_text(&hotword_pack);
        if !corpus_text.is_empty() {
            tracing::info!("Qwen HTTP ASR 词库: {} 个词（已编译）", hotword_count);
        } else {
            tracing::info!("Qwen HTTP ASR 词库: 未配置");
        }

        let request_body =
            build_request_body(self.profile, self.language_mode, &hotwords, &audio_base64);

        tracing::info!("发送请求到: {}", QWEN_API_URL);

        let response = self
            .client
            .post(QWEN_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("X-DashScope-SSE", "disable")
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

        let mut text = parse_transcription_text(self.profile, &result)?;

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
        parse_transcription_text,
    };
    use crate::config::{AsrLanguageMode, QwenAsrProfile};
    use crate::personalization::hotword_compiler::QWEN_HTTP_MAX_HOTWORDS;
    use crate::personalization::CorrectionPair;

    #[test]
    fn qwen_audio_3_1_http_contract_and_response() {
        let profile: QwenAsrProfile = serde_json::from_str("\"qwen_audio_3_1\"").unwrap();
        let request = build_request_body(profile, AsrLanguageMode::Zh, &["Rust".into()], "abc");
        assert_eq!(request["model"], "qwen-audio-3.1-asr-flash");
        assert_eq!(
            request["input"]["messages"][0]["content"][0]["input_audio"]["data"],
            "data:audio/wav;base64,abc"
        );
        assert_eq!(request["parameters"]["vocabulary"]["Rust"], 4);
        assert_eq!(
            request["parameters"]["language_hints"],
            serde_json::json!(["zh"])
        );
        for response in [
            serde_json::json!({"output": {"text": "测试结果"}}),
            serde_json::json!({"output": {"output": {"sentence": {"text": "测试结果"}}}}),
        ] {
            assert_eq!(
                parse_transcription_text(profile, &response).unwrap(),
                "测试结果"
            );
        }
        assert!(parse_transcription_text(profile, &serde_json::json!({"output": {}})).is_err());
    }

    #[test]
    fn build_request_body_sets_auto_language() {
        let request = build_request_body(
            QwenAsrProfile::Qwen3Legacy,
            AsrLanguageMode::Auto,
            &[],
            "abc",
        );
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
        let request =
            build_request_body(QwenAsrProfile::Qwen3Legacy, AsrLanguageMode::Zh, &[], "abc");
        assert_eq!(request["parameters"]["language"], "zh");
    }

    #[test]
    fn builds_qwen_audio_3_http_request_by_default_contract() {
        let request = build_request_body(
            QwenAsrProfile::QwenAudio3,
            AsrLanguageMode::Auto,
            &["Windsurf".to_string(), "Rust".to_string()],
            "abc",
        );

        assert_eq!(request["model"], "qwen-audio-3.0-asr-flash");
        assert_eq!(request["input"]["messages"][0]["role"], "user");
        assert_eq!(
            request["input"]["messages"][0]["content"][0]["type"],
            "input_audio"
        );
        assert_eq!(
            request["input"]["messages"][0]["content"][0]["input_audio"]["data"],
            "data:audio/wav;base64,abc"
        );
        assert_eq!(request["parameters"]["format"], "wav");
        assert_eq!(request["parameters"]["sample_rate"], "16000");
        assert_eq!(request["parameters"]["vocabulary"]["Windsurf"], 4);
        assert_eq!(request["parameters"]["vocabulary"]["Rust"], 4);
        assert!(request["parameters"].get("language_hints").is_none());
    }

    #[test]
    fn builds_qwen_audio_3_zh_language_hint() {
        let request =
            build_request_body(QwenAsrProfile::QwenAudio3, AsrLanguageMode::Zh, &[], "abc");

        assert_eq!(
            request["parameters"]["language_hints"],
            serde_json::json!(["zh"])
        );
    }

    #[test]
    fn keeps_qwen3_legacy_http_request_shape() {
        let request = build_request_body(
            QwenAsrProfile::Qwen3Legacy,
            AsrLanguageMode::Auto,
            &["Windsurf".to_string(), "Rust".to_string()],
            "abc",
        );

        assert_eq!(request["model"], "qwen3-asr-flash");
        assert_eq!(
            request["input"]["messages"][0]["content"][0]["text"],
            "Windsurf、Rust"
        );
        assert_eq!(request["parameters"]["language"], "auto");
    }

    #[test]
    fn parses_qwen_audio_3_http_response() {
        let response = serde_json::json!({
            "output": { "text": "最新版识别结果。" }
        });

        assert_eq!(
            parse_transcription_text(QwenAsrProfile::QwenAudio3, &response).unwrap(),
            "最新版识别结果。"
        );
    }

    #[test]
    fn parses_qwen3_legacy_http_response() {
        let response = serde_json::json!({
            "output": {
                "choices": [{
                    "message": { "content": [{ "text": "旧版识别结果。" }] }
                }]
            }
        });

        assert_eq!(
            parse_transcription_text(QwenAsrProfile::Qwen3Legacy, &response).unwrap(),
            "旧版识别结果。"
        );
    }
}
