use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::PersonalizationRuntimeResult;

const MAX_RUNTIME_DIAGNOSTIC_TEXT_CHARS: usize = 160;
const MAX_RUNTIME_DIAGNOSTIC_CANDIDATES: usize = 20;
const MAX_RUNTIME_DIAGNOSTIC_FILES_PER_DAY: usize = 200;
const SECS_PER_DAY: u64 = 86_400;

#[derive(Debug, Serialize)]
struct PersonalizationRuntimeDiagnosticPayload {
    schema_version: u8,
    stage: &'static str,
    timestamp_ms: u128,
    source_text: String,
    output_text: String,
    changed: bool,
    elapsed_us: u64,
    candidate_count: usize,
    applied_count: usize,
    pass_summaries: Vec<Value>,
    candidates: Vec<Value>,
    applied: Vec<Value>,
}

pub fn write_runtime_diagnostic(
    source_text: &str,
    result: &PersonalizationRuntimeResult,
) -> Result<PathBuf> {
    let diagnostics_dir = runtime_diagnostics_dir()?;
    write_runtime_diagnostic_to_dir(&diagnostics_dir, source_text, result, current_unix_millis())
}

fn runtime_diagnostics_dir() -> Result<PathBuf> {
    let config_path = crate::config::AppConfig::config_path()?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(config_dir
        .join("diagnostics")
        .join(runtime_diagnostic_date_dir_from_unix_secs(now_secs)))
}

fn write_runtime_diagnostic_to_dir(
    output_dir: &Path,
    source_text: &str,
    result: &PersonalizationRuntimeResult,
    timestamp_ms: u128,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;
    let path = output_dir.join(format!(
        "personalization-{}-{}.json",
        timestamp_ms,
        uuid::Uuid::new_v4()
    ));
    let payload = runtime_diagnostic_payload(source_text, result, timestamp_ms);
    let content = serde_json::to_string_pretty(&payload)?;
    std::fs::write(&path, content)?;
    prune_runtime_diagnostics(output_dir, MAX_RUNTIME_DIAGNOSTIC_FILES_PER_DAY)?;
    Ok(path)
}

fn prune_runtime_diagnostics(output_dir: &Path, max_files: usize) -> Result<()> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.starts_with("personalization-")
            || path.extension().and_then(|ext| ext.to_str()) != Some("json")
        {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        let timestamp = runtime_diagnostic_timestamp_from_name(&file_name).unwrap_or_default();
        files.push((timestamp, modified, file_name, path));
    }

    if files.len() <= max_files {
        return Ok(());
    }

    files.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    let stale_count = files.len().saturating_sub(max_files);
    for (_, _, _, path) in files.into_iter().take(stale_count) {
        std::fs::remove_file(path)?;
    }

    Ok(())
}

fn runtime_diagnostic_timestamp_from_name(file_name: &str) -> Option<u128> {
    let without_prefix = file_name.strip_prefix("personalization-")?;
    let (timestamp, _) = without_prefix.split_once('-')?;
    timestamp.parse().ok()
}

fn runtime_diagnostic_payload(
    source_text: &str,
    result: &PersonalizationRuntimeResult,
    timestamp_ms: u128,
) -> PersonalizationRuntimeDiagnosticPayload {
    let conversion = &result.conversion;
    PersonalizationRuntimeDiagnosticPayload {
        schema_version: 1,
        stage: "personalization",
        timestamp_ms,
        source_text: truncate_chars(source_text, MAX_RUNTIME_DIAGNOSTIC_TEXT_CHARS),
        output_text: truncate_chars(&conversion.text, MAX_RUNTIME_DIAGNOSTIC_TEXT_CHARS),
        changed: conversion.changed,
        elapsed_us: result.elapsed_us,
        candidate_count: conversion.diagnostics.candidates.len(),
        applied_count: conversion.diagnostics.applied.len(),
        pass_summaries: conversion
            .diagnostics
            .pass_summaries
            .iter()
            .map(bounded_json)
            .collect(),
        candidates: conversion
            .diagnostics
            .candidates
            .iter()
            .take(MAX_RUNTIME_DIAGNOSTIC_CANDIDATES)
            .map(bounded_json)
            .collect(),
        applied: conversion
            .diagnostics
            .applied
            .iter()
            .take(MAX_RUNTIME_DIAGNOSTIC_CANDIDATES)
            .map(bounded_json)
            .collect(),
    }
}

