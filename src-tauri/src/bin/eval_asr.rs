use anyhow::{Context, Result};
use push_to_talk_lib::personalization::{
    CandidateDecision, ConversionCandidate, CorrectionPairStore, PersonalizationEngine,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_CORRECTION_PAIR_HIT_RATE: f32 = 0.70;
const MAX_FALSE_REPLACEMENT_RATE: f32 = 0.01;
const MAX_P95_LOCAL_LATENCY_MS: f64 = 30.0;

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
}

#[derive(Debug, Clone, PartialEq)]
struct QualityGateSummary {
    passed: bool,
    failures: Vec<String>,
}

fn main() -> Result<()> {
    let suite_dir = resolve_suite_dir(parse_suite_dir());
    let pair_path = suite_dir.join("correction_pairs.json");
    let cases_dir = suite_dir.join("cases");

    let store = CorrectionPairStore::load_json(&pair_path)
        .with_context(|| format!("加载纠错对失败: {}", pair_path.display()))?;
    let engine = PersonalizationEngine::new(store);
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
        });
    }

    print_report(&results);
    let metrics = compute_metrics(&results);
    let quality_gate = evaluate_quality_gates(&metrics);

    if quality_gate.passed {
        Ok(())
    } else {
        anyhow::bail!("ASR eval 未通过: {}", quality_gate.failures.join("; "));
    }
}

fn parse_suite_dir() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--suite" {
            if let Some(value) = args.next() {
                return PathBuf::from(value);
            }
        }
    }

    PathBuf::from("tests/asr_eval")
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

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }
}
