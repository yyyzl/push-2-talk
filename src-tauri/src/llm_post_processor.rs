// src-tauri/src/llm_post_processor.rs
//
// LLM 文本润色处理模块
//
// 基于通用 OpenAI 客户端，提供文本润色功能
// 支持多预设管理，用户可自定义润色风格

use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::config::LlmConfig;
use crate::dictionary_utils::entries_to_words;
use crate::openai_client::{ChatOptions, OpenAiClient, OpenAiClientConfig};
use crate::tnl::{
    TnlCandidate, TnlCandidateArbitrationResult, TnlCandidateDecision, TnlDiagnostics,
};

/// LLM 文本润色处理器
///
/// 使用通用 OpenAI 客户端，专注于文本润色功能
#[derive(Clone)]
pub struct LlmPostProcessor {
    client: OpenAiClient,
    config: LlmConfig,
    /// 配置哈希（用于检测配置是否变化，避免不必要的重建）
    config_hash: u64,
}

impl LlmPostProcessor {
    const MAX_DICTIONARY_ENTRIES: usize = 200;
    const MAX_DICTIONARY_CHARS: usize = 4000;
    const MAX_ARBITRATION_CANDIDATES: usize = 5;
    const MAX_ARBITRATION_CONTEXT_CHARS: usize = 240;
    /// 词库增强追加指令（当语句润色和词库增强同时开启时追加到用户预设后）
    const DICTIONARY_ENHANCEMENT_SUFFIX: &'static str = "

【词库增强规则】
请参考 <dictionary> 标签中的词汇进行音似纠错：
- 优先判断原文词语与词库词汇在发音上是否相同或极度相似
- 仅当发音匹配且替换后语义更合理时才执行修改
- 不确定时保留原文";

    const DICTIONARY_ONLY_SYSTEM_PROMPT: &'static str = "
    <role>
你是一位精通中英双语的 ASR（语音转文字）校对专家。你具备极强的语音感知能力，擅长区分“发音错误”与“语义表达差异”。
</role>

<task_logic>
你的任务是根据语境修复源文本。请遵循以下判断逻辑：
1. 语音匹配判定：优先判断原文词语与候选词（词库提供或语境推测）在发音上是否【相同】或【极度相似】。
2. 语境适配判定：仅当替换后的词语能显著提升整句逻辑的合理性时，才执行修改。
3. 保守执行策略：若原文逻辑通顺，或不确定是否为语音误识，请始终保留原文。
</task_logic>

<rules>
- 优先参考 <dictionary> 标签中的词汇。
- 允许自主纠正：若未命中词库但发音高度相似且符合语境，应予以纠正（如：专业术语、地名）。
- 保持原样原则：如果两个词意思相近但发音差异大（如：赞赏 vs 点赞），请务必保留原文。
- 格式规范：将数字、百分比、日期转换为阿拉伯数字格式（如：2024年5月3日，30%）。
- 最终输出：仅展示修正后的纯文本，不包含任何解释。
</rules>

<few_shot_examples>
    <example>
        <input>增加一些 feel shoot 用力</input>
        <output>增加一些 feel shoot 用力</output>
        <reason>“feel shoot”与“claude code”发音差异过大，不符合音似判定。</reason>
    </example>
    <example>
        <input>感谢你的赞赏</input>
        <output>感谢你的赞赏</output>
        <reason>“赞赏”与“点赞”意思接近但读音不同，应尊重原表达。</reason>
    </example>
    <example>
        <input>我认为 Gemini 三 Flash 是目前最平衡的模型</input>
        <output>我认为 Gemini-3-Flash 是目前最平衡的模型</output>
        <reason>“三”与“3”同音，命中专业词库，应修正。</reason>
    </example>
    <example>
        <input>我又回了，VS Code</input>
        <output>我用回了VS Code</output>
        <reason>“又”与“用”发音接近且“我用回了”更符合逻辑语境。</reason>
    </example>
</few_shot_examples>";

