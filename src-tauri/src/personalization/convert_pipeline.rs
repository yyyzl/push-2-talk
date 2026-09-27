use std::{collections::HashSet, time::Instant};

use crate::tnl::{is_common_english_word, Span, SpanType, WindowKey};

use super::correction_pair_store::{CorrectionPair, CorrectionPairStore};
use super::{
    CandidateDecision, ConversionCandidate, ConversionDiagnostics, ConversionResult, MatchKind,
    PassDiagnostics, PersonalizationEngineConfig,
};

const EXACT_TEXT_PASS: &str = "exact_text";
const SYLLABLE_MATCH_PASS: &str = "syllable_match";
const CONTEXT_RANK_BONUS_PER_TOKEN: f32 = 0.08;
const MAX_CONTEXT_RANK_BONUS: f32 = 0.24;
const NAMED_ENTITY_SCORE_BONUS: f32 = 0.04;

pub(super) struct ConvertContext<'a> {
    pub(super) source_text: &'a str,
    pub(super) windows: &'a [WindowKey],
    pub(super) store: &'a CorrectionPairStore,
    pub(super) config: &'a PersonalizationEngineConfig,
    pub(super) technical_spans: &'a [Span],
}

trait ConvertPass {
    fn name(&self) -> &'static str;

    fn enabled(&self, config: &PersonalizationEngineConfig) -> bool;

    fn collect_candidates(
        &self,
        context: &ConvertContext<'_>,
        candidates: &mut Vec<ConversionCandidate>,
    );
}

pub(super) struct ConvertPipeline {
    passes: Vec<Box<dyn ConvertPass>>,
}

impl Default for ConvertPipeline {
    fn default() -> Self {
        Self {
            passes: vec![Box::new(ExactTextPass), Box::new(SyllableMatchPass)],
        }
    }
}

impl ConvertPipeline {
    pub(super) fn run(&self, context: &ConvertContext<'_>) -> ConversionResult {
        let (mut candidates, mut pass_summaries) = self.collect_candidates(context);
        candidates.sort_by(|a, b| {
            let len_a = a.end.saturating_sub(a.start);
            let len_b = b.end.saturating_sub(b.start);
            len_b
                .cmp(&len_a)
                .then_with(|| b.rank_score.total_cmp(&a.rank_score))
                .then_with(|| b.score.total_cmp(&a.score))
                .then_with(|| a.start.cmp(&b.start))
        });

        let mut selected = Vec::new();
        for candidate in &mut candidates {
            if candidate.score < context.config.apply_threshold {
                candidate.decision = CandidateDecision::BelowApplyThreshold;
                continue;
            }
            if let Some(existing) = selected
                .iter()
                .find(|existing: &&ConversionCandidate| overlaps(existing, candidate))
            {
                candidate.decision = CandidateDecision::SkippedOverlap;
                candidate.blocked_by_pair_id = Some(existing.pair_id.clone());
                continue;
            }
            let mut applied = candidate.clone();
            applied.applied = true;
            applied.decision = CandidateDecision::Applied;
            candidate.applied = true;
            candidate.decision = CandidateDecision::Applied;
            selected.push(applied);
        }

        selected.sort_by(|a, b| b.start.cmp(&a.start));
        let mut output = context.source_text.to_string();
        for candidate in &selected {
            output.replace_range(candidate.start..candidate.end, &candidate.target);
        }

        selected.sort_by(|a, b| a.start.cmp(&b.start));
        update_pass_applied_counts(&mut pass_summaries, &selected);

        ConversionResult {
            changed: output != context.source_text,
            text: output,
            diagnostics: ConversionDiagnostics {
                candidates,
                applied: selected,
                pass_summaries,
            },
        }
    }

    fn collect_candidates(
        &self,
        context: &ConvertContext<'_>,
    ) -> (Vec<ConversionCandidate>, Vec<PassDiagnostics>) {
        let mut candidates = Vec::new();
        let mut summaries = Vec::with_capacity(self.passes.len());

        for pass in &self.passes {
            let enabled = pass.enabled(context.config);
            let mut summary = PassDiagnostics::new(pass.name(), enabled);

            if enabled {
                let started_at = Instant::now();
                let before_count = candidates.len();
                pass.collect_candidates(context, &mut candidates);
                summary.record(started_at, candidates.len() - before_count);
            }

            summaries.push(summary);
        }

        (candidates, summaries)
    }

    #[cfg(test)]
    fn pass_names(&self) -> Vec<&'static str> {
        self.passes.iter().map(|pass| pass.name()).collect()
    }
}

struct ExactTextPass;

