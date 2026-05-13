use anyhow::{Context, Result};
use push_to_talk_lib::personalization::{
    CandidateDecision, ConversionCandidate, ConversionDiagnostics, CorrectionPairStore, MatchKind,
    PersonalizationEngine, PersonalizationEngineConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_CORRECTION_PAIR_HIT_RATE: f32 = 0.70;
const MAX_FALSE_REPLACEMENT_RATE: f32 = 0.01;
const MAX_P95_LOCAL_LATENCY_MS: f64 = 30.0;
const MAX_DIAGNOSTIC_TEXT_CHARS: usize = 160;
const MAX_DIAGNOSTIC_CANDIDATES: usize = 20;
const DIAGNOSTICS_FILE_NAME: &str = "asr_eval_diagnostics.json";

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvalArgs {
    suite_dir: PathBuf,
    diagnostics_out: Option<PathBuf>,
    disable_exact_text_pass: bool,
    disable_syllable_match_pass: bool,
    allow_quality_gate_failure: bool,
}

impl EvalArgs {
    fn engine_config(&self) -> PersonalizationEngineConfig {
        PersonalizationEngineConfig {
            enable_exact_text_pass: !self.disable_exact_text_pass,
            enable_syllable_match_pass: !self.disable_syllable_match_pass,
            ..PersonalizationEngineConfig::default()
        }
    }
}

#[derive(Debug, Deserialize)]
struct EvalCase {
    audio_id: String,
    #[allow(dead_code)]
    audio_wav_path: Option<String>,
    provider: String,
    raw_asr_text: String,
    expected_text: String,
    #[allow(dead_code)]
    user_final_text: Option<String>,
    category: String,
    #[allow(dead_code)]
    notes: Option<String>,
}

#[derive(Debug)]
struct CaseResult {
    case: EvalCase,
    actual_text: String,
    passed: bool,
    applied_count: usize,
    decision_counts: CandidateDecisionCounts,
    local_latency_ms: f64,
    diagnostics: ConversionDiagnostics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct LatencySummary {
    avg_ms: f64,
    p95_ms: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CandidateDecisionCounts {
    total: usize,
    applied: usize,
    below_threshold: usize,
    skipped_overlap: usize,
    pending: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MatchKindCounts {
    exact_text: usize,
    en_phonetic: usize,
    zh_pinyin_fuzzy: usize,
    mixed: usize,
    alias: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EvalMetrics {
    total: usize,
    passed: usize,
    final_accuracy: f32,
    correction_pair_hit_rate: f32,
    false_replacement_count: usize,
    false_replacement_rate: f32,
    latency: LatencySummary,
    decision_counts: CandidateDecisionCounts,
    candidate_match_counts: MatchKindCounts,
    applied_match_counts: MatchKindCounts,
}

#[derive(Debug, Clone, PartialEq)]
struct QualityGateSummary {
    passed: bool,
    failures: Vec<String>,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let engine_config = args.engine_config();
    let suite_dir = resolve_suite_dir(args.suite_dir);
    let pair_path = suite_dir.join("correction_pairs.json");
    let cases_dir = suite_dir.join("cases");

    let store = CorrectionPairStore::load_json(&pair_path)
        .with_context(|| format!("加载纠错对失败: {}", pair_path.display()))?;
    let engine = PersonalizationEngine::with_config(store, engine_config);
    let cases = load_cases(&cases_dir)?;

    if cases.is_empty() {
        anyhow::bail!("评测集为空: {}", cases_dir.display());
    }

    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        let started_at = Instant::now();
        let conversion = engine.convert(&case.raw_asr_text);
        let local_latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        let passed = conversion.text == case.expected_text;
        results.push(CaseResult {
            case,
            actual_text: conversion.text,
            passed,
            applied_count: conversion.diagnostics.applied.len(),
            decision_counts: count_candidate_decisions(&conversion.diagnostics.candidates),
            local_latency_ms,
            diagnostics: conversion.diagnostics,
        });
    }

    print_report(&results);
    if let Some(output_dir) = &args.diagnostics_out {
        let path = write_diagnostics(&results, &output_dir)?;
        eprintln!("ASR eval diagnostics written: {}", path.display());
    }
    let metrics = compute_metrics(&results);
    let quality_gate = evaluate_quality_gates(&metrics);

    if should_exit_success(&quality_gate, args.allow_quality_gate_failure) {
        if !quality_gate.passed {
            eprintln!(
                "ASR eval quality gate failed but exit is allowed: {}",
                quality_gate.failures.join("; ")
            );
        }
        Ok(())
    } else {
        anyhow::bail!("ASR eval 未通过: {}", quality_gate.failures.join("; "));
    }
}

fn parse_args() -> Result<EvalArgs> {
    parse_args_from(std::env::args().skip(1))
}

fn parse_args_from(args: impl IntoIterator<Item = String>) -> Result<EvalArgs> {
    let mut suite_dir = PathBuf::from("tests/asr_eval");
    let mut diagnostics_out = None;
    let mut disable_exact_text_pass = false;
    let mut disable_syllable_match_pass = false;
    let mut allow_quality_gate_failure = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--suite" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--suite 缺少路径参数");
                };
                suite_dir = PathBuf::from(value);
            }
            "--diagnostics-out" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--diagnostics-out 缺少目录参数");
                };
                diagnostics_out = Some(PathBuf::from(value));
            }
            "--disable-exact-text-pass" => {
                disable_exact_text_pass = true;
            }
            "--disable-syllable-match-pass" => {
                disable_syllable_match_pass = true;
            }
            "--allow-quality-gate-failure" => {
                allow_quality_gate_failure = true;
            }
            _ => {
                anyhow::bail!("未知参数: {}", arg);
            }
        }
    }

    Ok(EvalArgs {
        suite_dir,
        diagnostics_out,
        disable_exact_text_pass,
        disable_syllable_match_pass,
        allow_quality_gate_failure,
    })
}