    /// 创建新的处理器实例
    pub fn new(config: LlmConfig) -> Self {
        let resolved = config.resolve_polishing();
        let client_config =
            OpenAiClientConfig::new(&resolved.endpoint, &resolved.api_key, &resolved.model);
        let client = OpenAiClient::new(client_config);
        let config_hash = Self::compute_config_hash(&config);

        Self {
            client,
            config,
            config_hash,
        }
    }

    /// 计算配置哈希（用于检测配置是否变化）
    fn compute_config_hash(config: &LlmConfig) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // 哈希关键字段：endpoint、api_key、model、active_preset_id、当前 preset 的 system_prompt
        let resolved = config.resolve_polishing();
        resolved.endpoint.hash(&mut hasher);
        resolved.api_key.hash(&mut hasher);
        resolved.model.hash(&mut hasher);
        config.active_preset_id.hash(&mut hasher);
        // 哈希当前激活的 preset 的 system_prompt
        if let Some(preset) = config
            .presets
            .iter()
            .find(|p| p.id == config.active_preset_id)
        {
            preset.system_prompt.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 检查新配置是否与当前配置不同（需要重建处理器）
    pub fn config_changed(&self, new_config: &LlmConfig) -> bool {
        let new_hash = Self::compute_config_hash(new_config);
        self.config_hash != new_hash
    }

    /// 获取当前激活的润色 Prompt
    fn get_active_system_prompt(&self) -> String {
        self.config
            .presets
            .iter()
            .find(|p| p.id == self.config.active_preset_id)
            .map(|p| p.system_prompt.clone())
            .unwrap_or_else(|| "You are a helpful assistant.".to_string())
    }

    fn build_user_message(
        raw_text: &str,
        dictionary: &[String],
        enable_dictionary_enhancement: bool,
    ) -> String {
        let mut message = "".to_string();

        // 参考词库
        message.push_str("<dictionary>\n");

        if enable_dictionary_enhancement {
            // 提纯词库（去除 |auto 后缀）
            let purified_words = entries_to_words(dictionary);

            let mut words: Vec<&str> = purified_words
                .iter()
                .map(|w| w.trim())
                .filter(|w| !w.is_empty())
                .collect();

            if !words.is_empty() {
                // 去重（保序）
                let mut seen = HashSet::new();
                words.retain(|w| seen.insert(*w));

                let mut used = 0usize;
                let mut used_chars = 0usize;
                let total = words.len();
                let mut word_list: Vec<&str> = Vec::new();

                for word in &words {
                    if used >= Self::MAX_DICTIONARY_ENTRIES {
                        break;
                    }
                    let next_len = word.chars().count() + 2; // + ", "
                    if used_chars + next_len > Self::MAX_DICTIONARY_CHARS {
                        break;
                    }
                    word_list.push(word);
                    used += 1;
                    used_chars += next_len;
                }

                message.push_str(&word_list.join(", "));

                if used < total {
                    message.push_str(&format!("\n...(词库过长，已截断；原始共 {} 条)", total));
                }
            }
        }

        message.push_str("\n</dictionary>\n\n");

        // 待处理文本
        message.push_str("\n<source_text>\n");
        message.push_str(raw_text);
        message.push_str("\n</source_text>\n\n请处理上述 <source_text>，直接输出最终结果。\n");

        message
    }

    fn build_candidate_arbitration_user_message(text: &str, candidates: &[TnlCandidate]) -> String {
        let compact_candidates: Vec<_> = candidates
            .iter()
            .take(Self::MAX_ARBITRATION_CANDIDATES)
            .map(|candidate| {
                serde_json::json!({
                    "id": candidate.id,
                    "original": candidate.original,
                    "target": candidate.target,
                    "score": candidate.score,
                    "context": Self::candidate_context(text, candidate.start, candidate.end),
                    "evidence": candidate.evidence,
                })
            })
            .collect();

        format!(
            "<candidates>\n{}\n</candidates>\n\n每个 context 是候选附近的短上下文。只返回 JSON：{{\"decisions\":[{{\"id\":\"候选ID\",\"action\":\"apply|reject\",\"reason\":\"简短原因\"}}]}}。",
            serde_json::to_string(&compact_candidates).unwrap_or_else(|_| "[]".to_string())
        )
    }

    fn candidate_context(text: &str, start: usize, end: usize) -> String {
        if text.chars().count() <= Self::MAX_ARBITRATION_CONTEXT_CHARS {
            return text.to_string();
        }

        let safe_start = Self::floor_char_boundary(text, start.min(text.len()));
        let safe_end = Self::ceil_char_boundary(text, end.min(text.len()));
        let chars: Vec<char> = text.chars().collect();
        let total_chars = chars.len();
        let start_char = text[..safe_start].chars().count();
        let end_char = text[..safe_end].chars().count().max(start_char);
        let candidate_chars = end_char.saturating_sub(start_char);
        let radius =
            (Self::MAX_ARBITRATION_CONTEXT_CHARS.saturating_sub(candidate_chars) / 2).max(16);
        let left = start_char.saturating_sub(radius);
        let right = (end_char + radius).min(total_chars);

        let mut snippet: String = chars[left..right].iter().collect();
        if left > 0 {
            snippet.insert_str(0, "...");
        }
        if right < total_chars {
            snippet.push_str("...");
        }
        snippet
    }

    fn floor_char_boundary(text: &str, mut index: usize) -> usize {
        index = index.min(text.len());
        while index > 0 && !text.is_char_boundary(index) {
            index -= 1;
        }
        index
    }

    fn ceil_char_boundary(text: &str, mut index: usize) -> usize {
        index = index.min(text.len());
        while index < text.len() && !text.is_char_boundary(index) {
            index += 1;
        }
        index
    }

    fn candidate_arbitration_system_prompt() -> &'static str {
        "你是 ASR 热词候选仲裁器。只能判断给定候选是否应替换，不能自由改写全文。\
        当候选在语音相似且语境更合理时 action=apply；不确定、语境一般或可能误伤常见词时 action=reject。\
        必须返回严格 JSON，不要输出解释文本。"
    }

