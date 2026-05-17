//! 个性化二次解码 MVP。
//!
//! 这一层消费 ASR 已经输出的文本，不重新识别音频。当前同时服务 Week 1
//! mini eval、普通听写和 AI 助手链路中的本地二次解码。

use anyhow::Result;
use std::time::Instant;

use crate::tnl::{
    TnlCandidate, TnlCandidateDecision, TnlCandidateRisk, TnlCandidateSource, TnlDiagnostics,
};

mod app_context_hotwords;
mod correction_pair_store;
mod engine;
pub mod hotword_compiler;
pub(crate) mod phonetic_keys;
mod runtime_diagnostics;
mod user_terms_store;

pub(crate) use app_context_hotwords::augment_dictionary_with_app_context_hotwords;
pub use correction_pair_store::{
    default_correction_pairs_path, record_accepted_correction_pair,
    record_llm_arbitration_feedback_pair, record_observed_correction_pair,
    record_rejected_correction_pair, record_reverted_correction_pair, CorrectionPair,
    CorrectionPairStore,
};
pub use engine::{
    CandidateDecision, ConversionCandidate, ConversionDiagnostics, ConversionResult, MatchKind,
    PassDiagnostics, PersonalizationEngine, PersonalizationEngineConfig,
};
pub use runtime_diagnostics::write_runtime_diagnostic;
pub use user_terms_store::{default_user_terms_db_path, UserTerm, UserTermStore};

const PERSONALIZATION_PENDING_LLM_MIN_SCORE: f32 = 0.68;
const PERSONALIZATION_DIAGNOSTIC_MIN_SCORE: f32 = 0.55;

#[derive(Debug, Clone)]
pub struct PersonalizationRuntimeResult {
    pub text: String,
    pub changed: bool,
    pub conversion: ConversionResult,
    pub elapsed_us: u64,
}

pub fn apply_personalization_with_store(
    text: String,
    store: CorrectionPairStore,
) -> PersonalizationRuntimeResult {
    apply_personalization_with_store_and_config(text, store, PersonalizationEngineConfig::default())
}

pub fn apply_personalization_with_store_and_config(
    text: String,
    store: CorrectionPairStore,
    config: PersonalizationEngineConfig,
) -> PersonalizationRuntimeResult {
    let engine = PersonalizationEngine::with_config(store, config);
    let started_at = Instant::now();
    let conversion = engine.convert(&text);
    let elapsed_us = started_at.elapsed().as_micros() as u64;

    PersonalizationRuntimeResult {
        text: conversion.text.clone(),
        changed: conversion.changed,
        conversion,
        elapsed_us,
    }
}

pub fn apply_default_personalization(text: String) -> Result<Option<PersonalizationRuntimeResult>> {
    apply_default_personalization_with_config(text, PersonalizationEngineConfig::default())
}

pub fn apply_default_personalization_with_config(
    text: String,
    config: PersonalizationEngineConfig,
) -> Result<Option<PersonalizationRuntimeResult>> {
    let path = default_correction_pairs_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let store = CorrectionPairStore::load_json(&path)?;
    Ok(Some(apply_personalization_with_store_and_config(
        text, store, config,
    )))
}

pub(crate) fn personalization_candidates_to_tnl_diagnostics(
    conversion: &ConversionResult,
) -> Option<TnlDiagnostics> {
    if conversion.changed {
        return None;
    }

    let candidates = conversion
        .diagnostics
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| personalization_candidate_to_tnl_candidate(idx, candidate))
        .collect::<Vec<_>>();

    TnlDiagnostics::from_candidates(candidates)
}