impl ConvertPass for ExactTextPass {
    fn name(&self) -> &'static str {
        EXACT_TEXT_PASS
    }

    fn enabled(&self, config: &PersonalizationEngineConfig) -> bool {
        config.enable_exact_text_pass
    }

    fn collect_candidates(
        &self,
        context: &ConvertContext<'_>,
        candidates: &mut Vec<ConversionCandidate>,
    ) {
        for window in context.windows {
            let start = window.byte_range.start;
            let end = window.byte_range.end;
            let window_text = window.text.as_str();

            for pair in context.store.lookup_by_text(window_text) {
                push_candidate(
                    candidates,
                    pair,
                    window_text,
                    start,
                    end,
                    exact_score(pair),
                    MatchKind::ExactText,
                    context.source_text,
                );
            }
        }
    }
}

struct SyllableMatchPass;

impl ConvertPass for SyllableMatchPass {
    fn name(&self) -> &'static str {
        SYLLABLE_MATCH_PASS
    }

    fn enabled(&self, config: &PersonalizationEngineConfig) -> bool {
        config.enable_syllable_match_pass
    }

    fn collect_candidates(
        &self,
        context: &ConvertContext<'_>,
        candidates: &mut Vec<ConversionCandidate>,
    ) {
        for window in context.windows {
            let start = window.byte_range.start;
            let end = window.byte_range.end;
            let window_text = window.text.as_str();
            let keys = &window.keys;
            let has_chinese = window.has_chinese;
            let has_ascii = window.has_ascii;
            let has_named_entity_overlap =
                overlaps_named_entity(context.technical_spans, start, end);

            if has_ascii && !has_chinese {
                for key in &keys.en_phonetic_keys {
                    for pair in context.store.lookup_by_en_phonetic(key) {
                        push_candidate(
                            candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            syllable_score(pair, pair.confidence * 0.97, has_named_entity_overlap),
                            MatchKind::EnPhonetic,
                            context.source_text,
                        );
                    }
                }
            }

            if has_chinese && !has_ascii {
                if let Some(key) = &keys.zh_pinyin_fuzzy_key {
                    for pair in context.store.lookup_by_zh_pinyin_fuzzy(key) {
                        push_candidate(
                            candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            syllable_score(pair, pair.confidence * 0.9, has_named_entity_overlap),
                            MatchKind::ZhPinyinFuzzy,
                            context.source_text,
                        );
                    }
                }
            }

            if has_chinese && has_ascii {
                for key in &keys.mixed_keys {
                    for pair in context.store.lookup_by_mixed(key) {
                        push_candidate(
                            candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            syllable_score(pair, pair.confidence * 0.9, has_named_entity_overlap),
                            MatchKind::Mixed,
                            context.source_text,
                        );
                    }
                    for pair in context.store.lookup_by_alias_key(key) {
                        push_candidate(
                            candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            alias_score(pair, has_named_entity_overlap),
                            MatchKind::Alias,
                            context.source_text,
                        );
                    }
                }

                for key in &keys.alias_keys {
                    for pair in context.store.lookup_by_alias_key(key) {
                        push_candidate(
                            candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            alias_score(pair, has_named_entity_overlap),
                            MatchKind::Alias,
                            context.source_text,
                        );
                    }
                }
            }
        }
    }
}

impl PassDiagnostics {
    fn new(name: impl Into<String>, enabled: bool) -> Self {
        Self {
            name: name.into(),
            enabled,
            elapsed_us: 0,
            candidate_count: 0,
            applied_count: 0,
        }
    }

    fn record(&mut self, started_at: Instant, candidate_count: usize) {
        self.elapsed_us = self
            .elapsed_us
            .saturating_add(started_at.elapsed().as_micros() as u64);
        self.candidate_count = self.candidate_count.saturating_add(candidate_count);
    }
}

fn alias_score(pair: &CorrectionPair, named_entity_overlap: bool) -> f32 {
    let score = if pair.is_user_confirmed() {
        pair.confidence * 0.92
    } else {
        pair.confidence * 0.82
    };
    syllable_score(pair, score, named_entity_overlap)
}

fn exact_score(pair: &CorrectionPair) -> f32 {
    if pair.is_manual() {
        1.0
    } else {
        auto_score(pair, pair.confidence.clamp(0.0, 1.0))
    }
}

fn auto_score(pair: &CorrectionPair, score: f32) -> f32 {
    if pair.requires_manual_for_auto_apply() {
        0.0
    } else {
        score
    }
}

fn syllable_score(pair: &CorrectionPair, score: f32, named_entity_overlap: bool) -> f32 {
    let score = auto_score(pair, score);
    if named_entity_overlap && score > 0.0 {
        (score + NAMED_ENTITY_SCORE_BONUS).min(1.0)
    } else {
        score
    }
}

fn overlaps_named_entity(technical_spans: &[Span], start: usize, end: usize) -> bool {
    technical_spans
        .iter()
        .any(|span| span.span_type == SpanType::NamedEntity && start < span.end && span.start < end)
}

