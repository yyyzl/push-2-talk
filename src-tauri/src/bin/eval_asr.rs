use anyhow::{Context, Result};
use push_to_talk_lib::personalization::{
    CandidateDecision, ConversionCandidate, ConversionDiagnostics, CorrectionPairStore, MatchKind,
    PassDiagnostics, PersonalizationEngine, PersonalizationEngineConfig,
};
use push_to_talk_lib::{clean_disfluency, DisfluencyMode};
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
const MAX_EVAL_WINDOW_TOKENS: usize = 16;
const DIAGNOSTICS_FILE_NAME: &str = "asr_eval_diagnostics.json";

#[derive(Debug, Clone, PartialEq)]
struct EvalArgs {
    suite_dir: PathBuf,
    diagnostics_out: Option<PathBuf>,
    disable_exact_text_pass: bool,
    disable_syllable_match_pass: bool,
    allow_quality_gate_failure: bool,
    disfluency_mode: DisfluencyMode,
    apply_threshold: Option<f32>,
    max_window_tokens: Option<usize>,
    sweep_thresholds: Vec<f32>,
    sweep_window_tokens: Vec<usize>,
}

impl EvalArgs {
    fn engine_config(&self) -> PersonalizationEngineConfig {
        let mut config = PersonalizationEngineConfig {
            enable_exact_text_pass: !self.disable_exact_text_pass,
            enable_syllable_match_pass: !self.disable_syllable_match_pass,
            ..PersonalizationEngineConfig::default()
        };
        if let Some(apply_threshold) = self.apply_threshold {
            config.apply_threshold = apply_threshold;
        }
        if let Some(max_window_tokens) = self.max_window_tokens {
            config.max_window_tokens = max_window_tokens;
        }
        config
    }

    fn is_sweep(&self) -> bool {
        !self.sweep_thresholds.is_empty() || !self.sweep_window_tokens.is_empty()
    }

    fn sweep_threshold_values(&self) -> Vec<f32> {
        if !self.sweep_thresholds.is_empty() {
            return self.sweep_thresholds.clone();
        }

        vec![self
            .apply_threshold
            .unwrap_or_else(|| PersonalizationEngineConfig::default().apply_threshold)]
    }