fn current_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn runtime_diagnostic_date_dir_from_unix_secs(secs: u64) -> String {
    let days = (secs / SECS_PER_DAY) as i64;
    let (year, month, day) = civil_from_unix_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = month_index + if month_index < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);

    (year as i32, month as u32, day as u32)
}

fn bounded_json<T: Serialize>(payload: &T) -> Value {
    let mut value = serde_json::to_value(payload).unwrap_or(Value::Null);
    truncate_json_strings(&mut value, MAX_RUNTIME_DIAGNOSTIC_TEXT_CHARS);
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

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let truncated = value.chars().take(max_chars).collect::<String>();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personalization::{
        apply_personalization_with_store, CorrectionPair, CorrectionPairStore,
    };

    #[test]
    fn runtime_diagnostic_date_dir_uses_utc_day() {
        assert_eq!(runtime_diagnostic_date_dir_from_unix_secs(0), "1970-01-01");
        assert_eq!(
            runtime_diagnostic_date_dir_from_unix_secs(1_704_067_200),
            "2024-01-01"
        );
    }

    #[test]
    fn write_runtime_diagnostic_bounds_payload() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let pairs = (0..25)
            .map(|idx| {
                let mut pair = CorrectionPair::new(
                    format!("claude-{idx}"),
                    "cloud code",
                    format!("Claude Code {idx}"),
                );
                pair.source = "manual".to_string();
                pair.confidence = 0.98;
                pair
            })
            .collect();
        let result = apply_personalization_with_store(
            "cloud code".to_string(),
            CorrectionPairStore::new(pairs),
        );

        let path =
            write_runtime_diagnostic_to_dir(temp.path(), &"cloud code ".repeat(200), &result, 456)
                .expect("write diagnostic");
        let payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read diagnostic"))
                .expect("parse diagnostic");

        assert_eq!(payload["schema_version"], 1);
        assert_eq!(payload["stage"], "personalization");
        assert_eq!(payload["elapsed_us"], result.elapsed_us);
        assert_eq!(payload["timestamp_ms"], 456);
        assert_eq!(
            payload["candidates"].as_array().expect("candidates").len(),
            20
        );
        assert!(payload["source_text"]
            .as_str()
            .expect("source text")
            .ends_with("..."));
        assert!(payload["pass_summaries"]
            .as_array()
            .expect("pass summaries")
            .iter()
            .any(|summary| summary["name"] == "exact_text"));
    }

    #[test]
    fn write_runtime_diagnostic_prunes_old_runtime_files() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let result = apply_personalization_with_store(
            "plain text".to_string(),
            CorrectionPairStore::new(vec![]),
        );

        for idx in 0..(MAX_RUNTIME_DIAGNOSTIC_FILES_PER_DAY + 3) {
            let path = temp
                .path()
                .join(format!("personalization-old-{idx:04}.json"));
            std::fs::write(path, "{}").expect("write old diagnostic");
        }
        let unrelated_path = temp.path().join("other-diagnostic.json");
        std::fs::write(&unrelated_path, "{}").expect("write unrelated diagnostic");

        let new_path = write_runtime_diagnostic_to_dir(temp.path(), "plain text", &result, 999)
            .expect("write diagnostic");

        let personalization_count = std::fs::read_dir(temp.path())
            .expect("read diagnostic dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy();
                file_name.starts_with("personalization-")
                    && entry.path().extension().is_some_and(|ext| ext == "json")
            })
            .count();

        assert_eq!(personalization_count, MAX_RUNTIME_DIAGNOSTIC_FILES_PER_DAY);
        assert!(new_path.exists());
        assert!(unrelated_path.exists());
        assert!(!temp.path().join("personalization-old-0000.json").exists());
    }
}