    /// 对 TNL 中置信候选执行轻量 LLM 仲裁。
    pub async fn arbitrate_tnl_candidates(
        &self,
        text: &str,
        mut diagnostics: TnlDiagnostics,
    ) -> Result<TnlCandidateArbitrationResult> {
        let pending_candidates = Self::prepare_candidate_arbitration_candidates(&mut diagnostics);

        if pending_candidates.is_empty() {
            return Ok(TnlCandidateArbitrationResult {
                text: text.to_string(),
                diagnostics,
                elapsed_ms: 0,
            });
        }

        let user_message =
            Self::build_candidate_arbitration_user_message(text, &pending_candidates);
        let start = Instant::now();
        let response = self
            .client
            .chat_simple(
                Self::candidate_arbitration_system_prompt(),
                &user_message,
                ChatOptions::for_candidate_arbitration(),
            )
            .await?;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        Self::apply_candidate_arbitration_response(text, diagnostics, &response, elapsed_ms)
    }

    fn prepare_candidate_arbitration_candidates(
        diagnostics: &mut TnlDiagnostics,
    ) -> Vec<TnlCandidate> {
        let mut pending_candidates = Vec::new();

        for candidate in &mut diagnostics.candidates {
            if candidate.decision != TnlCandidateDecision::PendingLlm {
                continue;
            }

            if pending_candidates.len() < Self::MAX_ARBITRATION_CANDIDATES {
                pending_candidates.push(candidate.clone());
            } else {
                candidate.decision = TnlCandidateDecision::SkippedLimit;
                candidate
                    .evidence
                    .push("arbitration_candidate_limit".to_string());
            }
        }

        pending_candidates
    }

