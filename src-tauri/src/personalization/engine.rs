use serde::{Deserialize, Serialize};

use crate::tnl::SyllableLattice;

use super::correction_pair_store::{CorrectionPair, CorrectionPairStore};

const DEFAULT_MAX_WINDOW_TOKENS: usize = 5;
const DEFAULT_APPLY_THRESHOLD: f32 = 0.88;

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
    max_window_tokens: usize,
    apply_threshold: f32,
}

impl PersonalizationEngine {
    pub fn new(store: CorrectionPairStore) -> Self {
        Self {
            store,
            max_window_tokens: DEFAULT_MAX_WINDOW_TOKENS,
            apply_threshold: DEFAULT_APPLY_THRESHOLD,
        }
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
        let mut candidates = self.collect_candidates(&lattice);
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
            if candidate.score < self.apply_threshold {
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

        ConversionResult {
            changed: output != text,
            text: output,
            diagnostics: ConversionDiagnostics {
                candidates,
                applied: selected,
            },
        }
    }

    fn collect_candidates(&self, lattice: &SyllableLattice) -> Vec<ConversionCandidate> {
        let mut candidates = Vec::new();

        for window in lattice.windows(self.max_window_tokens) {
            let start = window.byte_range.start;
            let end = window.byte_range.end;
            let window_text = window.text.as_str();
            let keys = &window.keys;
            let has_chinese = window.has_chinese;
            let has_ascii = window.has_ascii;

            for pair in self.store.lookup_by_text(window_text) {
                push_candidate(
                    &mut candidates,
                    pair,
                    window_text,
                    start,
                    end,
                    exact_score(pair),
                    MatchKind::ExactText,
                );
            }

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
                        );
                    }
                }
            }
        }

        candidates
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
) {
    if pair.corrected_text == original {
        return;
    }
    let rank_score = candidate_rank_score(pair, score);

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

fn candidate_rank_score(pair: &CorrectionPair, score: f32) -> f32 {
    score * pair.frequency.max(1) as f32
}

fn overlaps(a: &ConversionCandidate, b: &ConversionCandidate) -> bool {
    a.start < b.end && b.start < a.end
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
    fn does_not_generalize_cloud_single_word() {
        let engine = engine();

        let result = engine.convert("I use cloud storage");

        assert_eq!(result.text, "I use cloud storage");
        assert!(!result.changed);
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
}
