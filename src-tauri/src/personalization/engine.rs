use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Instant};

use crate::tnl::SyllableLattice;

use super::correction_pair_store::{CorrectionPair, CorrectionPairStore};

const DEFAULT_MAX_WINDOW_TOKENS: usize = 5;
const DEFAULT_APPLY_THRESHOLD: f32 = 0.88;
const EXACT_TEXT_PASS: &str = "exact_text";
const SYLLABLE_MATCH_PASS: &str = "syllable_match";
const CONTEXT_RANK_BONUS_PER_TOKEN: f32 = 0.08;
const MAX_CONTEXT_RANK_BONUS: f32 = 0.24;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PersonalizationEngineConfig {
    pub max_window_tokens: usize,
    pub apply_threshold: f32,
    pub enable_exact_text_pass: bool,
    pub enable_syllable_match_pass: bool,
}

impl Default for PersonalizationEngineConfig {
    fn default() -> Self {
        Self {
            max_window_tokens: DEFAULT_MAX_WINDOW_TOKENS,
            apply_threshold: DEFAULT_APPLY_THRESHOLD,
            enable_exact_text_pass: true,
            enable_syllable_match_pass: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub text: String,
    pub changed: bool,
    pub diagnostics: ConversionDiagnostics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionDiagnostics {
    pub candidates: Vec<ConversionCandidate>,
    pub applied: Vec<ConversionCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pass_summaries: Vec<PassDiagnostics>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PassDiagnostics {
    pub name: String,
    pub enabled: bool,
    pub elapsed_us: u64,
    pub candidate_count: usize,
    pub applied_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionCandidate {
    pub pair_id: String,
    pub original: String,
    pub target: String,
    pub start: usize,
    pub end: usize,
    pub score: f32,
    #[serde(default)]
    pub rank_score: f32,
    pub match_kind: MatchKind,
    pub applied: bool,
    #[serde(default)]
    pub decision: CandidateDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_by_pair_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    ExactText,
    EnPhonetic,
    ZhPinyinFuzzy,
    Mixed,
    Alias,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDecision {
    #[default]
    Pending,
    Applied,
    BelowApplyThreshold,
    SkippedOverlap,
}

pub struct PersonalizationEngine {
    store: CorrectionPairStore,
    config: PersonalizationEngineConfig,
}

impl PersonalizationEngine {
    pub fn new(store: CorrectionPairStore) -> Self {
        Self::with_config(store, PersonalizationEngineConfig::default())
    }

    pub fn with_config(store: CorrectionPairStore, config: PersonalizationEngineConfig) -> Self {
        Self { store, config }
    }

    pub fn convert(&self, text: &str) -> ConversionResult {
        if text.trim().is_empty() {
            return ConversionResult {
                text: text.to_string(),
                changed: false,
                diagnostics: ConversionDiagnostics::default(),
            };
        }

        let lattice = SyllableLattice::from_asr_text(text);
        let (mut candidates, mut pass_summaries) = self.collect_candidates(&lattice);
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
            if candidate.score < self.config.apply_threshold {
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
        let mut output = text.to_string();
        for candidate in &selected {
            output.replace_range(candidate.start..candidate.end, &candidate.target);
        }

        selected.sort_by(|a, b| a.start.cmp(&b.start));
        update_pass_applied_counts(&mut pass_summaries, &selected);

        ConversionResult {
            changed: output != text,
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
        lattice: &SyllableLattice,
    ) -> (Vec<ConversionCandidate>, Vec<PassDiagnostics>) {
        let mut candidates = Vec::new();
        let mut exact_summary =
            PassDiagnostics::new(EXACT_TEXT_PASS, self.config.enable_exact_text_pass);
        let mut syllable_summary =
            PassDiagnostics::new(SYLLABLE_MATCH_PASS, self.config.enable_syllable_match_pass);

        for window in lattice.windows(self.config.max_window_tokens) {
            let start = window.byte_range.start;
            let end = window.byte_range.end;
            let window_text = window.text.as_str();
            let keys = &window.keys;
            let has_chinese = window.has_chinese;
            let has_ascii = window.has_ascii;

            if self.config.enable_exact_text_pass {
                let started_at = Instant::now();
                let before_count = candidates.len();
                for pair in self.store.lookup_by_text(window_text) {
                    push_candidate(
                        &mut candidates,
                        pair,
                        window_text,
                        start,
                        end,
                        exact_score(pair),
                        MatchKind::ExactText,
                        &lattice.source_text,
                    );
                }
                exact_summary.record(started_at, candidates.len() - before_count);
            }

            if !self.config.enable_syllable_match_pass {
                continue;
            }

            let started_at = Instant::now();
            let before_count = candidates.len();
            if has_ascii && !has_chinese {
                for key in &keys.en_phonetic_keys {
                    for pair in self.store.lookup_by_en_phonetic(key) {
                        push_candidate(
                            &mut candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            auto_score(pair, pair.confidence * 0.97),
                            MatchKind::EnPhonetic,
                            &lattice.source_text,
                        );
                    }
                }
            }

            if has_chinese && !has_ascii {
                if let Some(key) = &keys.zh_pinyin_fuzzy_key {
                    for pair in self.store.lookup_by_zh_pinyin_fuzzy(key) {
                        push_candidate(
                            &mut candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            auto_score(pair, pair.confidence * 0.9),
                            MatchKind::ZhPinyinFuzzy,
                            &lattice.source_text,
                        );
                    }
                }
            }

            if has_chinese && has_ascii {
                for key in &keys.mixed_keys {
                    for pair in self.store.lookup_by_mixed(key) {
                        push_candidate(
                            &mut candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            auto_score(pair, pair.confidence * 0.9),
                            MatchKind::Mixed,
                            &lattice.source_text,
                        );
                    }
                    for pair in self.store.lookup_by_alias_key(key) {
                        push_candidate(
                            &mut candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            alias_score(pair),
                            MatchKind::Alias,
                            &lattice.source_text,
                        );
                    }
                }

                for key in &keys.alias_keys {
                    for pair in self.store.lookup_by_alias_key(key) {
                        push_candidate(
                            &mut candidates,
                            pair,
                            window_text,
                            start,
                            end,
                            alias_score(pair),
                            MatchKind::Alias,
                            &lattice.source_text,
                        );
                    }
                }
            }
            syllable_summary.record(started_at, candidates.len() - before_count);
        }

        (candidates, vec![exact_summary, syllable_summary])
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

fn alias_score(pair: &CorrectionPair) -> f32 {
    let score = if pair.is_user_confirmed() {
        pair.confidence * 0.92
    } else {
        pair.confidence * 0.82
    };
    auto_score(pair, score)
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
        && !crate::tnl::is_common_english_word(&term)
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
    use crate::personalization::CorrectionPair;

    fn engine() -> PersonalizationEngine {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]))
    }

    #[test]
    fn corrects_claude_code_family() {
        let engine = engine();

        assert_eq!(
            engine.convert("我打开 cloud code").text,
            "我打开 Claude Code"
        );
        assert_eq!(
            engine.convert("我打开 claud code").text,
            "我打开 Claude Code"
        );
        assert_eq!(
            engine.convert("我打开 cloud coat").text,
            "我打开 Claude Code"
        );
        assert_eq!(
            engine.convert("我打开 克劳德 code").text,
            "我打开 Claude Code"
        );
    }

    #[test]
    fn corrects_chinese_alias_without_whitespace_before_name() {
        let engine = engine();

        assert_eq!(
            engine.convert("我打开克劳德 code").text,
            "我打开Claude Code"
        );
    }

    #[test]
    fn does_not_generalize_cloud_single_word() {
        let engine = engine();

        let result = engine.convert("I use cloud storage");

        assert_eq!(result.text, "I use cloud storage");
        assert!(!result.changed);
    }

    #[test]
    fn does_not_match_phrase_across_sentence_punctuation() {
        let engine = engine();

        let result = engine.convert("先说 cloud。code 再继续");

        assert_eq!(result.text, "先说 cloud。code 再继续");
        assert!(!result.changed);
        assert!(result.diagnostics.applied.is_empty());
    }

    #[test]
    fn learned_single_common_word_pair_does_not_auto_apply() {
        let mut pair = CorrectionPair::new("learned-cloud", "cloud", "Claude");
        pair.source = "learned".to_string();
        pair.confidence = 0.98;
        pair.accepted_count = 1;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("I use cloud storage");

        assert_eq!(result.text, "I use cloud storage");
        assert!(!result.changed);
        assert!(result
            .diagnostics
            .candidates
            .iter()
            .all(|candidate| candidate.score < DEFAULT_APPLY_THRESHOLD));
    }

    #[test]
    fn learned_single_chinese_char_pair_does_not_auto_apply() {
        let mut pair = CorrectionPair::new("learned-ma", "麻", "吗");
        pair.source = "learned".to_string();
        pair.confidence = 0.98;
        pair.accepted_count = 1;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("麻烦打开设置");

        assert_eq!(result.text, "麻烦打开设置");
        assert!(!result.changed);
        assert!(result
            .diagnostics
            .candidates
            .iter()
            .all(|candidate| candidate.score < DEFAULT_APPLY_THRESHOLD));
    }

    #[test]
    fn manual_single_chinese_char_pair_can_auto_apply() {
        let mut pair = CorrectionPair::new("manual-ma", "麻", "吗");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("麻烦打开设置");

        assert_eq!(result.text, "吗烦打开设置");
        assert!(result.changed);
    }

    #[test]
    fn learned_multi_word_pair_with_common_word_still_auto_applies() {
        let mut pair = CorrectionPair::new("learned-cloud-code", "cloud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.confidence = 0.98;
        pair.accepted_count = 1;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("我打开 cloud code");

        assert_eq!(result.text, "我打开 Claude Code");
        assert!(result.changed);
    }

    #[test]
    fn repeated_pair_beats_one_off_candidate_for_same_span() {
        let mut one_off = CorrectionPair::new("one-off-cloud-code", "cloud code", "Cloud Code");
        one_off.source = "learned".to_string();
        one_off.confidence = 0.98;
        one_off.accepted_count = 1;
        one_off.frequency = 1;

        let mut repeated = CorrectionPair::new("repeated-cloud-code", "cloud code", "Claude Code");
        repeated.source = "learned".to_string();
        repeated.confidence = 0.90;
        repeated.accepted_count = 10;
        repeated.frequency = 10;

        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![one_off, repeated]));
        let result = engine.convert("我打开 cloud code");

        assert_eq!(result.text, "我打开 Claude Code");
        assert!(result.changed);
        let applied = result
            .diagnostics
            .candidates
            .iter()
            .find(|candidate| candidate.pair_id == "repeated-cloud-code")
            .expect("repeated candidate");
        assert!(applied.applied);
        assert_eq!(applied.decision, CandidateDecision::Applied);

        let blocked = result
            .diagnostics
            .candidates
            .iter()
            .find(|candidate| candidate.pair_id == "one-off-cloud-code")
            .expect("one-off candidate");
        assert!(!blocked.applied);
        assert_eq!(blocked.decision, CandidateDecision::SkippedOverlap);
        assert_eq!(
            blocked.blocked_by_pair_id.as_deref(),
            Some("repeated-cloud-code")
        );
    }

    #[test]
    fn matching_surrounding_context_boosts_same_span_candidate_rank() {
        let mut jetbrains =
            CorrectionPair::new("jetbrains-cloud-code", "cloud code", "Claude Code");
        jetbrains.source = "learned".to_string();
        jetbrains.confidence = 0.98;
        jetbrains.accepted_count = 1;
        jetbrains.frequency = 1;
        jetbrains.surrounding_context =
            Some("在 JetBrains 项目里把 cloud code 改成 Claude Code".to_string());

        let mut github = CorrectionPair::new("github-cloud-code", "cloud code", "GitHub Copilot");
        github.source = "learned".to_string();
        github.confidence = 0.98;
        github.accepted_count = 1;
        github.frequency = 1;
        github.surrounding_context =
            Some("在 GitHub issue 里把 cloud code 改成 GitHub Copilot".to_string());

        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![jetbrains, github]));
        let result = engine.convert("在 GitHub issue 里打开 cloud code");

        assert_eq!(result.text, "在 GitHub issue 里打开 GitHub Copilot");
        let github_candidate = result
            .diagnostics
            .candidates
            .iter()
            .find(|candidate| candidate.pair_id == "github-cloud-code")
            .expect("github candidate");
        let jetbrains_candidate = result
            .diagnostics
            .candidates
            .iter()
            .find(|candidate| candidate.pair_id == "jetbrains-cloud-code")
            .expect("jetbrains candidate");
        assert!(github_candidate.rank_score > jetbrains_candidate.rank_score);
        assert_eq!(github_candidate.decision, CandidateDecision::Applied);
        assert_eq!(
            jetbrains_candidate.decision,
            CandidateDecision::SkippedOverlap
        );
    }

    #[test]
    fn frequency_does_not_bypass_apply_threshold() {
        let mut pair =
            CorrectionPair::new("low-confidence-cloud-code", "cloud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.confidence = DEFAULT_APPLY_THRESHOLD - 0.01;
        pair.accepted_count = 100;
        pair.frequency = 100;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));
        let result = engine.convert("我打开 cloud code");

        assert_eq!(result.text, "我打开 cloud code");
        assert!(!result.changed);
        assert!(result
            .diagnostics
            .candidates
            .iter()
            .any(|candidate| candidate.rank_score > DEFAULT_APPLY_THRESHOLD));
        assert!(result.diagnostics.candidates.iter().all(|candidate| {
            candidate.decision == CandidateDecision::BelowApplyThreshold && !candidate.applied
        }));
        assert!(result.diagnostics.applied.is_empty());
    }

    #[test]
    fn context_rank_boost_does_not_bypass_apply_threshold() {
        let mut pair = CorrectionPair::new(
            "low-confidence-context-cloud-code",
            "cloud code",
            "Claude Code",
        );
        pair.source = "learned".to_string();
        pair.confidence = DEFAULT_APPLY_THRESHOLD - 0.01;
        pair.accepted_count = 1;
        pair.frequency = 1;
        pair.surrounding_context =
            Some("在 GitHub issue 里把 cloud code 改成 Claude Code".to_string());
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));
        let result = engine.convert("在 GitHub issue 里打开 cloud code");

        assert_eq!(result.text, "在 GitHub issue 里打开 cloud code");
        assert!(!result.changed);
        let candidate = result
            .diagnostics
            .candidates
            .iter()
            .find(|candidate| candidate.pair_id == "low-confidence-context-cloud-code")
            .expect("candidate");
        assert!(candidate.rank_score > candidate.score);
        assert_eq!(candidate.decision, CandidateDecision::BelowApplyThreshold);
        assert!(result.diagnostics.applied.is_empty());
    }

    #[test]
    fn manual_single_common_word_pair_can_auto_apply() {
        let mut pair = CorrectionPair::new("manual-cloud", "cloud", "Claude");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("I use cloud storage");

        assert_eq!(result.text, "I use Claude storage");
        assert!(result.changed);
    }

    #[test]
    fn mixed_language_pair_does_not_auto_apply_to_ascii_tail_only() {
        let mut pair = CorrectionPair::new("openai-mixed", "欧喷 ai", "OpenAI");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("enable ai mode");

        assert_eq!(result.text, "enable ai mode");
        assert!(!result.changed);
        assert!(result.diagnostics.applied.is_empty());
    }

    #[test]
    fn mixed_language_pair_does_not_auto_apply_to_chinese_head_only() {
        let mut pair = CorrectionPair::new("openai-mixed", "欧喷 ai", "OpenAI");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));