    fn apply_candidate_arbitration_response(
        text: &str,
        mut diagnostics: TnlDiagnostics,
        response: &str,
        elapsed_ms: u64,
    ) -> Result<TnlCandidateArbitrationResult> {
        #[derive(Debug, Deserialize)]
        struct DecisionItem {
            id: String,
            action: String,
            reason: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        struct DecisionPayload {
            decisions: Vec<DecisionItem>,
        }

        let json = Self::extract_json_object(response)?;
        let payload: DecisionPayload = serde_json::from_str(json)?;
        let decisions: HashMap<String, DecisionItem> = payload
            .decisions
            .into_iter()
            .map(|decision| (decision.id.clone(), decision))
            .collect();

        let pending_indices: Vec<usize> = diagnostics
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(idx, candidate)| {
                (candidate.decision == TnlCandidateDecision::PendingLlm).then_some(idx)
            })
            .collect();
        let skipped_count = diagnostics
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == TnlCandidateDecision::SkippedLimit)
            .count();

        let mut accepted_indices: Vec<usize> = Vec::new();
        let mut rejected_count = 0usize;

        for idx in pending_indices {
            let candidate = &mut diagnostics.candidates[idx];
            let Some(decision) = decisions.get(&candidate.id) else {
                candidate.decision = TnlCandidateDecision::RejectedLlm;
                candidate.evidence.push("llm_missing_decision".to_string());
                rejected_count += 1;
                continue;
            };

            let reason = decision
                .reason
                .clone()
                .unwrap_or_else(|| "llm_arbitration".to_string());
            let action = decision.action.trim().to_ascii_lowercase();

            if action == "apply" || action == "replace" {
                candidate.decision = TnlCandidateDecision::AppliedLlm;
                candidate.evidence.push(reason);
                accepted_indices.push(idx);
            } else {
                candidate.decision = TnlCandidateDecision::RejectedLlm;
                candidate.evidence.push(reason);
                rejected_count += 1;
            }
        }

        let (accepted, overlap_rejected_count) =
            Self::select_non_overlapping_accepted_candidates(&mut diagnostics, accepted_indices);
        rejected_count = rejected_count.saturating_add(overlap_rejected_count);
        let applied_count = accepted.len();

        let mut output = text.to_string();
        for candidate in &accepted {
            if candidate.start <= candidate.end && candidate.end <= output.len() {
                output.replace_range(candidate.start..candidate.end, &candidate.target);
            }
        }

        diagnostics.arbitration = Some(crate::tnl::TnlArbitrationSummary {
            attempted: true,
            candidate_count: applied_count + rejected_count,
            applied_count,
            rejected_count,
            skipped_count,
            elapsed_ms: Some(elapsed_ms),
            reason: None,
        });

