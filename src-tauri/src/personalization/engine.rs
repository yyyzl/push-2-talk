use serde::{Deserialize, Serialize};

use crate::tnl::{Span, SyllableLattice};

use super::convert_pipeline::{ConvertContext, ConvertPipeline};
use super::correction_pair_store::CorrectionPairStore;

const DEFAULT_MAX_WINDOW_TOKENS: usize = 5;
const DEFAULT_APPLY_THRESHOLD: f32 = 0.88;

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

impl PersonalizationEngineConfig {
    pub fn from_tnl_config(config: &crate::config::TnlConfig) -> Self {
        Self {
            max_window_tokens: config.personalization_max_window_tokens,
            apply_threshold: config.personalization_apply_threshold,
            enable_exact_text_pass: config.enable_personalization_exact_text_pass,
            enable_syllable_match_pass: config.enable_personalization_syllable_match_pass,
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
        self.convert_with_technical_spans(text, &[])
    }

    pub fn convert_with_technical_spans(
        &self,
        text: &str,
        technical_spans: &[Span],
    ) -> ConversionResult {
        if text.trim().is_empty() {
            return ConversionResult {
                text: text.to_string(),
                changed: false,
                diagnostics: ConversionDiagnostics::default(),
            };
        }

        let lattice = SyllableLattice::from_asr_text(text);
        let windows = lattice.windows(self.config.max_window_tokens);
        let context = ConvertContext {
            source_text: &lattice.source_text,
            windows: &windows,
            store: &self.store,
            config: &self.config,
            technical_spans,
        };

        ConvertPipeline::default().run(&context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personalization::CorrectionPair;
    use crate::tnl::{Span, SpanType};

    fn engine() -> PersonalizationEngine {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]))
    }

    #[test]
    fn config_maps_from_tnl_config() {
        let mut tnl_config = crate::config::TnlConfig::default();
        tnl_config.enable_personalization_exact_text_pass = false;
        tnl_config.enable_personalization_syllable_match_pass = false;
        tnl_config.personalization_max_window_tokens = 3;
        tnl_config.personalization_apply_threshold = 0.99;

        let config = PersonalizationEngineConfig::from_tnl_config(&tnl_config);

        assert!(!config.enable_exact_text_pass);
        assert!(!config.enable_syllable_match_pass);
        assert_eq!(config.max_window_tokens, 3);
        assert!((config.apply_threshold - 0.99).abs() < f32::EPSILON);
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
    fn named_entity_alias_candidate_without_span_stays_below_threshold() {
        let engine = named_entity_alias_engine();

        let result = engine.convert("我打开 克劳德 code");

        assert_eq!(result.text, "我打开 克劳德 code");
        let candidate = candidate_by_pair_id(&result, "learned-claude-code");
        assert!(candidate.score < DEFAULT_APPLY_THRESHOLD);
        assert_eq!(candidate.decision, CandidateDecision::BelowApplyThreshold);
        assert!(result.diagnostics.applied.is_empty());
    }

    #[test]
    fn named_entity_span_lifts_alias_candidate_over_threshold() {
        let engine = named_entity_alias_engine();
        let text = "我打开 克劳德 code";
        let spans = vec![named_entity_span(text, "克劳德")];

        let result = engine.convert_with_technical_spans(text, &spans);

        assert_eq!(result.text, "我打开 Claude Code");
        let candidate = candidate_by_pair_id(&result, "learned-claude-code");
        assert!(candidate.score >= DEFAULT_APPLY_THRESHOLD);
        assert_eq!(candidate.decision, CandidateDecision::Applied);
        assert_eq!(candidate.match_kind, MatchKind::Alias);
    }

    #[test]
    fn non_overlapping_named_entity_span_does_not_boost_candidate() {
        let engine = named_entity_alias_engine();
        let text = "北京 打开 克劳德 code";
        let spans = vec![named_entity_span(text, "北京")];

        let result = engine.convert_with_technical_spans(text, &spans);

        assert_eq!(result.text, text);
        let candidate = candidate_by_pair_id(&result, "learned-claude-code");
        assert!(candidate.score < DEFAULT_APPLY_THRESHOLD);
        assert_eq!(candidate.decision, CandidateDecision::BelowApplyThreshold);
    }

    #[test]
    fn named_entity_span_does_not_revive_risky_single_word_pair() {
        let mut pair = CorrectionPair::new("learned-cloud", "cloud", "Claude");
        pair.source = "learned".to_string();
        pair.confidence = 0.98;
        pair.accepted_count = 1;
        let engine = PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]));
        let text = "I use cloud storage";
        let spans = vec![named_entity_span(text, "cloud")];

        let result = engine.convert_with_technical_spans(text, &spans);

        assert_eq!(result.text, text);
        assert!(!result.changed);
        assert!(result
            .diagnostics
            .candidates
            .iter()
            .all(|candidate| candidate.score == 0.0));
    }

    #[test]
    fn named_entity_span_does_not_boost_exact_text_pass() {
        let mut pair = CorrectionPair::new("learned-deepseek", "深度求索", "DeepSeek");
        pair.source = "learned".to_string();
        pair.confidence = DEFAULT_APPLY_THRESHOLD - 0.01;
        let engine = PersonalizationEngine::with_config(
            CorrectionPairStore::new(vec![pair]),
            PersonalizationEngineConfig {
                enable_syllable_match_pass: false,
                ..PersonalizationEngineConfig::default()
            },
        );
        let text = "我在用深度求索";
        let spans = vec![named_entity_span(text, "深度求索")];

        let result = engine.convert_with_technical_spans(text, &spans);

        assert_eq!(result.text, text);
        let candidate = candidate_by_pair_id(&result, "learned-deepseek");
        assert_eq!(candidate.match_kind, MatchKind::ExactText);
        assert!(candidate.score < DEFAULT_APPLY_THRESHOLD);
        assert_eq!(candidate.decision, CandidateDecision::BelowApplyThreshold);
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

    fn named_entity_alias_engine() -> PersonalizationEngine {
        let mut pair = CorrectionPair::new("learned-claude-code", "claud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.confidence = 0.93;
        pair.accepted_count = 1;
        pair.alias_keys.push("kelaode|code".to_string());
        PersonalizationEngine::new(CorrectionPairStore::new(vec![pair]))
    }

    fn named_entity_span(text: &str, term: &str) -> Span {
        let start = text.find(term).expect("span term exists");
        Span {
            text: term.to_string(),
            start,
            end: start + term.len(),
            span_type: SpanType::NamedEntity,
        }
    }

    fn candidate_by_pair_id<'a>(
        result: &'a ConversionResult,
        pair_id: &str,
    ) -> &'a ConversionCandidate {
        result
            .diagnostics
            .candidates
            .iter()
            .find(|candidate| candidate.pair_id == pair_id)
            .expect("candidate")
    }
}
