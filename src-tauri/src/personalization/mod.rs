//! 个性化二次解码 MVP。
//!
//! 这一层消费 ASR 已经输出的文本，不重新识别音频。当前同时服务 Week 1
//! mini eval、普通听写和 AI 助手链路中的本地二次解码。

use anyhow::Result;
use std::time::Instant;

mod correction_pair_store;
mod engine;
pub(crate) mod phonetic_keys;
mod runtime_diagnostics;

pub use correction_pair_store::{
    default_correction_pairs_path, record_accepted_correction_pair,
    record_observed_correction_pair, record_rejected_correction_pair, CorrectionPair,
    CorrectionPairStore,
};
pub use engine::{
    CandidateDecision, ConversionCandidate, ConversionDiagnostics, ConversionResult, MatchKind,
    PassDiagnostics, PersonalizationEngine, PersonalizationEngineConfig,
};
pub use runtime_diagnostics::write_runtime_diagnostic;

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
    let engine = PersonalizationEngine::new(store);
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
    let path = default_correction_pairs_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let store = CorrectionPairStore::load_json(&path)?;
    Ok(Some(apply_personalization_with_store(text, store)))
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
}