    fn sweep_window_token_values(&self) -> Vec<usize> {
        if !self.sweep_window_tokens.is_empty() {
            return self.sweep_window_tokens.clone();
        }

        vec![self
            .max_window_tokens
            .unwrap_or_else(|| PersonalizationEngineConfig::default().max_window_tokens)]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct EvalRunConfig {
    suite_dir: String,
    enable_exact_text_pass: bool,
    enable_syllable_match_pass: bool,
    apply_threshold: f32,
    max_window_tokens: usize,
    allow_quality_gate_failure: bool,
    disfluency_mode: DisfluencyMode,
}

impl EvalRunConfig {
    fn from_args(args: &EvalArgs) -> Self {
        let engine_config = args.engine_config();
        Self {
            suite_dir: args.suite_dir.display().to_string(),
            enable_exact_text_pass: engine_config.enable_exact_text_pass,
            enable_syllable_match_pass: engine_config.enable_syllable_match_pass,
            apply_threshold: engine_config.apply_threshold,
            max_window_tokens: engine_config.max_window_tokens,
            allow_quality_gate_failure: args.allow_quality_gate_failure,
            disfluency_mode: args.disfluency_mode,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
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
    #[serde(default)]
    disfluency_mode: Option<DisfluencyMode>,
}

#[derive(Debug)]
struct CaseResult {
    case: EvalCase,
    actual_text: String,
    passed: bool,
    applied_count: usize,
    decision_counts: CandidateDecisionCounts,
    local_latency_ms: f64,
    effective_disfluency_mode: DisfluencyMode,
    diagnostics: ConversionDiagnostics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
struct LatencySummary {
    avg_ms: f64,
    p95_ms: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct CandidateDecisionCounts {
    total: usize,
    applied: usize,
    below_threshold: usize,
    skipped_overlap: usize,
    pending: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct MatchKindCounts {
    exact_text: usize,
    en_phonetic: usize,
    zh_pinyin_fuzzy: usize,
    mixed: usize,
    alias: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct PassSummaryCounts {
    enabled_cases: usize,
    disabled_cases: usize,
    candidate_count: usize,
    applied_count: usize,
    elapsed_us: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct PassSummaryTotals {
    exact_text: PassSummaryCounts,
    syllable_match: PassSummaryCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct EvalMetrics {
    total: usize,
    passed: usize,
    final_accuracy: f32,
    correction_pair_hit_rate: f32,
    exact_text_hit_rate: f32,
    syllable_match_hit_rate: f32,
    false_replacement_count: usize,
    false_replacement_rate: f32,
    latency: LatencySummary,
    decision_counts: CandidateDecisionCounts,
    candidate_match_counts: MatchKindCounts,
    applied_match_counts: MatchKindCounts,
    pass_summary_totals: PassSummaryTotals,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct QualityGateSummary {
    passed: bool,
    failures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct SweepRow {
    apply_threshold: f32,
    max_window_tokens: usize,
    metrics: EvalMetrics,
    quality_gate: QualityGateSummary,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let suite_dir = resolve_suite_dir(args.suite_dir.clone());
    let pair_path = suite_dir.join("correction_pairs.json");
    let cases_dir = suite_dir.join("cases");

    let store = CorrectionPairStore::load_json(&pair_path)
        .with_context(|| format!("加载纠错对失败: {}", pair_path.display()))?;
    let cases = load_cases(&cases_dir)?;

    if cases.is_empty() {
        anyhow::bail!("评测集为空: {}", cases_dir.display());
    }

    if args.is_sweep() {
        run_sweep(&args, &store, &cases)
    } else {
        run_single_eval(&args, &store, &cases)
    }
}

fn run_single_eval(args: &EvalArgs, store: &CorrectionPairStore, cases: &[EvalCase]) -> Result<()> {
    let results = evaluate_cases(store, cases, args.engine_config(), args.disfluency_mode);
    print_report(&results);
    if let Some(output_dir) = &args.diagnostics_out {
        let run_config = EvalRunConfig::from_args(args);
        let path = write_diagnostics(&results, &output_dir, &run_config)?;
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

fn run_sweep(args: &EvalArgs, store: &CorrectionPairStore, cases: &[EvalCase]) -> Result<()> {
    if args.diagnostics_out.is_some() {
        anyhow::bail!("--diagnostics-out 不能和 sweep 参数同时使用");
    }

    let mut rows = Vec::new();
    for apply_threshold in args.sweep_threshold_values() {
        for max_window_tokens in args.sweep_window_token_values() {
            let mut config = args.engine_config();
            config.apply_threshold = apply_threshold;
            config.max_window_tokens = max_window_tokens;
            let results = evaluate_cases(store, cases, config, args.disfluency_mode);
            let metrics = compute_metrics(&results);
            let quality_gate = evaluate_quality_gates(&metrics);
            rows.push(SweepRow {
                apply_threshold,
                max_window_tokens,
                metrics,
                quality_gate,
            });
        }
    }

    print_sweep_report(&rows);
    let failures = rows
        .iter()
        .filter(|row| !row.quality_gate.passed)
        .map(|row| {
            format!(
                "threshold={:.2}, window={}: {}",
                row.apply_threshold,
                row.max_window_tokens,
                row.quality_gate.failures.join("; ")
            )
        })
        .collect::<Vec<_>>();

    if failures.is_empty() || args.allow_quality_gate_failure {
        if !failures.is_empty() {
            eprintln!(
                "ASR eval sweep quality gate failed but exit is allowed: {}",
                failures.join(" | ")
            );
        }
        Ok(())
    } else {
        anyhow::bail!("ASR eval sweep 未通过: {}", failures.join(" | "));
    }
}

fn evaluate_cases(
    store: &CorrectionPairStore,
    cases: &[EvalCase],
    engine_config: PersonalizationEngineConfig,
    default_disfluency_mode: DisfluencyMode,
) -> Vec<CaseResult> {
    let engine = PersonalizationEngine::with_config(store.clone(), engine_config);
    let mut results = Vec::with_capacity(cases.len());
    for case in cases.iter().cloned() {
        let started_at = Instant::now();
        let effective_disfluency_mode = case.disfluency_mode.unwrap_or(default_disfluency_mode);
        let cleaned = clean_disfluency(&case.raw_asr_text, effective_disfluency_mode);
        let conversion = engine.convert(&cleaned.text);
        let local_latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        let passed = conversion.text == case.expected_text;
        results.push(CaseResult {
            case,
            actual_text: conversion.text,
            passed,
            applied_count: conversion.diagnostics.applied.len(),
            decision_counts: count_candidate_decisions(&conversion.diagnostics.candidates),
            local_latency_ms,
            effective_disfluency_mode,
            diagnostics: conversion.diagnostics,
        });
    }

    results
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
    let mut disfluency_mode = DisfluencyMode::Conservative;
    let mut apply_threshold = None;
    let mut max_window_tokens = None;
    let mut sweep_thresholds = Vec::new();
    let mut sweep_window_tokens = Vec::new();
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
            "--disfluency-mode" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--disfluency-mode 缺少模式参数");
                };
                disfluency_mode = parse_disfluency_mode(&value)?;
            }
            "--apply-threshold" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--apply-threshold 缺少数值参数");
                };
                apply_threshold = Some(parse_apply_threshold(&value)?);
            }
            "--max-window-tokens" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--max-window-tokens 缺少数值参数");
                };
                max_window_tokens = Some(parse_max_window_tokens(&value)?);
            }
            "--sweep-thresholds" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--sweep-thresholds 缺少逗号分隔数值参数");
                };
                sweep_thresholds = parse_apply_threshold_list(&value)?;
            }
            "--sweep-window-tokens" => {
                let Some(value) = args.next() else {
                    anyhow::bail!("--sweep-window-tokens 缺少逗号分隔整数参数");
                };
                sweep_window_tokens = parse_max_window_tokens_list(&value)?;
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
        disfluency_mode,
        apply_threshold,
        max_window_tokens,
        sweep_thresholds,
        sweep_window_tokens,
    })
}

fn parse_disfluency_mode(value: &str) -> Result<DisfluencyMode> {
    match value {
        "off" => Ok(DisfluencyMode::Off),
        "conservative" => Ok(DisfluencyMode::Conservative),
        "aggressive" => Ok(DisfluencyMode::Aggressive),
        _ => anyhow::bail!("--disfluency-mode 必须是 off、conservative 或 aggressive: {value}"),
    }
}

fn parse_apply_threshold(value: &str) -> Result<f32> {
    let threshold = value
        .parse::<f32>()
        .with_context(|| format!("--apply-threshold 不是合法数字: {value}"))?;
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        anyhow::bail!("--apply-threshold 必须在 0.0 到 1.0 之间");
    }
    Ok(threshold)
}

fn parse_max_window_tokens(value: &str) -> Result<usize> {
    let max_window_tokens = value
        .parse::<usize>()
        .with_context(|| format!("--max-window-tokens 不是合法整数: {value}"))?;
    if max_window_tokens == 0 || max_window_tokens > MAX_EVAL_WINDOW_TOKENS {
        anyhow::bail!(
            "--max-window-tokens 必须在 1 到 {} 之间",
            MAX_EVAL_WINDOW_TOKENS
        );
    }
    Ok(max_window_tokens)
}

fn parse_apply_threshold_list(value: &str) -> Result<Vec<f32>> {
    parse_comma_separated_values(value, "--sweep-thresholds", parse_apply_threshold)
}

fn parse_max_window_tokens_list(value: &str) -> Result<Vec<usize>> {
    parse_comma_separated_values(value, "--sweep-window-tokens", parse_max_window_tokens)
}

fn parse_comma_separated_values<T>(
    value: &str,
    flag: &str,
    parse_item: impl Fn(&str) -> Result<T>,
) -> Result<Vec<T>> {
    let mut values = Vec::new();
    for raw_item in value.split(',') {
        let item = raw_item.trim();
        if item.is_empty() {
            anyhow::bail!("{flag} 包含空值");
        }
        values.push(parse_item(item)?);
    }

    if values.is_empty() {
        anyhow::bail!("{flag} 至少需要一个值");
    }

    Ok(values)
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
        "- exact_text_hit_rate: {:.2}%",
        metrics.exact_text_hit_rate * 100.0
    );
    println!(
        "- syllable_match_hit_rate: {:.2}%",
        metrics.syllable_match_hit_rate * 100.0
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
    print_pass_summary("exact_text", metrics.pass_summary_totals.exact_text);
    print_pass_summary("syllable_match", metrics.pass_summary_totals.syllable_match);
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

fn print_sweep_report(rows: &[SweepRow]) {
    println!("# ASR Eval Sweep");
    println!();
    println!("| Threshold | WindowTokens | Passed | Accuracy | HitRate | ExactHit | SyllableHit | FalseReplacement | P95(ms) | Applied | BelowThreshold | QualityGate |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|");

    for row in rows {
        println!(
            "| {:.2} | {} | {}/{} | {:.2}% | {:.2}% | {:.2}% | {:.2}% | {:.2}% | {:.3} | {} | {} | {} |",
            row.apply_threshold,
            row.max_window_tokens,
            row.metrics.passed,
            row.metrics.total,
            row.metrics.final_accuracy * 100.0,
            row.metrics.correction_pair_hit_rate * 100.0,
            row.metrics.exact_text_hit_rate * 100.0,
            row.metrics.syllable_match_hit_rate * 100.0,
            row.metrics.false_replacement_rate * 100.0,
            row.metrics.latency.p95_ms,
            row.metrics.decision_counts.applied,
            row.metrics.decision_counts.below_threshold,
            if row.quality_gate.passed {
                "PASS"
            } else {
                "FAIL"
            },
        );
    }

    for row in rows.iter().filter(|row| !row.quality_gate.passed) {
        println!(
            "- quality_gate_failure threshold={:.2} window={}: {}",
            row.apply_threshold,
            row.max_window_tokens,
            row.quality_gate.failures.join("; ")
        );
    }
}

fn write_diagnostics(
    results: &[CaseResult],
    output_dir: &Path,
    run_config: &EvalRunConfig,
) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("创建诊断目录失败: {}", output_dir.display()))?;
    let path = output_dir.join(DIAGNOSTICS_FILE_NAME);
    let payload = EvalDiagnosticsPayload::from_results(results, run_config);
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
    let exact_text_hit_rate = ratio(
        results
            .iter()
            .filter(|result| result_has_exact_text_hit(result))
            .count(),
        total,
    );
    let syllable_match_hit_rate = ratio(
        results
            .iter()
            .filter(|result| result_has_syllable_match_hit(result))
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
        exact_text_hit_rate,
        syllable_match_hit_rate,
        false_replacement_count,
        false_replacement_rate,
        latency: summarize_latency_ms(&latencies),
        decision_counts: summarize_candidate_decisions(results),
        candidate_match_counts: summarize_candidate_match_kinds(results),
        applied_match_counts: summarize_applied_match_kinds(results),
        pass_summary_totals: summarize_pass_summaries(results),
    }
}

fn result_has_exact_text_hit(result: &CaseResult) -> bool {
    result
        .diagnostics
        .applied
        .iter()
        .any(|candidate| candidate.match_kind == MatchKind::ExactText)
}

fn result_has_syllable_match_hit(result: &CaseResult) -> bool {
    result
        .diagnostics
        .applied
        .iter()
        .any(|candidate| candidate.match_kind != MatchKind::ExactText)
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

fn print_pass_summary(name: &str, counts: PassSummaryCounts) {
    println!("- {name}_pass_enabled_cases: {}", counts.enabled_cases);
    println!("- {name}_pass_disabled_cases: {}", counts.disabled_cases);
    println!("- {name}_pass_candidates: {}", counts.candidate_count);
    println!("- {name}_pass_applied: {}", counts.applied_count);
    println!("- {name}_pass_elapsed_us: {}", counts.elapsed_us);
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

fn summarize_pass_summaries(results: &[CaseResult]) -> PassSummaryTotals {
    results
        .iter()
        .fold(PassSummaryTotals::default(), |mut total, result| {
            for summary in &result.diagnostics.pass_summaries {
                match summary.name.as_str() {
                    "exact_text" => add_pass_summary(&mut total.exact_text, summary),
                    "syllable_match" => add_pass_summary(&mut total.syllable_match, summary),
                    _ => {}
                }
            }
            total
        })
}

fn add_pass_summary(total: &mut PassSummaryCounts, summary: &PassDiagnostics) {
    if summary.enabled {
        total.enabled_cases += 1;
    } else {
        total.disabled_cases += 1;
    }
    total.candidate_count = total
        .candidate_count
        .saturating_add(summary.candidate_count);
    total.applied_count = total.applied_count.saturating_add(summary.applied_count);
    total.elapsed_us = total.elapsed_us.saturating_add(summary.elapsed_us);
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
    eval_config: EvalRunConfig,
    metrics: EvalMetrics,
    quality_gate: QualityGateSummary,
    cases: Vec<EvalCaseDiagnostics>,
}

impl EvalDiagnosticsPayload {
    fn from_results(results: &[CaseResult], run_config: &EvalRunConfig) -> Self {
        let metrics = compute_metrics(results);
        let quality_gate = evaluate_quality_gates(&metrics);
        Self {
            schema_version: 4,
            eval_config: run_config.clone(),
            metrics,
            quality_gate,
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
    disfluency_mode: DisfluencyMode,
    local_latency_ms: f64,
    candidate_count: usize,
    applied_count: usize,
    pass_summaries: Vec<Value>,
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
            disfluency_mode: result.effective_disfluency_mode,
            local_latency_ms: result.local_latency_ms,
            candidate_count: result.diagnostics.candidates.len(),
            applied_count: result.diagnostics.applied.len(),
            pass_summaries: result
                .diagnostics
                .pass_summaries
                .iter()
                .map(bounded_json)
                .collect(),
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
    bounded_json(candidate)
}

fn bounded_json<T: Serialize>(payload: &T) -> Value {
    let mut value = serde_json::to_value(payload).unwrap_or(Value::Null);
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
    fn compute_metrics_reports_pass_hit_rates_by_case() {
        let mut exact_case = case_result_with_counts(CandidateDecisionCounts::default());
        exact_case.diagnostics.applied =
            vec![candidate_with_match_kind(MatchKind::ExactText, true)];
        exact_case.applied_count = exact_case.diagnostics.applied.len();

        let mut syllable_case = case_result_with_counts(CandidateDecisionCounts::default());
        syllable_case.diagnostics.applied =
            vec![candidate_with_match_kind(MatchKind::EnPhonetic, true)];
        syllable_case.applied_count = syllable_case.diagnostics.applied.len();

        let mut combined_case = case_result_with_counts(CandidateDecisionCounts::default());
        combined_case.diagnostics.applied = vec![
            candidate_with_match_kind(MatchKind::ExactText, true),
            candidate_with_match_kind(MatchKind::Alias, true),
        ];
        combined_case.applied_count = combined_case.diagnostics.applied.len();

        let untouched_case = case_result_with_counts(CandidateDecisionCounts::default());

        let metrics = compute_metrics(&[exact_case, syllable_case, combined_case, untouched_case]);

        assert_eq!(metrics.correction_pair_hit_rate, 0.75);
        assert_eq!(metrics.exact_text_hit_rate, 0.50);
        assert_eq!(metrics.syllable_match_hit_rate, 0.50);
    }

    #[test]
    fn summarize_pass_summaries_counts_each_pass_independently() {
        let mut first = case_result_with_counts(CandidateDecisionCounts::default());
        first.diagnostics.pass_summaries = vec![
            pass_summary("exact_text", true, 2, 1, 10),
            pass_summary("syllable_match", true, 3, 2, 20),
        ];
        let mut second = case_result_with_counts(CandidateDecisionCounts::default());
        second.diagnostics.pass_summaries = vec![
            pass_summary("exact_text", true, 1, 1, 7),
            pass_summary("syllable_match", false, 0, 0, 0),
        ];

        assert_eq!(
            summarize_pass_summaries(&[first, second]),
            PassSummaryTotals {
                exact_text: PassSummaryCounts {
                    enabled_cases: 2,
                    disabled_cases: 0,
                    candidate_count: 3,
                    applied_count: 2,
                    elapsed_us: 17,
                },
                syllable_match: PassSummaryCounts {
                    enabled_cases: 1,
                    disabled_cases: 1,
                    candidate_count: 3,
                    applied_count: 2,
                    elapsed_us: 20,
                },
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
            exact_text_hit_rate: 0.40,
            syllable_match_hit_rate: 0.40,
            false_replacement_count: 0,
            false_replacement_rate: 0.0,
            latency: LatencySummary {
                avg_ms: 1.0,
                p95_ms: 10.0,
            },
            decision_counts: CandidateDecisionCounts::default(),
            candidate_match_counts: MatchKindCounts::default(),
            applied_match_counts: MatchKindCounts::default(),
            pass_summary_totals: PassSummaryTotals::default(),
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
            exact_text_hit_rate: 0.20,
            syllable_match_hit_rate: 0.40,
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
            pass_summary_totals: PassSummaryTotals::default(),
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
            "--disfluency-mode".to_string(),
            "aggressive".to_string(),
            "--apply-threshold".to_string(),
            "0.75".to_string(),
            "--max-window-tokens".to_string(),
            "3".to_string(),
        ])
        .expect("parse args");

        assert_eq!(args.suite_dir, PathBuf::from("tests/custom_eval"));
        assert_eq!(
            args.diagnostics_out,
            Some(PathBuf::from("target/asr-diagnostics"))
        );
        assert!(args.disable_syllable_match_pass);
        assert!(args.allow_quality_gate_failure);
        assert_eq!(args.disfluency_mode, DisfluencyMode::Aggressive);
        let config = args.engine_config();
        assert!(config.enable_exact_text_pass);
        assert!(!config.enable_syllable_match_pass);
        assert_eq!(config.apply_threshold, 0.75);
        assert_eq!(config.max_window_tokens, 3);
    }

    #[test]
    fn parse_args_can_disable_exact_text_pass() {
        let args = parse_args_from(["--disable-exact-text-pass".to_string()])
            .expect("parse exact text flag");

        assert!(args.disable_exact_text_pass);
        assert!(!args.engine_config().enable_exact_text_pass);
        assert_eq!(args.disfluency_mode, DisfluencyMode::Conservative);
    }

    #[test]
    fn parse_args_rejects_invalid_tuning_values() {
        assert!(parse_args_from(["--apply-threshold".to_string(), "1.5".to_string(),]).is_err());
        assert!(parse_args_from(["--apply-threshold".to_string(), "nan".to_string(),]).is_err());
        assert!(parse_args_from(["--max-window-tokens".to_string(), "0".to_string(),]).is_err());
        assert!(parse_args_from(["--disfluency-mode".to_string(), "fast".to_string(),]).is_err());
        assert!(parse_args_from([
            "--max-window-tokens".to_string(),
            (MAX_EVAL_WINDOW_TOKENS + 1).to_string(),
        ])
        .is_err());
    }

    #[test]
    fn parse_args_accepts_sweep_values() {
        let args = parse_args_from([
            "--sweep-thresholds".to_string(),
            "0.70,0.88,0.99".to_string(),
            "--sweep-window-tokens".to_string(),
            "2,5".to_string(),
        ])
        .expect("parse sweep args");

        assert!(args.is_sweep());
        assert_eq!(args.sweep_thresholds, vec![0.70, 0.88, 0.99]);
        assert_eq!(args.sweep_window_tokens, vec![2, 5]);
    }

    #[test]
    fn evaluate_cases_applies_default_and_case_disfluency_modes() {
        let mut cloud_pair = CorrectionPair::new("cloud-code", "cloud code", "Claude Code");
        cloud_pair.source = "manual".to_string();
        cloud_pair.confidence = 0.98;
        let mut windsurf_pair = CorrectionPair::new("wind-surf", "wind surf", "Windsurf");
        windsurf_pair.source = "manual".to_string();
        windsurf_pair.confidence = 0.98;
        let store = CorrectionPairStore::new(vec![cloud_pair, windsurf_pair]);
        let cases = vec![
            EvalCase {
                audio_id: "default-conservative".to_string(),
                audio_wav_path: None,
                provider: "fixture".to_string(),
                raw_asr_text: "嗯，我打开 cloud code".to_string(),
                expected_text: "我打开 Claude Code".to_string(),
                user_final_text: None,
                category: "disfluency_cleanup".to_string(),
                notes: None,
                disfluency_mode: None,
            },
            EvalCase {
                audio_id: "case-aggressive".to_string(),
                audio_wav_path: None,
                provider: "fixture".to_string(),
                raw_asr_text: "我我我打开 wind surf".to_string(),
                expected_text: "我打开 Windsurf".to_string(),
                user_final_text: None,
                category: "disfluency_cleanup".to_string(),
                notes: None,
                disfluency_mode: Some(DisfluencyMode::Aggressive),
            },
        ];

        let results = evaluate_cases(
            &store,
            &cases,
            PersonalizationEngineConfig::default(),
            DisfluencyMode::Conservative,
        );

        assert!(results.iter().all(|result| result.passed));
        assert_eq!(
            results[0].effective_disfluency_mode,
            DisfluencyMode::Conservative
        );
        assert_eq!(
            results[1].effective_disfluency_mode,
            DisfluencyMode::Aggressive
        );
    }

    #[test]
    fn parse_args_rejects_invalid_sweep_values() {
        assert!(
            parse_args_from(["--sweep-thresholds".to_string(), "0.88,nan".to_string(),]).is_err()
        );
        assert!(parse_args_from(["--sweep-thresholds".to_string(), "0.88,".to_string(),]).is_err());
        assert!(
            parse_args_from(["--sweep-window-tokens".to_string(), "5,0".to_string(),]).is_err()
        );
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

        let run_config = default_run_config();
        let path =
            write_diagnostics(&[result], temp.path(), &run_config).expect("write diagnostics");
        let payload: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read diagnostics"))
                .expect("parse diagnostics");

        assert_eq!(payload["schema_version"], 4);
        let case = &payload["cases"][0];
        assert_eq!(
            case["candidates"].as_array().expect("candidates").len(),
            MAX_DIAGNOSTIC_CANDIDATES
        );
        let pass_summaries = case["pass_summaries"].as_array().expect("pass summaries");
        assert!(pass_summaries
            .iter()
            .any(|summary| summary["name"] == "exact_text"));
        assert!(pass_summaries
            .iter()
            .any(|summary| summary["name"] == "syllable_match"));
        assert!(case["raw_asr_text"]
            .as_str()
            .expect("raw text")
            .ends_with("..."));
        assert!(case["actual_text"]
            .as_str()
            .expect("actual text")
            .ends_with("..."));
    }

    #[test]
    fn diagnostics_export_includes_metrics_and_quality_gate_summary() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let mut exact_case = case_result_with_counts(CandidateDecisionCounts {
            total: 1,
            applied: 1,
            below_threshold: 0,
            skipped_overlap: 0,
            pending: 0,
        });
        exact_case.passed = true;
        exact_case.actual_text = exact_case.case.expected_text.clone();
        exact_case.diagnostics.candidates =
            vec![candidate_with_match_kind(MatchKind::ExactText, true)];
        exact_case.diagnostics.applied = exact_case.diagnostics.candidates.clone();
        exact_case.applied_count = exact_case.diagnostics.applied.len();

        let run_config = default_run_config();
        let path =
            write_diagnostics(&[exact_case], temp.path(), &run_config).expect("write diagnostics");
        let payload: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read diagnostics"))
                .expect("parse diagnostics");

        assert_eq!(payload["schema_version"], 4);
        assert_eq!(payload["metrics"]["total"], 1);
        assert_eq!(payload["metrics"]["passed"], 1);
        assert_eq!(payload["metrics"]["correction_pair_hit_rate"], 1.0);
        assert_eq!(payload["metrics"]["exact_text_hit_rate"], 1.0);
        assert_eq!(payload["metrics"]["syllable_match_hit_rate"], 0.0);
        assert_eq!(payload["quality_gate"]["passed"], true);
        assert!(payload["quality_gate"]["failures"]
            .as_array()
            .expect("quality gate failures")
            .is_empty());
    }

    #[test]
    fn diagnostics_export_includes_effective_eval_config() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let args = parse_args_from([
            "--suite".to_string(),
            "tests/custom_eval".to_string(),
            "--disable-syllable-match-pass".to_string(),
            "--allow-quality-gate-failure".to_string(),
            "--disfluency-mode".to_string(),
            "off".to_string(),
            "--apply-threshold".to_string(),
            "0.75".to_string(),
            "--max-window-tokens".to_string(),
            "3".to_string(),
        ])
        .expect("parse args");
        let run_config = EvalRunConfig::from_args(&args);

        let path = write_diagnostics(
            &[case_result_with_counts(CandidateDecisionCounts::default())],
            temp.path(),
            &run_config,
        )
        .expect("write diagnostics");
        let payload: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read diagnostics"))
                .expect("parse diagnostics");

        assert_eq!(payload["schema_version"], 4);
        assert_eq!(payload["eval_config"]["suite_dir"], "tests/custom_eval");
        assert_eq!(payload["eval_config"]["enable_exact_text_pass"], true);
        assert_eq!(payload["eval_config"]["enable_syllable_match_pass"], false);
        assert_eq!(payload["eval_config"]["apply_threshold"], 0.75);
        assert_eq!(payload["eval_config"]["max_window_tokens"], 3);
        assert_eq!(payload["eval_config"]["allow_quality_gate_failure"], true);
        assert_eq!(payload["eval_config"]["disfluency_mode"], "off");
        assert_eq!(payload["cases"][0]["disfluency_mode"], "conservative");
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
                disfluency_mode: None,
            },
            actual_text: "actual".to_string(),
            passed: false,
            applied_count: decision_counts.applied,
            decision_counts,
            local_latency_ms: 0.0,
            effective_disfluency_mode: DisfluencyMode::Conservative,
            diagnostics: ConversionDiagnostics::default(),
        }
    }

    fn default_run_config() -> EvalRunConfig {
        EvalRunConfig::from_args(&EvalArgs {
            suite_dir: PathBuf::from("tests/asr_eval"),
            diagnostics_out: None,
            disable_exact_text_pass: false,
            disable_syllable_match_pass: false,
            allow_quality_gate_failure: false,
            disfluency_mode: DisfluencyMode::Conservative,
            apply_threshold: None,
            max_window_tokens: None,
            sweep_thresholds: Vec::new(),
            sweep_window_tokens: Vec::new(),
        })
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

    fn pass_summary(
        name: &str,
        enabled: bool,
        candidate_count: usize,
        applied_count: usize,
        elapsed_us: u64,
    ) -> PassDiagnostics {
        PassDiagnostics {
            name: name.to_string(),
            enabled,
            elapsed_us,
            candidate_count,
            applied_count,
        }
    }
}