fn personalization_candidate_to_tnl_candidate(
    index: usize,
    candidate: &ConversionCandidate,
) -> Option<TnlCandidate> {
    if candidate.decision != CandidateDecision::BelowApplyThreshold {
        return None;
    }

    let (decision, risk) = if candidate.score >= PERSONALIZATION_PENDING_LLM_MIN_SCORE {
        (TnlCandidateDecision::PendingLlm, TnlCandidateRisk::Medium)
    } else if candidate.score >= PERSONALIZATION_DIAGNOSTIC_MIN_SCORE {
        (TnlCandidateDecision::RejectedLocal, TnlCandidateRisk::High)
    } else {
        return None;
    };

    Some(TnlCandidate {
        id: format!(
            "personalization-{}-{}-{}",
            candidate.start, candidate.end, index
        ),
        original: candidate.original.clone(),
        target: candidate.target.clone(),
        start: candidate.start,
        end: candidate.end,
        score: candidate.score,
        risk,
        source: TnlCandidateSource::PersonalizationCorrectionPair,
        evidence: vec![
            format!("pair_id:{}", candidate.pair_id),
            format!("match_kind:{:?}", candidate.match_kind),
            format!("rank_score:{:.3}", candidate.rank_score),
        ],
        decision,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersonalizationArbitrationFeedback {
    pub pair_id: String,
    pub accepted: bool,
}

pub(crate) fn personalization_arbitration_feedback_from_tnl(
    diagnostics: &TnlDiagnostics,
) -> Vec<PersonalizationArbitrationFeedback> {
    diagnostics
        .candidates
        .iter()
        .filter_map(|candidate| {
            if candidate.source != TnlCandidateSource::PersonalizationCorrectionPair {
                return None;
            }

            let accepted = match candidate.decision {
                TnlCandidateDecision::AppliedLlm => true,
                TnlCandidateDecision::RejectedLlm if !is_non_decision_llm_rejection(candidate) => {
                    false
                }
                _ => return None,
            };

            let pair_id = personalization_pair_id(candidate)?;
            Some(PersonalizationArbitrationFeedback { pair_id, accepted })
        })
        .collect()
}

pub(crate) fn record_personalization_arbitration_feedback_from_tnl(
    diagnostics: &TnlDiagnostics,
) -> Result<usize> {
    let mut updated_count = 0usize;
    for feedback in personalization_arbitration_feedback_from_tnl(diagnostics) {
        if record_llm_arbitration_feedback_pair(Some(&feedback.pair_id), feedback.accepted)?
            .is_some()
        {
            updated_count = updated_count.saturating_add(1);
        }
    }
    Ok(updated_count)
}

fn personalization_pair_id(candidate: &TnlCandidate) -> Option<String> {
    candidate
        .evidence
        .iter()
        .find_map(|item| item.strip_prefix("pair_id:"))
        .map(str::trim)
        .filter(|pair_id| !pair_id.is_empty())
        .map(ToOwned::to_owned)
}

fn is_non_decision_llm_rejection(candidate: &TnlCandidate) -> bool {
    candidate
        .evidence
        .iter()
        .any(|item| item == "llm_missing_decision" || item == "llm_overlap_rejected")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_apply_personalization_with_store_returns_conversion_details() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let store = CorrectionPairStore::new(vec![pair]);

        let result = apply_personalization_with_store("我打开 cloud code".to_string(), store);

        assert!(result.changed);
        assert_eq!(result.text, "我打开 Claude Code");
        assert_eq!(result.conversion.text, "我打开 Claude Code");
        assert_eq!(result.conversion.diagnostics.applied.len(), 1);
    }

    #[test]
    fn runtime_config_can_disable_syllable_match_pass() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        let store = CorrectionPairStore::new(vec![pair]);
        let config = PersonalizationEngineConfig {
            enable_syllable_match_pass: false,
            ..PersonalizationEngineConfig::default()
        };

        let result = apply_personalization_with_store_and_config(
            "我打开 克劳德 code".to_string(),
            store,
            config,
        );

        assert!(!result.changed);
        assert_eq!(result.text, "我打开 克劳德 code");
        assert!(result
            .conversion
            .diagnostics
            .pass_summaries
            .iter()
            .any(|summary| summary.name == "syllable_match" && !summary.enabled));
    }

    #[test]
    fn medium_confidence_candidate_exports_pending_llm_diagnostic() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.accepted_count = 1;
        pair.confidence = 0.80;
        let store = CorrectionPairStore::new(vec![pair]);

        let result = apply_personalization_with_store("cloud code".to_string(), store);
        let diagnostics = personalization_candidates_to_tnl_diagnostics(&result.conversion)
            .expect("medium-confidence candidate should enter LLM arbitration");

        assert!(!result.changed);
        assert_eq!(diagnostics.candidates.len(), 1);
        assert_eq!(diagnostics.candidates[0].original, "cloud code");
        assert_eq!(diagnostics.candidates[0].target, "Claude Code");
        assert_eq!(
            diagnostics.candidates[0].decision,
            crate::tnl::TnlCandidateDecision::PendingLlm
        );
        assert_eq!(
            diagnostics.candidates[0].source,
            crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair
        );
    }

    #[test]
    fn low_confidence_candidate_exports_rejected_local_diagnostic() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.accepted_count = 1;
        pair.confidence = 0.60;
        let store = CorrectionPairStore::new(vec![pair]);

        let result = apply_personalization_with_store("cloud code".to_string(), store);
        let diagnostics = personalization_candidates_to_tnl_diagnostics(&result.conversion)
            .expect("low-confidence candidate should stay diagnostic-only");

        assert!(!diagnostics.has_pending_llm());
        assert_eq!(
            diagnostics.candidates[0].decision,
            crate::tnl::TnlCandidateDecision::RejectedLocal
        );
    }

    #[test]
    fn arbitration_feedback_extracts_only_personalization_llm_decisions() {
        let diagnostics = TnlDiagnostics {
            candidates: vec![
                TnlCandidate {
                    id: "personalization-0-10-0".to_string(),
                    original: "cloud code".to_string(),
                    target: "Claude Code".to_string(),
                    start: 0,
                    end: 10,
                    score: 0.80,
                    risk: TnlCandidateRisk::Medium,
                    source: TnlCandidateSource::PersonalizationCorrectionPair,
                    evidence: vec!["pair_id:learned-claude".to_string()],
                    decision: TnlCandidateDecision::AppliedLlm,
                },
                TnlCandidate {
                    id: "personalization-11-19-1".to_string(),
                    original: "open eye".to_string(),
                    target: "OpenAI".to_string(),
                    start: 11,
                    end: 19,
                    score: 0.72,
                    risk: TnlCandidateRisk::Medium,
                    source: TnlCandidateSource::PersonalizationCorrectionPair,
                    evidence: vec!["pair_id:learned-openai".to_string()],
                    decision: TnlCandidateDecision::RejectedLlm,
                },
                TnlCandidate {
                    id: "dictionary-0-7-0".to_string(),
                    original: "Cruiser".to_string(),
                    target: "Cursor".to_string(),
                    start: 0,
                    end: 7,
                    score: 0.72,
                    risk: TnlCandidateRisk::Medium,
                    source: TnlCandidateSource::DictionaryPhonetic,
                    evidence: vec!["pair_id:should-ignore".to_string()],
                    decision: TnlCandidateDecision::AppliedLlm,
                },
                TnlCandidate {
                    id: "personalization-20-25-2".to_string(),
                    original: "cloud".to_string(),
                    target: "Claude".to_string(),
                    start: 20,
                    end: 25,
                    score: 0.60,
                    risk: TnlCandidateRisk::High,
                    source: TnlCandidateSource::PersonalizationCorrectionPair,
                    evidence: vec!["pair_id:rejected-local".to_string()],
                    decision: TnlCandidateDecision::RejectedLocal,
                },
                TnlCandidate {
                    id: "personalization-26-36-3".to_string(),
                    original: "cloud code".to_string(),
                    target: "Claude Code".to_string(),
                    start: 26,
                    end: 36,
                    score: 0.80,
                    risk: TnlCandidateRisk::Medium,
                    source: TnlCandidateSource::PersonalizationCorrectionPair,
                    evidence: vec![
                        "pair_id:missing-decision".to_string(),
                        "llm_missing_decision".to_string(),
                    ],
                    decision: TnlCandidateDecision::RejectedLlm,
                },
                TnlCandidate {
                    id: "personalization-37-42-4".to_string(),
                    original: "cloud".to_string(),
                    target: "Claude".to_string(),
                    start: 37,
                    end: 42,
                    score: 0.74,
                    risk: TnlCandidateRisk::Medium,
                    source: TnlCandidateSource::PersonalizationCorrectionPair,
                    evidence: vec![
                        "pair_id:overlap-rejected".to_string(),
                        "llm_overlap_rejected".to_string(),
                    ],
                    decision: TnlCandidateDecision::RejectedLlm,
                },
            ],
            arbitration: None,
        };

        let feedback = personalization_arbitration_feedback_from_tnl(&diagnostics);

        assert_eq!(
            feedback,
            vec![
                PersonalizationArbitrationFeedback {
                    pair_id: "learned-claude".to_string(),
                    accepted: true,
                },
                PersonalizationArbitrationFeedback {
                    pair_id: "learned-openai".to_string(),
                    accepted: false,
                },
            ]
        );
    }
}