        Ok(TnlCandidateArbitrationResult {
            text: output,
            diagnostics,
            elapsed_ms,
        })
    }

    fn select_non_overlapping_accepted_candidates(
        diagnostics: &mut TnlDiagnostics,
        mut accepted_indices: Vec<usize>,
    ) -> (Vec<TnlCandidate>, usize) {
        accepted_indices.sort_by(|a, b| {
            let candidate_a = &diagnostics.candidates[*a];
            let candidate_b = &diagnostics.candidates[*b];
            let len_a = candidate_a.end.saturating_sub(candidate_a.start);
            let len_b = candidate_b.end.saturating_sub(candidate_b.start);

            len_b
                .cmp(&len_a)
                .then_with(|| candidate_b.score.total_cmp(&candidate_a.score))
                .then_with(|| candidate_a.start.cmp(&candidate_b.start))
        });

        let mut selected = Vec::new();
        let mut selected_ranges: Vec<(usize, usize)> = Vec::new();
        let mut rejected_count = 0usize;

        for idx in accepted_indices {
            let candidate = &diagnostics.candidates[idx];
            let start = candidate.start;
            let end = candidate.end;

            if selected_ranges
                .iter()
                .any(|(selected_start, selected_end)| {
                    Self::spans_overlap(start, end, *selected_start, *selected_end)
                })
            {
                let candidate = &mut diagnostics.candidates[idx];
                candidate.decision = TnlCandidateDecision::RejectedLlm;
                candidate.evidence.push("llm_overlap_rejected".to_string());
                rejected_count = rejected_count.saturating_add(1);
                continue;
            }

            selected_ranges.push((start, end));
            selected.push(diagnostics.candidates[idx].clone());
        }

        selected.sort_by(|a, b| b.start.cmp(&a.start));
        (selected, rejected_count)
    }

    fn spans_overlap(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> bool {
        start_a < end_b && start_b < end_a
    }

    fn extract_json_object(response: &str) -> Result<&str> {
        let trimmed = response.trim();
        let without_fence = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .map(|s| s.trim())
            .unwrap_or(trimmed);
        let without_fence = without_fence
            .strip_suffix("```")
            .map(|s| s.trim())
            .unwrap_or(without_fence);

        let start = without_fence
            .find('{')
            .ok_or_else(|| anyhow::anyhow!("候选仲裁响应缺少 JSON 对象"))?;
        let end = without_fence
            .rfind('}')
            .ok_or_else(|| anyhow::anyhow!("候选仲裁响应缺少 JSON 对象结束符"))?;

        if start > end {
            anyhow::bail!("候选仲裁响应 JSON 范围无效");
        }

        Ok(&without_fence[start..=end])
    }

    /// 文本润色
    ///
    /// 使用当前激活的预设对 ASR 转写文本进行润色
    ///
    /// # Arguments
    /// * `raw_text` - ASR 转写的原始文本
    ///
    /// # Returns
    /// * 润色后的文本
    pub async fn polish_transcript(
        &self,
        raw_text: &str,
        dictionary: &[String],
        enable_post_process: bool,
        enable_dictionary_enhancement: bool,
    ) -> Result<String> {
        if raw_text.trim().is_empty() {
            return Ok(String::new());
        }

        let system_prompt = if enable_post_process {
            let base_prompt = self.get_active_system_prompt();
            if enable_dictionary_enhancement {
                // 两者都开：追加词库增强指令到用户预设后
                tracing::info!(
                    "LLM 后处理使用预设 ID: {} + 词库增强",
                    self.config.active_preset_id
                );
                format!("{}{}", base_prompt, Self::DICTIONARY_ENHANCEMENT_SUFFIX)
            } else {
                tracing::info!("LLM 后处理使用预设 ID: {}", self.config.active_preset_id);
                base_prompt
            }
        } else {
            tracing::info!("LLM 后处理: 仅词库增强（未启用语句润色）");
            Self::DICTIONARY_ONLY_SYSTEM_PROMPT.to_string()
        };

        let user_message =
            Self::build_user_message(raw_text, dictionary, enable_dictionary_enhancement);

        self.client
            .chat_simple(&system_prompt, &user_message, ChatOptions::for_polishing())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmFeatureConfig, LlmPreset, SharedLlmConfig};

    fn create_test_config() -> LlmConfig {
        LlmConfig {
            shared: SharedLlmConfig {
                providers: Vec::new(),
                default_provider_id: String::new(),
                polishing_provider_id: None,
                assistant_provider_id: None,
                learning_provider_id: None,
                endpoint: Some("https://api.example.com/v1/chat/completions".to_string()),
                api_key: Some("test-key".to_string()),
                default_model: Some("test-model".to_string()),
                polishing_model: None,
                assistant_model: None,
                learning_model: None,
            },
            feature_override: LlmFeatureConfig::default(),
            presets: vec![LlmPreset {
                id: "test".to_string(),
                name: "Test Preset".to_string(),
                system_prompt: "You are a test assistant.".to_string(),
                provider_id: None,
                model: None,
            }],
            active_preset_id: "test".to_string(),
        }
    }

    #[test]
    fn test_get_active_system_prompt() {
        let config = create_test_config();
        let processor = LlmPostProcessor::new(config);
        let prompt = processor.get_active_system_prompt();
        assert_eq!(prompt, "You are a test assistant.");
    }

    #[test]
    fn test_get_active_system_prompt_fallback() {
        let mut config = create_test_config();
        config.active_preset_id = "non-existent".to_string();
        let processor = LlmPostProcessor::new(config);
        let prompt = processor.get_active_system_prompt();
        assert_eq!(prompt, "You are a helpful assistant.");
    }

    #[test]
    fn test_build_user_message_without_dictionary() {
        let msg = LlmPostProcessor::build_user_message("hello", &[], true);
        assert!(msg.contains("<source_text>"));
        assert!(!msg.contains("<user_dictionary>"));
    }

    #[test]
    fn test_build_user_message_with_dictionary_enabled() {
        let dict = vec![
            "张三".to_string(),
            "  北京  ".to_string(),
            "张三".to_string(),
        ];
        let msg = LlmPostProcessor::build_user_message("你好", &dict, true);
        assert!(msg.contains("<dictionary>"));
        assert!(msg.contains("张三"));
        assert!(msg.contains("北京"));
        assert!(msg.contains("<source_text>"));
    }

    #[test]
    fn test_build_user_message_with_dictionary_disabled() {
        let dict = vec!["张三".to_string()];
        let msg = LlmPostProcessor::build_user_message("你好", &dict, false);
        assert!(!msg.contains("<user_dictionary>"));
    }

    fn pending_candidate(
        id: String,
        original: &str,
        target: &str,
        start: usize,
        end: usize,
    ) -> crate::tnl::TnlCandidate {
        crate::tnl::TnlCandidate {
            id,
            original: original.to_string(),
            target: target.to_string(),
            start,
            end,
            score: 0.72,
            risk: crate::tnl::TnlCandidateRisk::Medium,
            source: crate::tnl::TnlCandidateSource::DictionaryPhonetic,
            evidence: vec!["test".to_string()],
            decision: crate::tnl::TnlCandidateDecision::PendingLlm,
        }
    }

    #[test]
    fn test_apply_candidate_arbitration_response_updates_text_and_diagnostics() {
        let diagnostics = crate::tnl::TnlDiagnostics {
            candidates: vec![pending_candidate(
                "candidate-4-11-0".to_string(),
                "Cruiser",
                "Cursor",
                4,
                11,
            )],
            arbitration: None,
        };

        let result = LlmPostProcessor::apply_candidate_arbitration_response(
            "用 Cruiser 打开",
            diagnostics,
            r#"{"decisions":[{"id":"candidate-4-11-0","action":"apply","reason":"技术上下文明确"}]}"#,
            42,
        )
        .expect("仲裁 JSON 应可解析");

        assert_eq!(result.text, "用 Cursor 打开");
        assert_eq!(result.elapsed_ms, 42);
        assert_eq!(
            result.diagnostics.candidates[0].decision,
            crate::tnl::TnlCandidateDecision::AppliedLlm
        );
        assert_eq!(result.diagnostics.arbitration.unwrap().applied_count, 1);
    }

    #[test]
    fn test_apply_candidate_arbitration_response_rejects_missing_decision() {
        let diagnostics = crate::tnl::TnlDiagnostics {
            candidates: vec![pending_candidate(
                "candidate-0-7-0".to_string(),
                "Cruiser",
                "Cursor",
                0,
                7,
            )],
            arbitration: None,
        };

        let result = LlmPostProcessor::apply_candidate_arbitration_response(
            "Cruiser",
            diagnostics,
            r#"{"decisions":[]}"#,
            12,
        )
        .expect("空决策列表也应保守处理");

        assert_eq!(result.text, "Cruiser");
        assert_eq!(
            result.diagnostics.candidates[0].decision,
            crate::tnl::TnlCandidateDecision::RejectedLlm
        );
    }

    #[test]
    fn test_apply_candidate_arbitration_response_rejects_overlapping_accepts() {
        let diagnostics = crate::tnl::TnlDiagnostics {
            candidates: vec![
                pending_candidate(
                    "candidate-0-10-long".to_string(),
                    "cloud code",
                    "Claude Code",
                    0,
                    10,
                ),
                pending_candidate("candidate-0-5-short".to_string(), "cloud", "Claude", 0, 5),
            ],
            arbitration: None,
        };

        let result = LlmPostProcessor::apply_candidate_arbitration_response(
            "cloud code",
            diagnostics,
            r#"{"decisions":[{"id":"candidate-0-10-long","action":"apply","reason":"短语更完整"},{"id":"candidate-0-5-short","action":"apply","reason":"单词也相似"}]}"#,
            18,
        )
        .expect("仲裁 JSON 应可解析");

        assert_eq!(result.text, "Claude Code");
        assert_eq!(
            result.diagnostics.candidates[0].decision,
            crate::tnl::TnlCandidateDecision::AppliedLlm
        );
        assert_eq!(
            result.diagnostics.candidates[1].decision,
            crate::tnl::TnlCandidateDecision::RejectedLlm
        );
        assert!(result.diagnostics.candidates[1]
            .evidence
            .contains(&"llm_overlap_rejected".to_string()));
        let arbitration = result.diagnostics.arbitration.unwrap();
        assert_eq!(arbitration.applied_count, 1);
        assert_eq!(arbitration.rejected_count, 1);
    }

    #[test]
    fn test_prepare_candidate_arbitration_candidates_respects_limit() {
        let mut diagnostics = crate::tnl::TnlDiagnostics {
            candidates: (0..7)
                .map(|idx| {
                    pending_candidate(
                        format!("candidate-{}-{}", idx, idx + 1),
                        "Cruiser",
                        "Cursor",
                        idx,
                        idx + 1,
                    )
                })
                .collect(),
            arbitration: None,
        };

        let selected = LlmPostProcessor::prepare_candidate_arbitration_candidates(&mut diagnostics);

        assert_eq!(selected.len(), LlmPostProcessor::MAX_ARBITRATION_CANDIDATES);
        assert_eq!(
            diagnostics.candidates[5].decision,
            crate::tnl::TnlCandidateDecision::SkippedLimit
        );
        assert_eq!(
            diagnostics.candidates[6].decision,
            crate::tnl::TnlCandidateDecision::SkippedLimit
        );
    }

    #[test]
    fn test_candidate_arbitration_message_uses_bounded_context() {
        let text = format!("{}Cruiser{}", "a".repeat(500), "b".repeat(500));
        let candidate = pending_candidate(
            "candidate-500-507-0".to_string(),
            "Cruiser",
            "Cursor",
            500,
            507,
        );

        let msg = LlmPostProcessor::build_candidate_arbitration_user_message(&text, &[candidate]);

        assert!(msg.contains("\"context\""));
        assert!(msg.contains("Cruiser"));
        assert!(!msg.contains(&"a".repeat(300)));
        assert!(!msg.contains(&"b".repeat(300)));
    }

    // ============================================================================
    // PRD per-preset-llm-override — T1-T6 (resolve + hash)
    // ============================================================================

    use crate::config::LlmProvider;

    fn config_with_two_providers() -> LlmConfig {
        LlmConfig {
            shared: SharedLlmConfig {
                providers: vec![
                    LlmProvider {
                        id: "prov-default".to_string(),
                        name: "Default Provider".to_string(),
                        endpoint: "https://default.example.com/v1/chat/completions".to_string(),
                        api_key: "default-key".to_string(),
                        default_model: "default-model".to_string(),
                    },
                    LlmProvider {
                        id: "prov-strong".to_string(),
                        name: "Strong Provider".to_string(),
                        endpoint: "https://strong.example.com/v1/chat/completions".to_string(),
                        api_key: "strong-key".to_string(),
                        default_model: "strong-default-model".to_string(),
                    },
                ],
                default_provider_id: "prov-default".to_string(),
                polishing_provider_id: None,
                assistant_provider_id: None,
                learning_provider_id: None,
                endpoint: None,
                api_key: None,
                default_model: None,
                polishing_model: None,
                assistant_model: None,
                learning_model: None,
            },
            feature_override: LlmFeatureConfig::default(),
            presets: vec![
                LlmPreset {
                    id: "p-default".to_string(),
                    name: "Default Preset".to_string(),
                    system_prompt: "default prompt".to_string(),
                    provider_id: None,
                    model: None,
                },
                LlmPreset {
                    id: "p-override".to_string(),
                    name: "Override Preset".to_string(),
                    system_prompt: "override prompt".to_string(),
                    provider_id: Some("prov-strong".to_string()),
                    model: None,
                },
            ],
            active_preset_id: "p-default".to_string(),
        }
    }

    /// T1: preset.provider_id 覆盖时，resolve 到目标 provider
    #[test]
    fn test_preset_provider_override_resolves() {
        let mut config = config_with_two_providers();
        config.active_preset_id = "p-override".to_string();
        let resolved = config.resolve_polishing();
        assert!(resolved.endpoint.starts_with("https://strong.example.com"));
        assert_eq!(resolved.api_key, "strong-key");
        assert_eq!(resolved.model, "strong-default-model");
    }

    /// T2: preset.provider_id + preset.model 同时覆盖时，model 用 preset.model
    #[test]
    fn test_preset_model_override_resolves() {
        let mut config = config_with_two_providers();
        config.active_preset_id = "p-override".to_string();
        config.presets[1].model = Some("custom-model".to_string());
        let resolved = config.resolve_polishing();
        assert_eq!(resolved.api_key, "strong-key");
        assert_eq!(resolved.model, "custom-model");
    }

    /// T3: preset.provider_id 指向不存在的 provider 时，fallback 到默认链（不 panic）
    /// 边界防御场景（手工编辑 config 才能触发）
    #[test]
    fn test_preset_provider_missing_falls_back() {
        let mut config = config_with_two_providers();
        config.presets[1].provider_id = Some("non-existent-id".to_string());
        config.active_preset_id = "p-override".to_string();
        let resolved = config.resolve_polishing();
        // Falls back to default chain → polishing_provider_id (None) → default_provider_id ("prov-default")
        assert_eq!(resolved.api_key, "default-key");
        assert_eq!(resolved.model, "default-model");
    }

    /// T4: 切换 active_preset 到有覆盖的 preset 时，hash 变化（resolved.* 三元组变了）
    #[test]
    fn test_compute_config_hash_changes_on_preset_switch_with_override() {
        let mut config = config_with_two_providers();
        // active = p-default → resolves to default-key/default-model
        let hash_default = LlmPostProcessor::compute_config_hash(&config);

        config.active_preset_id = "p-override".to_string();
        // active = p-override → resolves to strong-key/strong-default-model
        let hash_override = LlmPostProcessor::compute_config_hash(&config);

        assert_ne!(
            hash_default, hash_override,
            "切到覆盖 preset 后 hash 必须变化，否则 LlmPostProcessor 不会重建"
        );
    }

    /// T5: 仅修改不在 hash 内的字段时 hash 不变（回归测试）
    /// 注意：hash 包含 resolved.endpoint/api_key/model + active_preset_id + 当前 preset 的 system_prompt
    /// 切 preset / 改 system_prompt / 改 provider_id 都会让 hash 变化（这是现状行为）
    /// 此测试断言：仅改 preset.name（不在 hash 内）时 hash 稳定
    #[test]
    fn test_compute_config_hash_stable_when_only_unrelated_field_changes() {
        let config1 = config_with_two_providers();
        let hash1 = LlmPostProcessor::compute_config_hash(&config1);

        let mut config2 = config_with_two_providers();
        config2.presets[0].name = "Renamed Preset".to_string();
        let hash2 = LlmPostProcessor::compute_config_hash(&config2);

        assert_eq!(
            hash1, hash2,
            "改 preset.name 不应影响 hash（name 不在 hash 输入内）"
        );
    }

    /// T6: preset 覆盖 provider 时跳过 shared.polishing_model（核心契约）
    /// 避免徽章文案显示「provider.default_model」但实际行为却用 polishing_model 的错位
    #[test]
    fn test_resolve_skips_shared_polishing_model_when_preset_overrides_provider() {
        let mut config = config_with_two_providers();
        config.shared.polishing_model = Some("shared-polishing-model".to_string());
        config.active_preset_id = "p-override".to_string();
        // p-override.provider_id = prov-strong, model = None
        // 预期：跳过 shared.polishing_model，用 prov-strong.default_model
        let resolved = config.resolve_polishing();
        assert_eq!(resolved.model, "strong-default-model");
        assert_ne!(resolved.model, "shared-polishing-model");
    }
}
