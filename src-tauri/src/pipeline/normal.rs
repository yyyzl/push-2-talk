// 普通模式处理管道
//
// 处理流程：ASR结果 → 可选LLM润色 → 自动插入文本
//
// 这是默认的处理模式，保持与原有行为完全兼容
//
// 设计原则：Pipeline 不持有锁，所有依赖通过参数传入

use anyhow::Result;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::types::{PipelineResult, TranscriptionContext, TranscriptionMode};
use crate::config::AppConfig;
use crate::learning::coordinator::start_learning_observation;
use crate::llm_post_processor::LlmPostProcessor;
use crate::personalization::{
    apply_default_personalization_with_config, personalization_candidates_to_tnl_diagnostics,
    record_personalization_arbitration_feedback_from_tnl, write_runtime_diagnostic,
    ConversionResult, PersonalizationEngineConfig,
};
use crate::text_inserter::TextInserter;
use crate::tnl::{TnlCandidateDecision, TnlDiagnostics, TnlEngine};

const CANDIDATE_ARBITRATION_TIMEOUT_MS: u64 = 800;

/// 普通模式处理管道
///
/// 职责：
/// 1. 可选的 LLM 后处理（润色、翻译等）
/// 2. 自动插入文本到当前活动窗口
///
/// 设计：无状态，所有依赖通过 process() 参数传入
pub struct NormalPipeline;

impl NormalPipeline {
    /// 创建普通模式管道
    pub fn new() -> Self {
        Self
    }

    /// 处理 ASR 结果
    ///
    /// # Arguments
    /// * `app` - Tauri 应用句柄（用于发送事件）
    /// * `post_processor` - LLM 后处理器（调用方负责从锁中获取）
    /// * `text_inserter` - 文本插入器（调用方负责从锁中获取）
    /// * `asr_result` - ASR 转录结果
    /// * `asr_time_ms` - ASR 耗时（毫秒）
    /// * `_context` - 上下文（普通模式不使用）
    /// * `target_hwnd` - 目标窗口句柄（用于焦点恢复）
    ///
    /// # Returns
    /// * `Ok(PipelineResult)` - 处理成功
    /// * `Err(e)` - 处理失败
    pub async fn process(
        &self,
        app: &AppHandle,
        post_processor: Option<LlmPostProcessor>,
        enable_post_process: bool,
        dictionary: Vec<String>,
        enable_dictionary_enhancement: bool,
        text_inserter: &mut Option<TextInserter>,
        asr_result: Result<String>,
        asr_time_ms: u64,
        _context: TranscriptionContext, // 普通模式不使用上下文
        target_hwnd: Option<isize>,     // 目标窗口句柄（用于焦点恢复）
    ) -> Result<PipelineResult> {
        // 1. 解包 ASR 结果
        let asr_text = asr_result?;
        tracing::info!(
            "NormalPipeline: 收到 ASR 结果: {} (耗时: {}ms)",
            asr_text,
            asr_time_ms
        );

        // 2. TNL 技术规范化（如果启用）
        let tnl_config = AppConfig::load()
            .map(|(c, _)| c.tnl_config)
            .unwrap_or_default();
        let tnl_enabled = tnl_config.enabled;
        let (text, tnl_changed, tnl_diagnostics) = if tnl_enabled {
            let engine = TnlEngine::new(dictionary.clone());
            let tnl_result = engine.normalize(&asr_text);
            if tnl_result.changed {
                tracing::info!(
                    "NormalPipeline: TNL 规范化: {} → {} (耗时: {}us, 替换: {})",
                    asr_text,
                    tnl_result.text,
                    tnl_result.elapsed_us,
                    tnl_result.applied.len()
                );
            }
            (tnl_result.text, tnl_result.changed, tnl_result.diagnostics)
        } else {
            (asr_text.clone(), false, None)
        };

        // 2.5. 本地个性化二次解码（MVP：仅当 correction_pairs.json 存在时启用）
        let (text, personalization_changed, personalization_diagnostics) = if tnl_enabled {
            Self::maybe_apply_personalization(
                text,
                PersonalizationEngineConfig::from_tnl_config(&tnl_config),
            )
        } else {
            (text, false, None)
        };
        let tnl_diagnostics =
            Self::merge_tnl_diagnostics(tnl_diagnostics, personalization_diagnostics);

        // 注意：历史记录存储 ASR 原文（asr_text），LLM 处理使用 TNL/个性化后的文本（text）

        // 3. 可选候选仲裁（绑定词库增强开关，不改变全文润色逻辑）
        let pre_arbitration_text = text.clone();
        let (text, tnl_diagnostics, candidate_llm_time_ms) = Self::maybe_arbitrate_candidates(
            post_processor.clone(),
            enable_dictionary_enhancement,
            text,
            tnl_diagnostics,
        )
        .await;
        let candidate_changed = text != pre_arbitration_text;
        Self::record_personalization_arbitration_feedback(&tnl_diagnostics);

        // 4. 可选 LLM 后处理
        let (final_text, original_text, llm_time_ms) = Self::maybe_polish(
            app,
            post_processor,
            enable_post_process,
            &dictionary,
            enable_dictionary_enhancement,
            &text,
        )
        .await;
        let combined_llm_time_ms = Self::sum_llm_time(candidate_llm_time_ms, llm_time_ms);

        // 5. 插入前隐藏窗口并主动恢复焦点到目标应用
        // 使用新的焦点恢复机制，确保文本插入到正确的窗口
        super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 6. 插入文本
        let inserted = Self::insert_text(text_inserter, &final_text);

        // 7. 触发学习观察（如果启用且插入成功）
        if inserted {
            if let Some(hwnd) = target_hwnd {
                if let Ok((config, _)) = AppConfig::load() {
                    if config.learning_config.enabled {
                        start_learning_observation(
                            app.clone(),
                            final_text.clone(),
                            hwnd,
                            config.learning_config,
                        );
                    }
                }
            }
        }

        // 8. 返回结果
        // 历史记录存储 ASR 原文（约束 C14）
        // 决定是否显示双栏：
        // - 有 LLM 处理 → 使用 LLM 返回的 original_text
        // - 无 LLM 处理但 TNL/候选仲裁改变了文本 → 设置原文以便前端显示双栏
        // - 无 LLM 处理且 TNL 未改变文本 → 不显示双栏（original_text = None）
        let history_original = if original_text.is_some() {
            original_text
        } else if tnl_changed || personalization_changed || candidate_changed {
            Some(asr_text)
        } else {
            None
        };

        let mut result = PipelineResult::success(
            final_text,
            history_original,
            None, // 普通模式无引用文本
            asr_time_ms,
            combined_llm_time_ms,
            TranscriptionMode::Normal,
            inserted,
        );
        result.tnl_diagnostics = tnl_diagnostics;

        Ok(result)
    }