fn resolve_suite_dir(path: PathBuf) -> PathBuf {
    if path.exists() {
        return path;
    }

    let parent_relative = PathBuf::from("..").join(&path);
    if parent_relative.exists() {
        return parent_relative;
    }

    path
}

fn load_cases(cases_dir: &Path) -> Result<Vec<EvalCase>> {
    let mut cases = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(cases_dir)
        .with_context(|| format!("读取评测目录失败: {}", cases_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("读取 case 失败: {}", path.display()))?;
        let mut file_cases: Vec<EvalCase> = serde_json::from_str(&content)
            .with_context(|| format!("解析 case JSON 失败: {}", path.display()))?;
        cases.append(&mut file_cases);
    }

    Ok(cases)
}

fn print_report(results: &[CaseResult]) {
    let metrics = compute_metrics(results);
    let quality_gate = evaluate_quality_gates(&metrics);

    println!("# ASR Eval Report");
    println!();
    println!("- cases: {}", metrics.total);
    println!("- passed: {}", metrics.passed);
    println!("- final_accuracy: {:.2}%", metrics.final_accuracy * 100.0);
    println!(
        "- correction_pair_hit_rate: {:.2}%",
        metrics.correction_pair_hit_rate * 100.0
    );
    println!(
        "- false_replacement_rate: {:.2}%",
        metrics.false_replacement_rate * 100.0
    );
    println!(
        "- false_replacement_count: {}",
        metrics.false_replacement_count
    );
    println!("- avg_latency_ms: {:.3}", metrics.latency.avg_ms);
    println!("- p95_latency_ms: {:.3}", metrics.latency.p95_ms);
    println!("- quality_gate_passed: {}", quality_gate.passed);
    println!("- candidates_total: {}", metrics.decision_counts.total);
    println!("- applied_candidates: {}", metrics.decision_counts.applied);
    println!(
        "- below_threshold_candidates: {}",
        metrics.decision_counts.below_threshold
    );
    println!(
        "- skipped_overlap_candidates: {}",
        metrics.decision_counts.skipped_overlap
    );
    println!("- pending_candidates: {}", metrics.decision_counts.pending);
    println!(
        "- exact_text_candidates: {}",
        metrics.candidate_match_counts.exact_text
    );
    println!(
        "- en_phonetic_candidates: {}",
        metrics.candidate_match_counts.en_phonetic
    );
    println!(
        "- zh_pinyin_fuzzy_candidates: {}",
        metrics.candidate_match_counts.zh_pinyin_fuzzy
    );
    println!(
        "- mixed_candidates: {}",
        metrics.candidate_match_counts.mixed
    );
    println!(
        "- alias_candidates: {}",
        metrics.candidate_match_counts.alias
    );
    println!(
        "- exact_text_applied: {}",
        metrics.applied_match_counts.exact_text
    );
    println!(
        "- en_phonetic_applied: {}",
        metrics.applied_match_counts.en_phonetic
    );
    println!(
        "- zh_pinyin_fuzzy_applied: {}",
        metrics.applied_match_counts.zh_pinyin_fuzzy
    );
    println!("- mixed_applied: {}", metrics.applied_match_counts.mixed);
    println!("- alias_applied: {}", metrics.applied_match_counts.alias);
    if !quality_gate.passed {
        for failure in &quality_gate.failures {
            println!("- quality_gate_failure: {}", failure);
        }
    }
    println!();
    println!("| ID | Provider | Category | Result | Latency(ms) | Candidates | Applied | Raw | Actual | Expected |");
    println!("|---|---|---|---|---:|---:|---:|---|---|---|");

    for result in results {
        println!(
            "| {} | {} | {} | {} | {:.3} | {} | {} | {} | {} | {} |",
            escape_md(&result.case.audio_id),
            escape_md(&result.case.provider),
            escape_md(&result.case.category),
            if result.passed { "PASS" } else { "FAIL" },
            result.local_latency_ms,
            result.decision_counts.total,
            result.applied_count,
            escape_md(&result.case.raw_asr_text),
            escape_md(&result.actual_text),
            escape_md(&result.case.expected_text),
        );
    }
}

