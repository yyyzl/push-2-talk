//! Shared deterministic text preparation for dictation and assistant instructions.
use crate::personalization::{
    apply_default_personalization_with_config_and_spans,
    personalization_candidates_to_tnl_diagnostics,
    record_personalization_arbitration_feedback_from_tnl, write_runtime_diagnostic,
    ConversionResult, PersonalizationEngineConfig,
};
use crate::{
    config::TnlConfig,
    tnl::{TnlDiagnostics, TnlEngine},
};

pub(crate) struct PreparedText {
    pub text: String,
    pub changed: bool,
    pub diagnostics: Option<TnlDiagnostics>,
}

pub(crate) fn prepare(text: &str, dictionary: &[String], config: &TnlConfig) -> PreparedText {
    if !config.enabled {
        return PreparedText {
            text: text.into(),
            changed: false,
            diagnostics: None,
        };
    }
    let normalized =
        TnlEngine::new_with_disfluency_mode(dictionary.to_vec(), config.disfluency_mode)
            .normalize(text);
    let (text, personalization_changed, diagnostics) = maybe_apply_personalization(
        normalized.text,
        PersonalizationEngineConfig::from_tnl_config(config),
        &normalized.technical_spans,
    );
    PreparedText {
        text,
        changed: normalized.changed || personalization_changed,
        diagnostics: merge_tnl_diagnostics(normalized.diagnostics, diagnostics),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_workflows_keep_unknown_words_without_a_dictionary() {
        let result = prepare(
            "嗯，我最近学习了他们的那个标准产品 cloud",
            &[],
            &TnlConfig::default(),
        );
        assert!(result.text.contains("cloud"));
        assert!(!result.text.contains("Claude"));
    }

    #[test]
    fn both_workflows_apply_the_same_explicit_dictionary() {
        let result = prepare(
            "嗯，我最近学习了他们的那个标准产品 cloud",
            &["Claude".into()],
            &TnlConfig::default(),
        );
        assert!(result.text.contains("Claude"));
        assert!(result.changed);
    }

    #[test]
    fn disabled_normalization_keeps_the_exact_input_in_both_workflows() {
        let mut config = TnlConfig::default();
        config.enabled = false;
        let source = "  cloud  ";
        let result = prepare(source, &["Claude".into()], &config);
        assert_eq!(result.text, source);
        assert!(!result.changed);
        assert!(result.diagnostics.is_none());
    }
}

pub(crate) fn record_personalization_arbitration_feedback(diagnostics: &Option<TnlDiagnostics>) {
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

pub(crate) fn merge_tnl_diagnostics(
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

pub(crate) fn maybe_apply_personalization(
    text: String,
    config: PersonalizationEngineConfig,
    technical_spans: &[crate::tnl::Span],
) -> (String, bool, Option<TnlDiagnostics>) {
    let source_text = text.clone();
    let result =
        match apply_default_personalization_with_config_and_spans(text, config, technical_spans) {
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
    log_personalization_result(&source_text, &result.conversion);

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

pub(crate) fn log_personalization_result(source_text: &str, result: &ConversionResult) {
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

const CANDIDATE_ARBITRATION_TIMEOUT_MS: u64 = 800;

pub(crate) async fn arbitrate<F, Fut>(
    text: String,
    diagnostics: Option<TnlDiagnostics>,
    run: F,
) -> (String, Option<TnlDiagnostics>, Option<u64>)
where
    F: FnOnce(String, TnlDiagnostics) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<crate::tnl::TnlCandidateArbitrationResult>>,
{
    let Some(diagnostics) = diagnostics else {
        return (text, None, None);
    };

    if !diagnostics.has_pending_llm() {
        return (text, Some(diagnostics), None);
    }

    tracing::info!(
        "文本处理: 开始 TNL 候选仲裁，候选数: {}",
        diagnostics.pending_llm_count()
    );

    let fallback_diagnostics = diagnostics.clone();
    let arbitration = tokio::time::timeout(
        std::time::Duration::from_millis(CANDIDATE_ARBITRATION_TIMEOUT_MS),
        run(text.clone(), diagnostics),
    )
    .await;

    match arbitration {
        Ok(Ok(result)) => {
            tracing::info!("文本处理: TNL 候选仲裁完成 (耗时: {}ms)", result.elapsed_ms);
            (
                result.text,
                Some(result.diagnostics),
                Some(result.elapsed_ms),
            )
        }
        Ok(Err(e)) => {
            tracing::warn!("文本处理: TNL 候选仲裁失败，保守跳过: {}", e);
            let mut diagnostics = fallback_diagnostics;
            diagnostics.mark_pending_skipped(
                crate::tnl::TnlCandidateDecision::SkippedError,
                "arbitration_error",
                None,
            );
            (text, Some(diagnostics), None)
        }
        Err(_) => {
            tracing::warn!("文本处理: TNL 候选仲裁超时，保守跳过");
            let mut diagnostics = fallback_diagnostics;
            diagnostics.mark_pending_skipped(
                crate::tnl::TnlCandidateDecision::SkippedTimeout,
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

#[cfg(test)]
mod arbitration_tests {
    use super::*;
    use crate::tnl::*;
    fn pending() -> TnlDiagnostics {
        TnlDiagnostics {
            candidates: vec![TnlCandidate {
                id: "candidate".into(),
                original: "Cruiser".into(),
                target: "Cursor".into(),
                start: 0,
                end: 7,
                score: 0.72,
                risk: TnlCandidateRisk::Medium,
                source: TnlCandidateSource::DictionaryPhonetic,
                evidence: vec![],
                decision: TnlCandidateDecision::PendingLlm,
            }],
            arbitration: None,
        }
    }
    #[tokio::test]
    async fn arbitration_failure_preserves_text_and_marks_candidate_skipped() {
        let (text, diagnostics, elapsed) =
            arbitrate("Cruiser".into(), Some(pending()), |_, _| async {
                anyhow::bail!("offline")
            })
            .await;
        assert_eq!(text, "Cruiser");
        assert!(elapsed.is_none());
        assert_eq!(
            diagnostics.unwrap().candidates[0].decision,
            TnlCandidateDecision::SkippedError
        );
    }
    #[tokio::test]
    async fn arbitration_uses_the_providers_answer_and_actual_elapsed_time() {
        let (text, _, elapsed) =
            arbitrate("Cruiser".into(), Some(pending()), |_, diagnostics| async {
                Ok(TnlCandidateArbitrationResult {
                    text: "Cursor".into(),
                    diagnostics,
                    elapsed_ms: 23,
                })
            })
            .await;
        assert_eq!(text, "Cursor");
        assert_eq!(elapsed, Some(23));
    }
    #[tokio::test]
    async fn no_candidates_does_not_call_a_provider() {
        let (text, _, elapsed) = arbitrate("Original".into(), None, |_, _| async {
            panic!("must not call")
        })
        .await;
        assert_eq!(text, "Original");
        assert!(elapsed.is_none());
    }
}
