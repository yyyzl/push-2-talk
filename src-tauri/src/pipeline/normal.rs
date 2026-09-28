// 普通模式处理管道
//
// 处理流程：ASR结果 → 可选LLM润色 → 自动插入文本
//
// 这是默认的处理模式，保持与原有行为完全兼容
//
// 设计原则：Pipeline 不持有锁，所有依赖通过参数传入

use crate::platform::InputTarget;
use anyhow::Result;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use super::types::{PipelineResult, TranscriptionMode};
use crate::learning::coordinator::start_learning_observation;
use crate::llm_post_processor::LlmPostProcessor;

use crate::text_inserter::TextInserter;
use crate::tnl::{TnlCandidateDecision, TnlDiagnostics};

fn require_transcript(result: Result<String>) -> Result<String> {
    let text = result?;
    anyhow::ensure!(
        !text.trim().is_empty(),
        "未识别到语音，请检查麦克风输入后重试"
    );
    Ok(text)
}

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
    /// * `settings` - 本轮录音启动时的配置快照
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
        settings: &crate::config::AppConfig,
        target_hwnd: Option<InputTarget>, // 目标窗口句柄（用于焦点恢复）
    ) -> Result<PipelineResult> {
        // 1. 解包 ASR 结果
        // Empty provider responses must not reach focus restoration, paste, learning or history.
        let asr_text = require_transcript(asr_result)?;
        tracing::info!(
            "NormalPipeline: 收到 ASR 结果: {} (耗时: {}ms)",
            asr_text,
            asr_time_ms
        );

        let prepared = super::text::prepare(&asr_text, &dictionary, &settings.tnl_config);
        let text = prepared.text;
        let text_changed = prepared.changed;
        let tnl_diagnostics = prepared.diagnostics;

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
        super::text::record_personalization_arbitration_feedback(&tnl_diagnostics);

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
        let focus_ready = super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 6. 插入文本
        let inserted = focus_ready && Self::insert_text(text_inserter, &final_text, target_hwnd);
        if !inserted {
            let _ = app.emit(
                "error",
                "无法自动粘贴到原输入位置，识别结果已保留在历史记录中，请手动复制",
            );
        }

        // 7. 触发学习观察（如果启用且插入成功）
        if inserted {
            if let Some(hwnd) = target_hwnd {
                if settings.learning_config.enabled {
                    start_learning_observation(
                        app.clone(),
                        final_text.clone(),
                        hwnd,
                        settings.learning_config.clone(),
                    );
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
        } else if text_changed || candidate_changed {
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

        super::text::arbitrate(text, Some(diagnostics), |text, diagnostics| async move {
            processor.arbitrate_tnl_candidates(&text, diagnostics).await
        })
        .await
    }

    #[cfg(test)]
    fn apply_personalization_with_store(
        text: String,
        store: crate::personalization::CorrectionPairStore,
    ) -> (String, bool) {
        let source_text = text.clone();
        let result = crate::personalization::apply_personalization_with_store(text, store);
        super::text::log_personalization_result(&source_text, &result.conversion);

        (result.text, result.changed)
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
    fn insert_text(
        text_inserter: &mut Option<TextInserter>,
        text: &str,
        target: Option<InputTarget>,
    ) -> bool {
        if let Some(ref mut inserter) = text_inserter {
            match inserter.insert_text(text, target) {
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
    fn empty_transcript_is_rejected_before_processing_or_insertion() {
        for text in ["", " \t\n", "\u{3000}"] {
            let error = require_transcript(Ok(text.to_string())).unwrap_err();
            assert!(error.to_string().contains("未识别到语音"));
        }
    }

    #[test]
    fn nonempty_transcript_preserves_content_and_spacing() {
        let text = "  测试语音，123。\n";
        assert_eq!(require_transcript(Ok(text.to_string())).unwrap(), text);
    }

    #[test]
    fn transcript_provider_error_is_preserved() {
        let error = require_transcript(Err(anyhow::anyhow!("provider unavailable"))).unwrap_err();
        assert_eq!(error.to_string(), "provider unavailable");
    }

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

        let merged =
            super::super::text::merge_tnl_diagnostics(Some(existing), Some(personalization))
                .expect("merged diagnostics");

        assert_eq!(merged.candidates.len(), 2);
        assert_eq!(merged.pending_llm_count(), 2);
        assert_eq!(
            merged.candidates[1].source,
            crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair
        );
    }
}