fn write_diagnostics(results: &[CaseResult], output_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("创建诊断目录失败: {}", output_dir.display()))?;
    let path = output_dir.join(DIAGNOSTICS_FILE_NAME);
    let payload = EvalDiagnosticsPayload::from_results(results);
    let content = serde_json::to_string_pretty(&payload)?;
    fs::write(&path, content).with_context(|| format!("写入诊断文件失败: {}", path.display()))?;
    Ok(path)
}

fn compute_metrics(results: &[CaseResult]) -> EvalMetrics {
    let total = results.len();
    let passed = results.iter().filter(|result| result.passed).count();
    let final_accuracy = ratio(passed, total);
    let correction_pair_hit_rate = ratio(
        results
            .iter()
            .filter(|result| result.applied_count > 0)
            .count(),
        total,
    );
    let false_replacement_count = results
        .iter()
        .filter(|result| {
            result.case.category == "false_positive_guard"
                && result.actual_text != result.case.expected_text
        })
        .count();
    let false_positive_guard_count = results
        .iter()
        .filter(|result| result.case.category == "false_positive_guard")
        .count();
    let false_replacement_rate = ratio(false_replacement_count, false_positive_guard_count.max(1));
    let latencies = results
        .iter()
        .map(|result| result.local_latency_ms)
        .collect::<Vec<_>>();

    EvalMetrics {
        total,
        passed,
        final_accuracy,
        correction_pair_hit_rate,
        false_replacement_count,
        false_replacement_rate,
        latency: summarize_latency_ms(&latencies),
        decision_counts: summarize_candidate_decisions(results),
        candidate_match_counts: summarize_candidate_match_kinds(results),
        applied_match_counts: summarize_applied_match_kinds(results),
    }
}