    async fn maybe_arbitrate_candidates(
        processor: Option<LlmPostProcessor>,
        enable_dictionary_enhancement: bool,
        text: String,
        diagnostics: Option<TnlDiagnostics>,
    ) -> (String, Option<TnlDiagnostics>, Option<u64>) {
        let Some(mut diagnostics) = diagnostics else {
            return (text, None, None);
        };

        if !diagnostics.has_pending_llm() {
            return (text, Some(diagnostics), None);
        }

        if !enable_dictionary_enhancement {
            diagnostics.mark_pending_skipped(
                TnlCandidateDecision::SkippedDisabled,
                "dictionary_enhancement_disabled",
                None,
            );
            return (text, Some(diagnostics), None);
        }

        let Some(processor) = processor else {
            diagnostics.mark_pending_skipped(
                TnlCandidateDecision::SkippedNoProcessor,
                "llm_not_configured",
                None,
            );
            return (text, Some(diagnostics), None);
        };

        tracing::info!(
            "NormalPipeline: 开始 TNL 候选仲裁，候选数: {}",
            diagnostics.pending_llm_count()
        );

        let fallback_diagnostics = diagnostics.clone();
        let arbitration = tokio::time::timeout(
            Duration::from_millis(CANDIDATE_ARBITRATION_TIMEOUT_MS),
            processor.arbitrate_tnl_candidates(&text, diagnostics),
        )
        .await;

        match arbitration {
            Ok(Ok(result)) => {
                tracing::info!(
                    "NormalPipeline: TNL 候选仲裁完成 (耗时: {}ms)",
                    result.elapsed_ms
                );
                (
                    result.text,
                    Some(result.diagnostics),
                    Some(result.elapsed_ms),
                )
            }
            Ok(Err(e)) => {
                tracing::warn!("NormalPipeline: TNL 候选仲裁失败，保守跳过: {}", e);
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedError,
                    "arbitration_error",
                    None,
                );
                (text, Some(diagnostics), None)
            }
            Err(_) => {
                tracing::warn!("NormalPipeline: TNL 候选仲裁超时，保守跳过");
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedTimeout,
                    "arbitration_timeout",
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                );
                (
                    text,
                    Some(diagnostics),
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                )
            }
        }
    }

    fn record_personalization_arbitration_feedback(diagnostics: &Option<TnlDiagnostics>) {
        let Some(diagnostics) = diagnostics else {
            return;
        };

        match record_personalization_arbitration_feedback_from_tnl(diagnostics) {
            Ok(updated_count) if updated_count > 0 => {
                tracing::info!(
                    "NormalPipeline: 个性化 LLM 仲裁反馈已写入，更新纠错对: {}",
                    updated_count
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("NormalPipeline: 写入个性化 LLM 仲裁反馈失败，已忽略: {}", e);
            }
        }
    }

    fn merge_tnl_diagnostics(
        existing: Option<TnlDiagnostics>,
        personalization: Option<TnlDiagnostics>,
    ) -> Option<TnlDiagnostics> {
        match (existing, personalization) {
            (None, None) => None,
            (Some(diagnostics), None) | (None, Some(diagnostics)) => Some(diagnostics),
            (Some(mut existing), Some(personalization)) => {
                existing.candidates.extend(personalization.candidates);
                if existing.arbitration.is_none() {
                    existing.arbitration = personalization.arbitration;
                }
                Some(existing)
            }
        }
    }

    fn maybe_apply_personalization(
        text: String,
        config: PersonalizationEngineConfig,
    ) -> (String, bool, Option<TnlDiagnostics>) {
        let source_text = text.clone();
        let result = match apply_default_personalization_with_config(text, config) {
            Ok(Some(result)) => result,
            Ok(None) => return (source_text, false, None),
            Err(e) => {
                tracing::warn!("NormalPipeline: 加载个性化纠错对失败，保守跳过: {}", e);
                return (source_text, false, None);
            }
        };

        if let Err(e) = write_runtime_diagnostic(&source_text, &result) {
            tracing::warn!("NormalPipeline: 写入个性化诊断失败，已忽略: {}", e);
        }
        Self::log_personalization_result(&source_text, &result.conversion);

        let diagnostics = personalization_candidates_to_tnl_diagnostics(&result.conversion);
        if let Some(diagnostics) = &diagnostics {
            if diagnostics.has_pending_llm() {
                tracing::info!(
                    "NormalPipeline: 个性化候选进入 LLM 仲裁，候选数: {}",
                    diagnostics.pending_llm_count()
                );
            }
        }

        (result.text, result.changed, diagnostics)
    }

    #[cfg(test)]
    fn apply_personalization_with_store(
        text: String,
        store: crate::personalization::CorrectionPairStore,
    ) -> (String, bool) {
        let source_text = text.clone();
        let result = crate::personalization::apply_personalization_with_store(text, store);
        Self::log_personalization_result(&source_text, &result.conversion);

        (result.text, result.changed)
    }

    fn log_personalization_result(source_text: &str, result: &ConversionResult) {
        if result.changed {
            tracing::info!(
                "NormalPipeline: 个性化二次解码: {} → {} (应用: {}, 候选: {})",
                source_text,
                result.text,
                result.diagnostics.applied.len(),
                result.diagnostics.candidates.len()
            );
        }
    }

    fn sum_llm_time(first: Option<u64>, second: Option<u64>) -> Option<u64> {
        match (first, second) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// 可选的 LLM 后处理
    ///
    /// 如果配置了 LLM 后处理器，则调用它进行润色
    /// 失败时返回原文
    async fn maybe_polish(
        app: &AppHandle,
        processor: Option<LlmPostProcessor>,
        enable_post_process: bool,
        dictionary: &[String],
        enable_dictionary_enhancement: bool,
        text: &str,
    ) -> (String, Option<String>, Option<u64>) {
        if !enable_post_process && !enable_dictionary_enhancement {
            return (text.to_string(), None, None);
        }

        // 仅开启词库增强且词库为空：无需调用 LLM
        if !enable_post_process && enable_dictionary_enhancement && dictionary.is_empty() {
            return (text.to_string(), None, None);
        }

        if let Some(processor) = processor {
            tracing::info!("NormalPipeline: 开始 LLM 后处理...");
            let _ = app.emit("post_processing", "polishing");

            let llm_start = Instant::now();
            match processor
                .polish_transcript(
                    text,
                    dictionary,
                    enable_post_process,
                    enable_dictionary_enhancement,
                )
                .await
            {
                Ok(polished) => {
                    let llm_elapsed = llm_start.elapsed().as_millis() as u64;
                    tracing::info!(
                        "NormalPipeline: LLM 后处理完成: {} (耗时: {}ms)",
                        polished,
                        llm_elapsed
                    );
                    (polished, Some(text.to_string()), Some(llm_elapsed))
                }
                Err(e) => {
                    tracing::warn!("NormalPipeline: LLM 后处理失败，使用原文: {}", e);
                    // 通知前端润色失败（脱敏：只发送通用提示，不暴露底层错误细节）
                    let _ = app.emit("polishing_failed", "润色服务暂时不可用");
                    // original_text 保持 None，避免被前端误判为"有润色结果"
                    (text.to_string(), None, None)
                }
            }
        } else {
            (text.to_string(), None, None)
        }
    }

    /// 插入文本到当前活动窗口
    ///
    /// 返回是否成功插入
    fn insert_text(text_inserter: &mut Option<TextInserter>, text: &str) -> bool {
        if let Some(ref mut inserter) = text_inserter {
            match inserter.insert_text(text) {
                Ok(()) => {
                    tracing::info!("NormalPipeline: 文本插入成功");
                    true
                }
                Err(e) => {
                    tracing::error!("NormalPipeline: 插入文本失败: {}", e);
                    false
                }
            }
        } else {
            tracing::warn!("NormalPipeline: TextInserter 未初始化");
            false
        }
    }
}

impl Default for NormalPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personalization::{CorrectionPair, CorrectionPairStore};

    #[test]
    fn test_pipeline_creation() {
        let _pipeline = NormalPipeline::new();
        // Pipeline 现在是无状态的，只需要能创建即可
    }

    #[test]
    fn test_apply_personalization_with_store_changes_known_pair() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        let store = CorrectionPairStore::new(vec![pair]);

        let (text, changed) = NormalPipeline::apply_personalization_with_store(
            "我打开 克劳德 code".to_string(),
            store,
        );

        assert!(changed);
        assert_eq!(text, "我打开 Claude Code");
    }

    #[test]
    fn test_apply_personalization_with_store_keeps_unrelated_text() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let store = CorrectionPairStore::new(vec![pair]);

        let (text, changed) = NormalPipeline::apply_personalization_with_store(
            "I use cloud storage".to_string(),
            store,
        );

        assert!(!changed);
        assert_eq!(text, "I use cloud storage");
    }

    #[test]
    fn test_merge_tnl_diagnostics_keeps_existing_and_personalization_candidates() {
        let existing = TnlDiagnostics {
            candidates: vec![crate::tnl::TnlCandidate {
                id: "tnl-0".to_string(),
                original: "Cruiser".to_string(),
                target: "Cursor".to_string(),
                start: 0,
                end: 7,
                score: 0.72,
                risk: crate::tnl::TnlCandidateRisk::Medium,
                source: crate::tnl::TnlCandidateSource::DictionaryPhonetic,
                evidence: vec!["tnl".to_string()],
                decision: TnlCandidateDecision::PendingLlm,
            }],
            arbitration: None,
        };
        let personalization = TnlDiagnostics {
            candidates: vec![crate::tnl::TnlCandidate {
                id: "personalization-8-18-0".to_string(),
                original: "cloud code".to_string(),
                target: "Claude Code".to_string(),
                start: 8,
                end: 18,
                score: 0.80,
                risk: crate::tnl::TnlCandidateRisk::Medium,
                source: crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair,
                evidence: vec!["pair_id:claude-code".to_string()],
                decision: TnlCandidateDecision::PendingLlm,
            }],
            arbitration: None,
        };

        let merged = NormalPipeline::merge_tnl_diagnostics(Some(existing), Some(personalization))
            .expect("merged diagnostics");

        assert_eq!(merged.candidates.len(), 2);
        assert_eq!(merged.pending_llm_count(), 2);
        assert_eq!(
            merged.candidates[1].source,
            crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair
        );
    }
}