        let result = engine.convert("我调用 欧盆 接口");

        assert_eq!(result.text, "我调用 欧盆 接口");
        assert!(!result.changed);
        assert!(result.diagnostics.applied.is_empty());
    }

    #[test]
    fn diagnostics_reports_enabled_pass_summaries() {
        let engine = engine();

        let result = engine.convert("我打开 claud code");

        let exact = pass_summary(&result, "exact_text");
        let syllable = pass_summary(&result, "syllable_match");
        assert!(exact.enabled);
        assert_eq!(exact.candidate_count, 0);
        assert_eq!(exact.applied_count, 0);
        assert!(syllable.enabled);
        assert_eq!(syllable.candidate_count, 1);
        assert_eq!(syllable.applied_count, 1);
    }

    #[test]
    fn diagnostics_reports_disabled_pass_summary() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let engine = PersonalizationEngine::with_config(
            CorrectionPairStore::new(vec![pair]),
            PersonalizationEngineConfig {
                enable_syllable_match_pass: false,
                ..PersonalizationEngineConfig::default()
            },
        );

        let result = engine.convert("我打开 cloud code");

        let exact = pass_summary(&result, "exact_text");
        let syllable = pass_summary(&result, "syllable_match");
        assert!(exact.enabled);
        assert_eq!(exact.candidate_count, 1);
        assert_eq!(exact.applied_count, 1);
        assert!(!syllable.enabled);
        assert_eq!(syllable.candidate_count, 0);
        assert_eq!(syllable.applied_count, 0);
    }

    #[test]
    fn disabling_syllable_match_pass_keeps_exact_pair() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let engine = PersonalizationEngine::with_config(
            CorrectionPairStore::new(vec![pair]),
            PersonalizationEngineConfig {
                enable_syllable_match_pass: false,
                ..PersonalizationEngineConfig::default()
            },
        );

        let result = engine.convert("我打开 cloud code");

        assert_eq!(result.text, "我打开 Claude Code");
        assert!(result.changed);
        assert!(result
            .diagnostics
            .applied
            .iter()
            .all(|candidate| candidate.match_kind == MatchKind::ExactText));
    }

    #[test]
    fn disabling_syllable_match_pass_skips_phonetic_and_alias_pairs() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        let engine = PersonalizationEngine::with_config(
            CorrectionPairStore::new(vec![pair]),
            PersonalizationEngineConfig {
                enable_syllable_match_pass: false,
                ..PersonalizationEngineConfig::default()
            },
        );

        let phonetic = engine.convert("我打开 claud code");
        let alias = engine.convert("我打开 克劳德 code");

        assert_eq!(phonetic.text, "我打开 claud code");
        assert_eq!(alias.text, "我打开 克劳德 code");
        assert!(!phonetic.changed);
        assert!(!alias.changed);
        assert!(phonetic.diagnostics.candidates.is_empty());
        assert!(alias.diagnostics.candidates.is_empty());
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