fn evaluate_quality_gates(metrics: &EvalMetrics) -> QualityGateSummary {
    let mut failures = Vec::new();

    if metrics.passed != metrics.total {
        failures.push(format!(
            "final_accuracy {:.2}% below 100.00%",
            metrics.final_accuracy * 100.0
        ));
    }
    if metrics.correction_pair_hit_rate < MIN_CORRECTION_PAIR_HIT_RATE {
        failures.push(format!(
            "correction_pair_hit_rate {:.2}% below {:.2}%",
            metrics.correction_pair_hit_rate * 100.0,
            MIN_CORRECTION_PAIR_HIT_RATE * 100.0
        ));
    }
    if metrics.false_replacement_rate > MAX_FALSE_REPLACEMENT_RATE {
        failures.push(format!(
            "false_replacement_rate {:.2}% above {:.2}%",
            metrics.false_replacement_rate * 100.0,
            MAX_FALSE_REPLACEMENT_RATE * 100.0
        ));
    }
    if metrics.latency.p95_ms > MAX_P95_LOCAL_LATENCY_MS {
        failures.push(format!(
            "p95_latency_ms {:.3} above {:.3}",
            metrics.latency.p95_ms, MAX_P95_LOCAL_LATENCY_MS
        ));
    }
    if metrics.decision_counts.pending > 0 {
        failures.push(format!(
            "pending_candidates {} above 0",
            metrics.decision_counts.pending
        ));
    }

    QualityGateSummary {
        passed: failures.is_empty(),
        failures,
    }
}

fn should_exit_success(
    quality_gate: &QualityGateSummary,
    allow_quality_gate_failure: bool,
) -> bool {
    quality_gate.passed || allow_quality_gate_failure
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn count_candidate_decisions(candidates: &[ConversionCandidate]) -> CandidateDecisionCounts {
    let mut counts = CandidateDecisionCounts {
        total: candidates.len(),
        ..CandidateDecisionCounts::default()
    };

    for candidate in candidates {
        match candidate.decision {
            CandidateDecision::Applied => counts.applied += 1,
            CandidateDecision::BelowApplyThreshold => counts.below_threshold += 1,
            CandidateDecision::SkippedOverlap => counts.skipped_overlap += 1,
            CandidateDecision::Pending => counts.pending += 1,
        }
    }

    counts
}

fn summarize_candidate_decisions(results: &[CaseResult]) -> CandidateDecisionCounts {
    results
        .iter()
        .fold(CandidateDecisionCounts::default(), |mut total, result| {
            total.total += result.decision_counts.total;
            total.applied += result.decision_counts.applied;
            total.below_threshold += result.decision_counts.below_threshold;
            total.skipped_overlap += result.decision_counts.skipped_overlap;
            total.pending += result.decision_counts.pending;
            total
        })
}

fn summarize_candidate_match_kinds(results: &[CaseResult]) -> MatchKindCounts {
    results
        .iter()
        .fold(MatchKindCounts::default(), |mut total, result| {
            total.add_candidates(&result.diagnostics.candidates);
            total
        })
}

fn summarize_applied_match_kinds(results: &[CaseResult]) -> MatchKindCounts {
    results
        .iter()
        .fold(MatchKindCounts::default(), |mut total, result| {
            total.add_candidates(&result.diagnostics.applied);
            total
        })
}

impl MatchKindCounts {
    fn add_candidates(&mut self, candidates: &[ConversionCandidate]) {
        for candidate in candidates {
            self.add(candidate.match_kind);
        }
    }

    fn add(&mut self, match_kind: MatchKind) {
        match match_kind {
            MatchKind::ExactText => self.exact_text += 1,
            MatchKind::EnPhonetic => self.en_phonetic += 1,
            MatchKind::ZhPinyinFuzzy => self.zh_pinyin_fuzzy += 1,
            MatchKind::Mixed => self.mixed += 1,
            MatchKind::Alias => self.alias += 1,
        }
    }
}

fn summarize_latency_ms(values: &[f64]) -> LatencySummary {
    if values.is_empty() {
        return LatencySummary::default();
    }

    let avg_ms = values.iter().sum::<f64>() / values.len() as f64;
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = ((sorted.len() as f64) * 0.95).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);

    LatencySummary {
        avg_ms,
        p95_ms: sorted[index],
    }
}

fn escape_md(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let truncated = value.chars().take(max_chars).collect::<String>();
    format!("{truncated}...")
}

#[derive(Debug, Serialize)]
struct EvalDiagnosticsPayload {
    schema_version: u8,
    cases: Vec<EvalCaseDiagnostics>,
}

