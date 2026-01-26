// LLM 词汇判断器
//
// 功能：调用 LLM 判断候选词是否值得学习
// 判断标准：专有名词、专业术语、高频词汇

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};

use crate::openai_client::{ChatOptions, Message, OpenAiClient, OpenAiClientConfig};

/// LLM 判断结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmJudgeResult {
    pub should_learn: bool,
    pub word: String,
    pub category: String,
    pub reason: String,
}

/// LLM 判断器
pub struct LlmJudge {
    client: OpenAiClient,
}

impl LlmJudge {
    /// 创建新的 LLM 判断器
    pub fn new(endpoint: &str, api_key: &str, model: &str) -> Self {
        let config = OpenAiClientConfig::new(endpoint, api_key, model);
        Self {
            client: OpenAiClient::new(config),
        }
    }

    /// 判断候选词是否值得学习
    ///
    /// # Arguments
    /// * `original` - 原文片段
    /// * `corrected` - 修正后片段
    /// * `context` - 上下文（前后各 10 字符）
    ///
    /// # Returns
    /// * `Ok(LlmJudgeResult)` - 判断成功
    /// * `Err` - 判断失败（超时或解析错误）
    pub async fn judge(
        &self,
        original: &str,
        corrected: &str,
        context: &str,
    ) -> Result<LlmJudgeResult> {
        let system_prompt = r#"你是一个词汇分类助手。判断用户修正的词汇是否应该添加到个人词典。

判断标准：
1. 专有名词（人名、地名、品牌、机构名）→ category: "proper_noun"
2. 专业术语（技术、医学、法律等领域）→ category: "term"
3. 高频使用的特定词汇 → category: "frequent"

返回 JSON 格式（严格遵循）：
{"should_learn": true/false, "word": "建议添加的词汇", "category": "proper_noun/term/frequent", "reason": "简短理由"}"#;

        let user_prompt = format!(
            "原文：\"{}\"\n修正：\"{}\"\n上下文：\"{}\"",
            original, corrected, context
        );

        let messages = vec![
            Message::system(system_prompt),
            Message::user(user_prompt),
        ];

        let options = ChatOptions {
            max_tokens: 256,
            temperature: 0.1,
        };

        // 3 秒超时
        let response = timeout(Duration::from_secs(3), self.client.chat(&messages, options))
            .await
            .map_err(|_| anyhow!("LLM 判断超时"))??;

        parse_llm_response(&response)
    }
}

fn parse_llm_response(text: &str) -> Result<LlmJudgeResult> {
    // 尝试直接解析
    if let Ok(parsed) = serde_json::from_str::<LlmJudgeResult>(text) {
        return sanitize_result(parsed);
    }

    // 尝试提取 JSON 部分
    let start = text.find('{').ok_or_else(|| anyhow!("LLM 响应缺少 JSON"))?;
    let end = text.rfind('}').ok_or_else(|| anyhow!("LLM 响应缺少 JSON"))?;
    let json = &text[start..=end];

    let parsed = serde_json::from_str::<LlmJudgeResult>(json)
        .map_err(|e| anyhow!("LLM 响应解析失败: {}", e))?;

    sanitize_result(parsed)
}

/// 验证并清理 LLM 响应
///
/// 检查：
/// 1. 如果 should_learn=true，词汇不能为空
/// 2. 词汇长度不超过 64 字符
/// 3. 分类为有效值
fn sanitize_result(mut result: LlmJudgeResult) -> Result<LlmJudgeResult> {
    // 最大词汇长度
    const MAX_WORD_LEN: usize = 64;

    // 有效分类列表
    const VALID_CATEGORIES: [&str; 3] = ["proper_noun", "term", "frequent"];

    // 1. 检查词汇（仅在 should_learn=true 时强制非空）
    let word = result.word.trim();
    if result.should_learn && word.is_empty() {
        // should_learn=true 但 word 为空，这是无效的
        return Err(anyhow!("LLM 返回 should_learn=true 但词汇为空"));
    }

    // 2. 检查词汇长度
    if word.chars().count() > MAX_WORD_LEN {
        tracing::warn!(
            "Learning: LLM 返回词汇过长 ({} 字符), 截断",
            word.chars().count()
        );
        result.word = word.chars().take(MAX_WORD_LEN).collect();
    } else {
        result.word = word.to_string();
    }

    // 3. 验证分类（仅在 should_learn=true 时检查）
    if result.should_learn && !VALID_CATEGORIES.contains(&result.category.as_str()) {
        tracing::debug!(
            "Learning: LLM 返回无效分类 '{}', 默认为 'term'",
            result.category
        );
        result.category = "term".to_string();
    }

    // 4. 限制 reason 长度（防止过长）
    const MAX_REASON_LEN: usize = 200;
    if result.reason.chars().count() > MAX_REASON_LEN {
        result.reason = result.reason.chars().take(MAX_REASON_LEN).collect::<String>() + "...";
    }

    Ok(result)
}