fn push_candidate(
    candidates: &mut Vec<ConversionCandidate>,
    pair: &CorrectionPair,
    original: &str,
    start: usize,
    end: usize,
    score: f32,
    match_kind: MatchKind,
    source_text: &str,
) {
    if pair.corrected_text == original {
        return;
    }
    let rank_score = candidate_rank_score(pair, score, source_text, original);

    if let Some(existing) = candidates.iter_mut().find(|candidate| {
        candidate.pair_id == pair.id && candidate.start == start && candidate.end == end
    }) {
        if rank_score > existing.rank_score
            || (rank_score == existing.rank_score && score > existing.score)
        {
            existing.score = score;
            existing.rank_score = rank_score;
            existing.match_kind = match_kind;
        }
        return;
    }

    candidates.push(ConversionCandidate {
        pair_id: pair.id.clone(),
        original: original.to_string(),
        target: pair.corrected_text.clone(),
        start,
        end,
        score,
        rank_score,
        match_kind,
        applied: false,
        decision: CandidateDecision::Pending,
        blocked_by_pair_id: None,
    });
}

fn candidate_rank_score(
    pair: &CorrectionPair,
    score: f32,
    source_text: &str,
    matched_text: &str,
) -> f32 {
    (score * pair.frequency.max(1) as f32) + context_rank_bonus(pair, source_text, matched_text)
}

fn context_rank_bonus(pair: &CorrectionPair, source_text: &str, matched_text: &str) -> f32 {
    let Some(surrounding_context) = pair.surrounding_context.as_deref() else {
        return 0.0;
    };

    let source_terms = context_terms(source_text);
    if source_terms.is_empty() {
        return 0.0;
    }

    let matched_terms = context_terms(matched_text);
    let overlap_count = context_terms(surrounding_context)
        .into_iter()
        .filter(|term| !matched_terms.contains(term) && source_terms.contains(term))
        .count();

    (overlap_count as f32 * CONTEXT_RANK_BONUS_PER_TOKEN).min(MAX_CONTEXT_RANK_BONUS)
}

fn context_terms(text: &str) -> HashSet<String> {
    let mut terms = HashSet::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            current.push(ch);
        } else if !current.is_empty() {
            push_context_term(&mut terms, &current);
            current.clear();
        }
    }

    if !current.is_empty() {
        push_context_term(&mut terms, &current);
    }

    terms
}

fn push_context_term(terms: &mut HashSet<String>, raw: &str) {
    let term = raw
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();

    if term.len() >= 2
        && term.chars().any(|ch| ch.is_ascii_alphabetic())
        && !is_common_english_word(&term)
    {
        terms.insert(term);
    }
}

fn overlaps(a: &ConversionCandidate, b: &ConversionCandidate) -> bool {
    a.start < b.end && b.start < a.end
}

fn update_pass_applied_counts(
    pass_summaries: &mut [PassDiagnostics],
    applied: &[ConversionCandidate],
) {
    for summary in pass_summaries {
        summary.applied_count = applied
            .iter()
            .filter(|candidate| pass_name_for_match_kind(candidate.match_kind) == summary.name)
            .count();
    }
}

fn pass_name_for_match_kind(match_kind: MatchKind) -> &'static str {
    match match_kind {
        MatchKind::ExactText => EXACT_TEXT_PASS,
        MatchKind::EnPhonetic | MatchKind::ZhPinyinFuzzy | MatchKind::Mixed | MatchKind::Alias => {
            SYLLABLE_MATCH_PASS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personalization::{ConversionResult, CorrectionPair};
    use crate::tnl::SyllableLattice;

    #[test]
    fn convert_pipeline_default_pass_order_is_stable() {
        let pipeline = ConvertPipeline::default();

        assert_eq!(
            pipeline.pass_names(),
            vec![EXACT_TEXT_PASS, SYLLABLE_MATCH_PASS]
        );
    }

    #[test]
    fn convert_pipeline_run_returns_complete_conversion_result() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let store = CorrectionPairStore::new(vec![pair]);
        let config = PersonalizationEngineConfig::default();
        let text = "我打开 cloud code";
        let lattice = SyllableLattice::from_asr_text(text);
        let windows = lattice.windows(config.max_window_tokens);
        let context = ConvertContext {
            source_text: &lattice.source_text,
            windows: &windows,
            store: &store,
            config: &config,
            technical_spans: &[],
        };

        let result = ConvertPipeline::default().run(&context);

        assert_eq!(result.text, "我打开 Claude Code");
        assert!(result.changed);
        assert_eq!(result.diagnostics.applied.len(), 1);
        assert_eq!(
            result.diagnostics.applied[0].decision,
            CandidateDecision::Applied
        );
        assert_eq!(pass_summary(&result, EXACT_TEXT_PASS).applied_count, 1);
    }

    fn pass_summary<'a>(result: &'a ConversionResult, name: &str) -> &'a PassDiagnostics {
        result
            .diagnostics
            .pass_summaries
            .iter()
            .find(|summary| summary.name == name)
            .expect("pass summary")
    }
}