impl EvalDiagnosticsPayload {
    fn from_results(results: &[CaseResult]) -> Self {
        Self {
            schema_version: 1,
            cases: results
                .iter()
                .map(EvalCaseDiagnostics::from_result)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct EvalCaseDiagnostics {
    audio_id: String,
    provider: String,
    category: String,
    passed: bool,
    raw_asr_text: String,
    actual_text: String,
    expected_text: String,
    local_latency_ms: f64,
    candidate_count: usize,
    applied_count: usize,
    candidates: Vec<Value>,
    applied: Vec<Value>,
}

impl EvalCaseDiagnostics {
    fn from_result(result: &CaseResult) -> Self {
        Self {
            audio_id: truncate_chars(&result.case.audio_id, MAX_DIAGNOSTIC_TEXT_CHARS),
            provider: truncate_chars(&result.case.provider, MAX_DIAGNOSTIC_TEXT_CHARS),
            category: truncate_chars(&result.case.category, MAX_DIAGNOSTIC_TEXT_CHARS),
            passed: result.passed,
            raw_asr_text: truncate_chars(&result.case.raw_asr_text, MAX_DIAGNOSTIC_TEXT_CHARS),
            actual_text: truncate_chars(&result.actual_text, MAX_DIAGNOSTIC_TEXT_CHARS),
            expected_text: truncate_chars(&result.case.expected_text, MAX_DIAGNOSTIC_TEXT_CHARS),
            local_latency_ms: result.local_latency_ms,
            candidate_count: result.diagnostics.candidates.len(),
            applied_count: result.diagnostics.applied.len(),
            candidates: result
                .diagnostics
                .candidates
                .iter()
                .take(MAX_DIAGNOSTIC_CANDIDATES)
                .map(bounded_candidate_json)
                .collect(),
            applied: result
                .diagnostics
                .applied
                .iter()
                .take(MAX_DIAGNOSTIC_CANDIDATES)
                .map(bounded_candidate_json)
                .collect(),
        }
    }
}

fn bounded_candidate_json(candidate: &ConversionCandidate) -> Value {
    let mut value = serde_json::to_value(candidate).unwrap_or(Value::Null);
    truncate_json_strings(&mut value, MAX_DIAGNOSTIC_TEXT_CHARS);
    value
}

fn truncate_json_strings(value: &mut Value, max_chars: usize) {
    match value {
        Value::String(text) => {
            *text = truncate_chars(text, max_chars);
        }
        Value::Array(values) => {
            for item in values {
                truncate_json_strings(item, max_chars);
            }
        }
        Value::Object(values) => {
            for item in values.values_mut() {
                truncate_json_strings(item, max_chars);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use push_to_talk_lib::personalization::CorrectionPair;

    #[test]
    fn summarize_latency_returns_zero_for_empty_input() {
        assert_eq!(summarize_latency_ms(&[]), LatencySummary::default());
    }

    #[test]
    fn summarize_latency_uses_nearest_rank_p95() {
        let summary = summarize_latency_ms(&[2.0, 1.0, 4.0, 100.0]);

        assert!((summary.avg_ms - 26.75).abs() < f64::EPSILON);
        assert!((summary.p95_ms - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summarize_candidate_decisions_adds_case_counts() {
        let results = vec![
            case_result_with_counts(CandidateDecisionCounts {
                total: 3,
                applied: 1,
                below_threshold: 2,
                skipped_overlap: 0,
                pending: 0,
            }),
            case_result_with_counts(CandidateDecisionCounts {
                total: 2,
                applied: 1,
                below_threshold: 0,
                skipped_overlap: 1,
                pending: 0,
            }),
        ];

        assert_eq!(
            summarize_candidate_decisions(&results),
            CandidateDecisionCounts {
                total: 5,
                applied: 2,
                below_threshold: 2,
                skipped_overlap: 1,
                pending: 0,
            }
        );
    }

    #[test]
    fn summarize_match_kinds_counts_candidates_and_applied_independently() {
        let mut result = case_result_with_counts(CandidateDecisionCounts::default());
        result.diagnostics.candidates = vec![
            candidate_with_match_kind(MatchKind::ExactText, true),
            candidate_with_match_kind(MatchKind::EnPhonetic, true),
            candidate_with_match_kind(MatchKind::EnPhonetic, false),
            candidate_with_match_kind(MatchKind::Mixed, false),
            candidate_with_match_kind(MatchKind::Alias, true),
        ];
        result.diagnostics.applied = result
            .diagnostics
            .candidates
            .iter()
            .filter(|candidate| candidate.applied)
            .cloned()
            .collect();

        assert_eq!(
            summarize_candidate_match_kinds(std::slice::from_ref(&result)),
            MatchKindCounts {
                exact_text: 1,
                en_phonetic: 2,
                mixed: 1,
                alias: 1,
                ..MatchKindCounts::default()
            }
        );
        assert_eq!(
            summarize_applied_match_kinds(std::slice::from_ref(&result)),
            MatchKindCounts {
                exact_text: 1,
                en_phonetic: 1,
                alias: 1,
                ..MatchKindCounts::default()
            }
        );
    }

    #[test]
    fn quality_gate_passes_when_metrics_meet_thresholds() {
        let summary = evaluate_quality_gates(&EvalMetrics {
            total: 5,
            passed: 5,
            final_accuracy: 1.0,
            correction_pair_hit_rate: 0.80,
            false_replacement_count: 0,
            false_replacement_rate: 0.0,
            latency: LatencySummary {
                avg_ms: 1.0,
                p95_ms: 10.0,
            },
            decision_counts: CandidateDecisionCounts::default(),
            candidate_match_counts: MatchKindCounts::default(),
            applied_match_counts: MatchKindCounts::default(),
        });

        assert!(summary.passed);
        assert!(summary.failures.is_empty());
    }

    #[test]
    fn quality_gate_reports_all_failed_thresholds() {
        let summary = evaluate_quality_gates(&EvalMetrics {
            total: 5,
            passed: 4,
            final_accuracy: 0.80,
            correction_pair_hit_rate: 0.60,
            false_replacement_count: 1,
            false_replacement_rate: 0.20,
            latency: LatencySummary {
                avg_ms: 20.0,
                p95_ms: 31.0,
            },
            decision_counts: CandidateDecisionCounts {
                pending: 1,
                ..CandidateDecisionCounts::default()
            },
            candidate_match_counts: MatchKindCounts::default(),
            applied_match_counts: MatchKindCounts::default(),
        });

        assert!(!summary.passed);
        assert_eq!(summary.failures.len(), 5);
        assert!(summary
            .failures
            .iter()
            .any(|failure| failure.contains("final_accuracy")));
        assert!(summary
            .failures
            .iter()
            .any(|failure| failure.contains("correction_pair_hit_rate")));
        assert!(summary
            .failures
            .iter()
            .any(|failure| failure.contains("false_replacement_rate")));
        assert!(summary
            .failures
            .iter()
            .any(|failure| failure.contains("p95_latency_ms")));
        assert!(summary
            .failures
            .iter()
            .any(|failure| failure.contains("pending_candidates")));
    }

    #[test]
    fn parse_args_accepts_suite_and_diagnostics_output() {
        let args = parse_args_from([
            "--suite".to_string(),
            "tests/custom_eval".to_string(),
            "--diagnostics-out".to_string(),
            "target/asr-diagnostics".to_string(),
            "--disable-syllable-match-pass".to_string(),
            "--allow-quality-gate-failure".to_string(),
        ])
        .expect("parse args");

        assert_eq!(args.suite_dir, PathBuf::from("tests/custom_eval"));
        assert_eq!(
            args.diagnostics_out,
            Some(PathBuf::from("target/asr-diagnostics"))
        );
        assert!(args.disable_syllable_match_pass);
        assert!(args.allow_quality_gate_failure);
        let config = args.engine_config();
        assert!(config.enable_exact_text_pass);
        assert!(!config.enable_syllable_match_pass);
    }

    #[test]
    fn parse_args_can_disable_exact_text_pass() {
        let args = parse_args_from(["--disable-exact-text-pass".to_string()])
            .expect("parse exact text flag");

        assert!(args.disable_exact_text_pass);
        assert!(!args.engine_config().enable_exact_text_pass);
    }

    #[test]
    fn quality_gate_failure_override_only_changes_exit_success() {
        let failed = QualityGateSummary {
            passed: false,
            failures: vec!["final_accuracy below 100%".to_string()],
        };
        let passed = QualityGateSummary {
            passed: true,
            failures: Vec::new(),
        };

        assert!(!should_exit_success(&failed, false));
        assert!(should_exit_success(&failed, true));
        assert!(should_exit_success(&passed, false));
    }

    #[test]
    fn truncate_chars_limits_without_splitting_unicode() {
        assert_eq!(truncate_chars("克劳德code", 4), "克劳德c...");
        assert_eq!(truncate_chars("Claude", 10), "Claude");
    }

    #[test]
    fn truncate_json_strings_bounds_nested_payload() {
        let mut value = serde_json::json!({
            "candidate": {
                "target": "Claude Code".repeat(MAX_DIAGNOSTIC_TEXT_CHARS + 1),
                "aliases": ["克劳德".repeat(MAX_DIAGNOSTIC_TEXT_CHARS + 1)]
            }
        });

        truncate_json_strings(&mut value, MAX_DIAGNOSTIC_TEXT_CHARS);

        assert!(value["candidate"]["target"]
            .as_str()
            .expect("target")
            .ends_with("..."));
        assert!(value["candidate"]["aliases"][0]
            .as_str()
            .expect("alias")
            .ends_with("..."));
    }

    #[test]
    fn diagnostics_export_truncates_text_and_candidate_lists() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let mut result = case_result_with_counts(CandidateDecisionCounts::default());
        result.case.raw_asr_text = "克劳德".repeat(MAX_DIAGNOSTIC_TEXT_CHARS + 1);
        result.actual_text = "Claude Code".repeat(MAX_DIAGNOSTIC_TEXT_CHARS + 1);
        let pairs = (0..MAX_DIAGNOSTIC_CANDIDATES + 5)
            .map(|idx| {
                let mut pair = CorrectionPair::new(
                    format!("pair-{idx}"),
                    "cloud code",
                    format!("Claude {idx}"),
                );
                pair.source = "manual".to_string();
                pair.confidence = 0.98;
                pair
            })
            .collect();
        let conversion =
            PersonalizationEngine::new(CorrectionPairStore::new(pairs)).convert("cloud code");
        assert!(conversion.diagnostics.candidates.len() > MAX_DIAGNOSTIC_CANDIDATES);
        result.diagnostics = conversion.diagnostics;

        let path = write_diagnostics(&[result], temp.path()).expect("write diagnostics");
        let payload: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read diagnostics"))
                .expect("parse diagnostics");

        let case = &payload["cases"][0];
        assert_eq!(
            case["candidates"].as_array().expect("candidates").len(),
            MAX_DIAGNOSTIC_CANDIDATES
        );
        assert!(case["raw_asr_text"]
            .as_str()
            .expect("raw text")
            .ends_with("..."));
        assert!(case["actual_text"]
            .as_str()
            .expect("actual text")
            .ends_with("..."));
    }

    fn case_result_with_counts(decision_counts: CandidateDecisionCounts) -> CaseResult {
        CaseResult {
            case: EvalCase {
                audio_id: "case".to_string(),
                audio_wav_path: None,
                provider: "fixture".to_string(),
                raw_asr_text: "raw".to_string(),
                expected_text: "expected".to_string(),
                user_final_text: None,
                category: "test".to_string(),
                notes: None,
            },
            actual_text: "actual".to_string(),
            passed: false,
            applied_count: decision_counts.applied,
            decision_counts,
            local_latency_ms: 0.0,
            diagnostics: ConversionDiagnostics::default(),
        }
    }

    fn candidate_with_match_kind(match_kind: MatchKind, applied: bool) -> ConversionCandidate {
        ConversionCandidate {
            pair_id: "pair".to_string(),
            original: "raw".to_string(),
            target: "target".to_string(),
            start: 0,
            end: 3,
            score: 1.0,
            rank_score: 1.0,
            match_kind,
            applied,
            decision: if applied {
                CandidateDecision::Applied
            } else {
                CandidateDecision::BelowApplyThreshold
            },
            blocked_by_pair_id: None,
        }
    }
}
