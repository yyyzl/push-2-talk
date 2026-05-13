use anyhow::{Context, Result};
use push_to_talk_lib::personalization::{CorrectionPairStore, PersonalizationEngine};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

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
        let conversion = engine.convert(&case.raw_asr_text);
        let passed = conversion.text == case.expected_text;
        results.push(CaseResult {
            case,
            actual_text: conversion.text,
            passed,
            applied_count: conversion.diagnostics.applied.len(),
        });
    }

    print_report(&results);

    if results.iter().all(|result| result.passed) {
        Ok(())
    } else {
        anyhow::bail!("ASR eval 未通过");
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
    let total = results.len();
    let passed = results.iter().filter(|result| result.passed).count();
    let accuracy = passed as f32 / total as f32;
    let correction_pair_hit_rate = results
        .iter()
        .filter(|result| result.applied_count > 0)
        .count() as f32
        / total as f32;
    let false_replacement_count = results
        .iter()
        .filter(|result| {
            result.case.category == "false_positive_guard"
                && result.actual_text != result.case.expected_text
        })
        .count();

    println!("# ASR Eval Report");
    println!();
    println!("- cases: {}", total);
    println!("- passed: {}", passed);
    println!("- final_accuracy: {:.2}%", accuracy * 100.0);
    println!(
        "- correction_pair_hit_rate: {:.2}%",
        correction_pair_hit_rate * 100.0
    );
    println!("- false_replacement_count: {}", false_replacement_count);
    println!();
    println!("| ID | Provider | Category | Result | Raw | Actual | Expected |");
    println!("|---|---|---|---|---|---|---|");

    for result in results {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            escape_md(&result.case.audio_id),
            escape_md(&result.case.provider),
            escape_md(&result.case.category),
            if result.passed { "PASS" } else { "FAIL" },
            escape_md(&result.case.raw_asr_text),
            escape_md(&result.actual_text),
            escape_md(&result.case.expected_text),
        );
    }
}

fn escape_md(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}
