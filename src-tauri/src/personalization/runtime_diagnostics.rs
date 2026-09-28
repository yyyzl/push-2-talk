use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::PersonalizationRuntimeResult;

const MAX_RUNTIME_DIAGNOSTIC_TEXT_CHARS: usize = 160;
const MAX_RUNTIME_DIAGNOSTIC_CANDIDATES: usize = 20;
const MAX_RUNTIME_DIAGNOSTIC_FILES: usize = 200;
const MAX_RUNTIME_DIAGNOSTIC_AGE_MS: u128 = 7 * 86_400_000;
const DIAGNOSTICS_ENV: &str = "PUSHTOTALK_PERSONALIZATION_DIAGNOSTICS";

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
) -> Result<Option<PathBuf>> {
    write_runtime_diagnostic_when_enabled(
        std::env::var(DIAGNOSTICS_ENV).as_deref() == Ok("1"),
        runtime_diagnostics_dir,
        source_text,
        result,
    )
}

fn write_runtime_diagnostic_when_enabled(
    enabled: bool,
    directory: impl FnOnce() -> Result<PathBuf>,
    source_text: &str,
    result: &PersonalizationRuntimeResult,
) -> Result<Option<PathBuf>> {
    if !enabled {
        return Ok(None);
    }
    write_runtime_diagnostic_to_dir(&directory()?, source_text, result, current_unix_millis())
        .map(Some)
}

fn runtime_diagnostics_dir() -> Result<PathBuf> {
    let config_path = crate::config::AppConfig::config_path()?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
    // New owned namespace: never prune historical daily diagnostic directories.
    Ok(config_dir
        .join("diagnostics")
        .join("personalization-session"))
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
    prune_runtime_diagnostics(output_dir, MAX_RUNTIME_DIAGNOSTIC_FILES)?;
    Ok(path)
}

fn prune_runtime_diagnostics(output_dir: &Path, max_files: usize) -> Result<()> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
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
        let Some(timestamp) = runtime_diagnostic_timestamp_from_name(&file_name) else {
            continue;
        };
        if current_unix_millis().saturating_sub(timestamp) > MAX_RUNTIME_DIAGNOSTIC_AGE_MS {
            std::fs::remove_file(path)?;
            continue;
        }
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
    fn disabled_diagnostics_never_resolve_or_create_a_directory() {
        let result = apply_personalization_with_store(
            "private text".to_string(),
            CorrectionPairStore::new(vec![]),
        );
        let path = write_runtime_diagnostic_when_enabled(
            false,
            || panic!("disabled diagnostics must not access the config or filesystem"),
            "private text",
            &result,
        )
        .unwrap();
        assert!(path.is_none());
    }

    #[test]
    fn diagnostics_expire_across_days_without_touching_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        let now = current_unix_millis();
        let old = dir
            .path()
            .join(format!("personalization-{}-old.json", now - 8 * 86_400_000));
        let recent = dir
            .path()
            .join(format!("personalization-{now}-recent.json"));
        let unrelated = dir.path().join("notes.json");
        for path in [&old, &recent, &unrelated] {
            std::fs::write(path, "{}").unwrap();
        }
        prune_runtime_diagnostics(dir.path(), 200).unwrap();
        assert!(!old.exists());
        assert!(recent.exists());
        assert!(unrelated.exists());
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

        let now = current_unix_millis();
        let path =
            write_runtime_diagnostic_to_dir(temp.path(), &"cloud code ".repeat(200), &result, now)
                .expect("write diagnostic");
        let payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read diagnostic"))
                .expect("parse diagnostic");

        assert_eq!(payload["schema_version"], 1);
        assert_eq!(payload["stage"], "personalization");
        assert_eq!(payload["elapsed_us"], result.elapsed_us);
        assert_eq!(payload["timestamp_ms"], now as u64);
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

        let now = current_unix_millis();
        for idx in 0..(MAX_RUNTIME_DIAGNOSTIC_FILES + 3) {
            let path = temp.path().join(format!(
                "personalization-{}-{idx:04}.json",
                now - 1000 + idx as u128
            ));
            std::fs::write(path, "{}").expect("write old diagnostic");
        }
        let unrelated_path = temp.path().join("other-diagnostic.json");
        std::fs::write(&unrelated_path, "{}").expect("write unrelated diagnostic");

        let new_path = write_runtime_diagnostic_to_dir(temp.path(), "plain text", &result, now)
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

        assert_eq!(personalization_count, MAX_RUNTIME_DIAGNOSTIC_FILES);
        assert!(new_path.exists());
        assert!(unrelated_path.exists());
        assert!(!temp
            .path()
            .join(format!("personalization-{}-0000.json", now - 1000))
            .exists());
    }
}
