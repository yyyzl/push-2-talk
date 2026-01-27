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
    /// * `context` - 上下文（前后各 10 个词）
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
        let system_prompt = r#"你是一个 ASR 词库优化助手。用户修正了语音识别结果，请判断是否应该将词汇添加到词库。

任务：
1. 首先判断修正后的词汇能否和上下文中的相邻词联动成一个更有意义的短语/术语
2. 如果能联动成短语，返回完整短语
3. 如果不能联动，返回修正后的单词本身
4. 判断这个词/短语是否值得加入词库

只有以下类型的词汇值得学习：
1. 专有名词（人名、地名、品牌、机构名）→ category: "proper_noun"
2. 专业术语（技术、医学、法律等领域）→ category: "term"
3. 高频使用的特定词汇 → category: "frequent"

不值得学习的词汇：
- 常见人名（如张伟、李明）
- 常见地名（如北京、上海）
- 普通名词/动词/形容词

---

## 参考案例（Few-Shot Examples）

### 案例 1：应该联动成短语
输入：
- 原文："cloud"
- 修正："claude"
- 上下文："我最近在学习 claude code 这个工具"

分析：
- "claude" 和后面的 "code" 可以联动成 "claude code"
- "claude code" 是一个技术工具名称，属于专业术语

输出：
{"should_learn": true, "word": "claude code", "category": "term", "reason": "AI 编程工具名称"}

---

### 案例 2：应该联动成短语
输入：
- 原文："人工只能"
- 修正："人工智能"
- 上下文："人工智能大模型的发展趋势"

分析：
- "人工智能" 和后面的 "大模型" 可以联动成 "人工智能大模型"
- "人工智能大模型" 是一个专业术语

输出：
{"should_learn": true, "word": "人工智能大模型", "category": "term", "reason": "AI 领域专业术语"}

---

### 案例 3：不应该联动（只学习单词）
输入：
- 原文："丽娜"
- 修正："李娜"
- 上下文："张伟和李娜一起去开会"

分析：
- "李娜" 和周围的 "张伟和" 不能联动成有意义的短语
- "张伟和李娜" 只是人名列举，不是固定搭配
- "李娜" 是人名，值得学习

输出：
{"should_learn": true, "word": "李娜", "category": "proper_noun", "reason": "人名"}

---

### 案例 4：不应该学习（常见词）
输入：
- 原文："张为"
- 修正："张伟"
- 上下文："今天天气很好，我要去见张伟"

分析：
- "张伟" 是常见人名，ASR 通常能正确识别
- 不值得加入词库

输出：
{"should_learn": false, "word": "张伟", "category": "proper_noun", "reason": "常见人名，ASR 通常能正确识别"}

---

返回 JSON 格式（严格遵循）：
{"should_learn": true/false, "word": "建议添加的词汇或短语", "category": "proper_noun/term/frequent", "reason": "简短理由"}"#;

        let user_prompt = format!(
            "原文：\"{}\"\n修正：\"{}\"\n上下文：\"{}\"\n\n请判断：\n1. 修正后的词能否和上下文联动成更有意义的短语？\n2. 这个词/短语是否值得加入词库？",
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

        // 5 秒超时（适应更长的 Few-Shot prompt）
        let response = timeout(Duration::from_secs(5), self.client.chat(&messages, options))
            .await
            .map_err(|_| anyhow!("LLM 判断超时（5s）"))??;

        parse_llm_response(&response)
    }
}

fn parse_llm_response(text: &str) -> Result<LlmJudgeResult> {
    // 尝试直接解析
    if let Ok(parsed) = serde_json::from_str::<LlmJudgeResult>(text) {
        return sanitize_result(parsed);
    }

    // 尝试提取 JSON 部分 - 使用更鲁棒的策略
    // 从第一个 '{' 开始，尝试解析每个可能的 JSON 对象
    let start = text.find('{').ok_or_else(|| anyhow!("LLM 响应缺少 JSON"))?;

    // 尝试从 start 位置开始，逐步扩展到每个 '}' 位置
    let mut last_error = None;
    for (idx, _) in text[start..].match_indices('}') {
        let end_pos = start + idx;
        let json = &text[start..=end_pos];

        match serde_json::from_str::<LlmJudgeResult>(json) {
            Ok(parsed) => return sanitize_result(parsed),
            Err(e) => last_error = Some(e),
        }
    }

    // 如果所有尝试都失败，返回最后一个错误
    Err(anyhow!(
        "LLM 响应解析失败: {}",
        last_error.map(|e| e.to_string()).unwrap_or_else(|| "未找到有效 JSON".to_string())
    ))
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
