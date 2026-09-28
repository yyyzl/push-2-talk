//! Configuration transactions and the SQLite dictionary projection.
//! Tauri commands and workflows use this service; only the repository writes config.json.
use crate::config::{
    repository::{ConfigRepository, ConfigSnapshot},
    AppConfig,
};
use crate::personalization::{default_user_terms_db_path, UserTermStore};
use std::collections::HashSet;

pub(crate) fn sync_user_terms_sidecar_from_dictionary_or_warn(
    dictionary: &[String],
    lifecycle: &str,
) {
    let _ = sync_user_terms_sidecar_from_dictionary_result_or_warn(
        sync_user_terms_sidecar_from_dictionary(dictionary),
        lifecycle,
    );
}

fn sync_user_terms_sidecar_from_dictionary(dictionary: &[String]) -> anyhow::Result<usize> {
    let path = default_user_terms_db_path()?;
    sync_user_terms_sidecar_from_dictionary_at_path(dictionary, &path)
}

#[cfg(test)]
fn sync_user_terms_sidecar_from_dictionary_at_path_or_warn(
    dictionary: &[String],
    path: &std::path::Path,
    lifecycle: &str,
) -> Option<usize> {
    sync_user_terms_sidecar_from_dictionary_result_or_warn(
        sync_user_terms_sidecar_from_dictionary_at_path(dictionary, path),
        lifecycle,
    )
}

fn sync_user_terms_sidecar_from_dictionary_result_or_warn(
    result: anyhow::Result<usize>,
    lifecycle: &str,
) -> Option<usize> {
    match result {
        Ok(count) => {
            tracing::debug!(
                "同步 user_terms sidecar 完成（{}）：{} 个词条",
                lifecycle,
                count
            );
            Some(count)
        }
        Err(e) => {
            tracing::warn!(
                "同步 user_terms sidecar 失败（{}），继续使用配置词典: {}",
                lifecycle,
                e
            );
            None
        }
    }
}

fn sync_user_terms_sidecar_from_dictionary_at_path(
    dictionary: &[String],
    path: &std::path::Path,
) -> anyhow::Result<usize> {
    let mut normalized_dictionary = dictionary.to_vec();
    crate::dictionary_utils::backfill_inferred_categories(&mut normalized_dictionary);

    let mut store = UserTermStore::open(path)?;
    store.hydrate_dictionary_entries(&normalized_dictionary)
}

pub(crate) fn dictionary_entries_from_user_terms_or_config(
    config_dictionary: &[String],
) -> Vec<String> {
    match default_user_terms_db_path() {
        Ok(path) => dictionary_entries_from_user_terms_or_config_at_path(config_dictionary, &path),
        Err(e) => {
            tracing::warn!(
                "解析 user_terms sidecar 路径失败，使用配置词典读取词库: {}",
                e
            );
            normalize_dictionary_for_config_storage(config_dictionary.to_vec())
        }
    }
}

fn dictionary_entries_from_user_terms_or_config_at_path(
    config_dictionary: &[String],
    path: &std::path::Path,
) -> Vec<String> {
    let normalized_config = normalize_dictionary_for_config_storage(config_dictionary.to_vec());

    let mut store = match UserTermStore::open(path) {
        Ok(store) => store,
        Err(e) => {
            tracing::warn!("读取 user_terms sidecar 失败，回退配置词典: {}", e);
            return normalized_config;
        }
    };

    match store.list_enabled_dictionary_entries() {
        Ok(entries) if !entries.is_empty() => entries,
        Ok(_) if !normalized_config.is_empty() && matches!(store.has_entries(), Ok(false)) => {
            tracing::debug!("user_terms sidecar 为空，从配置词典水合词库");
            if let Err(e) = store.hydrate_dictionary_entries(&normalized_config) {
                tracing::warn!("水合 user_terms sidecar 失败，回退配置词典: {}", e);
                return normalized_config;
            }
            match store.list_enabled_dictionary_entries() {
                Ok(entries) => entries,
                Err(e) => {
                    tracing::warn!("水合后读取 user_terms sidecar 失败，回退配置词典: {}", e);
                    normalized_config
                }
            }
        }
        Ok(_) => Vec::new(),
        Err(e) => {
            tracing::warn!("读取 user_terms sidecar 词条失败，回退配置词典: {}", e);
            normalized_config
        }
    }
}

pub(crate) fn upsert_user_term_sidecar_entry_and_snapshot_config(
    word: &str,
    source: &str,
    category: Option<&str>,
) -> Result<(AppConfig, Vec<String>), String> {
    let path = default_user_terms_db_path()
        .map_err(|e| format!("解析 user_terms sidecar 路径失败: {}", e))?;
    upsert_user_term_sidecar_entry_and_snapshot_config_at_path(word, source, category, &path)
}

fn upsert_user_term_sidecar_entry_and_snapshot_config_at_path(
    word: &str,
    source: &str,
    category: Option<&str>,
    path: &std::path::Path,
) -> Result<(AppConfig, Vec<String>), String> {
    let mut store =
        UserTermStore::open(path).map_err(|e| format!("打开 user_terms sidecar 失败: {}", e))?;
    store
        .upsert_dictionary_entry(word, source, category)
        .map_err(|e| format!("写入 user_terms sidecar 失败: {}", e))?;
    let entries = store
        .list_enabled_dictionary_entries()
        .map_err(|e| format!("读取 user_terms sidecar 失败: {}", e))?;
    snapshot_config_dictionary_from_user_term_entries(entries)
}

pub(crate) fn delete_user_term_sidecar_entries_and_snapshot_config(
    words: &[String],
) -> Result<(AppConfig, Vec<String>), String> {
    let path = default_user_terms_db_path()
        .map_err(|e| format!("解析 user_terms sidecar 路径失败: {}", e))?;
    delete_user_term_sidecar_entries_and_snapshot_config_at_path(words, &path)
}

fn delete_user_term_sidecar_entries_and_snapshot_config_at_path(
    words: &[String],
    path: &std::path::Path,
) -> Result<(AppConfig, Vec<String>), String> {
    let mut store =
        UserTermStore::open(path).map_err(|e| format!("打开 user_terms sidecar 失败: {}", e))?;
    store
        .disable_dictionary_entries(words)
        .map_err(|e| format!("删除 user_terms sidecar 词条失败: {}", e))?;
    let entries = store
        .list_enabled_dictionary_entries()
        .map_err(|e| format!("读取 user_terms sidecar 失败: {}", e))?;
    snapshot_config_dictionary_from_user_term_entries(entries)
}

pub(crate) fn snapshot_config_dictionary_from_user_term_entries(
    entries: Vec<String>,
) -> Result<(AppConfig, Vec<String>), String> {
    let normalized_entries = normalize_dictionary_for_config_storage(entries);
    mutate_persisted_config_with_result(|config| {
        config.dictionary = normalized_entries;
        Ok(config.dictionary.clone())
    })
}

pub(crate) fn runtime_dictionary_entries_from_user_terms_or_input(
    input_dictionary: &[String],
) -> Vec<String> {
    match default_user_terms_db_path() {
        Ok(path) => {
            runtime_dictionary_entries_from_user_terms_or_input_at_path(input_dictionary, &path)
        }
        Err(e) => {
            tracing::warn!(
                "解析 user_terms sidecar 路径失败，使用输入词典启动运行时词库: {}",
                e
            );
            normalized_runtime_dictionary_from_input(input_dictionary)
        }
    }
}

fn runtime_dictionary_entries_from_user_terms_or_input_at_path(
    input_dictionary: &[String],
    path: &std::path::Path,
) -> Vec<String> {
    let normalized_input = normalized_runtime_dictionary_from_input(input_dictionary);
    match UserTermStore::open(path).and_then(|store| {
        Ok((
            store.list_enabled_dictionary_entries()?,
            store.has_entries()?,
        ))
    }) {
        Ok((sidecar_entries, true)) => {
            tracing::debug!(
                "从 user_terms sidecar 合并运行时词库：{} 个用户词条，{} 个输入词条",
                sidecar_entries.len(),
                normalized_input.len()
            );
            merge_sidecar_user_terms_with_runtime_dictionary(sidecar_entries, normalized_input)
        }
        Ok(_) => {
            tracing::warn!("user_terms sidecar 没有启用词条，使用输入词典启动运行时词库");
            normalized_input
        }
        Err(e) => {
            tracing::warn!(
                "读取 user_terms sidecar 失败，使用输入词典启动运行时词库: {}",
                e
            );
            normalized_input
        }
    }
}

fn merge_sidecar_user_terms_with_runtime_dictionary(
    sidecar_entries: Vec<String>,
    runtime_entries: Vec<String>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut merged = Vec::new();

    let dynamic_entries = runtime_entries.into_iter().filter(|entry| {
        entry
            .split('|')
            .nth(1)
            .is_some_and(is_runtime_only_dictionary_source)
    });
    for entry in sidecar_entries.into_iter().chain(dynamic_entries) {
        let key = runtime_dictionary_entry_key(&entry);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        merged.push(entry);
    }

    merged
}

fn runtime_dictionary_entry_key(entry: &str) -> String {
    crate::dictionary_utils::extract_word(entry)
        .trim()
        .to_lowercase()
}

fn normalized_runtime_dictionary_from_input(input_dictionary: &[String]) -> Vec<String> {
    input_dictionary
        .iter()
        .filter_map(|entry| normalize_runtime_dictionary_entry(entry))
        .collect()
}

fn normalize_runtime_dictionary_entry(entry: &str) -> Option<String> {
    let word =
        crate::dictionary_utils::normalize_word(crate::dictionary_utils::extract_word(entry));
    if word.is_empty() {
        return None;
    }

    let mut parts = entry.split('|');
    let _ = parts.next();
    let source = parts
        .next()
        .map(str::trim)
        .filter(|source| !source.is_empty());
    let category = parts
        .next()
        .map(str::trim)
        .filter(|category| !category.is_empty());

    if let Some(source) = source.filter(|source| is_runtime_only_dictionary_source(source)) {
        let category = crate::dictionary_utils::normalize_category(category)
            .unwrap_or_else(|| crate::dictionary_utils::infer_dictionary_category(&word));
        return Some(format!("{}|{}|{}", word, source, category));
    }

    let source = match source {
        Some("auto") => "auto",
        _ => "manual",
    };
    let category = crate::dictionary_utils::normalize_or_infer_category(&word, category);
    Some(crate::dictionary_utils::format_entry_with_category(
        &word,
        source,
        Some(category),
    ))
}

fn is_runtime_only_dictionary_source(source: &str) -> bool {
    matches!(source, "domain" | "recent" | "builtin" | "app_context")
}

fn repository() -> Result<&'static ConfigRepository, String> {
    static REPOSITORY: std::sync::OnceLock<ConfigRepository> = std::sync::OnceLock::new();
    let path = AppConfig::config_path().map_err(|e| format!("解析配置路径失败: {e}"))?;
    Ok(REPOSITORY.get_or_init(|| ConfigRepository::new(path)))
}

fn prepare_dictionary(config: &mut AppConfig) {
    config.dictionary = dictionary_entries_from_user_terms_or_config(&config.dictionary);
}

pub(crate) fn load_config_snapshot() -> Result<ConfigSnapshot, String> {
    repository()?.read(prepare_dictionary)
}

pub(crate) fn load_persisted_config() -> Result<AppConfig, String> {
    load_config_snapshot().map(|snapshot| snapshot.config)
}

pub(crate) fn update_config_snapshot(patch: &serde_json::Value) -> Result<ConfigSnapshot, String> {
    repository()?
        .update(prepare_dictionary, |config| {
            crate::config::patch::apply_patch(config, patch)
        })
        .map(|(snapshot, ())| snapshot)
}

pub(crate) fn mutate_persisted_config_with_result<R, F>(
    mutator: F,
) -> Result<(AppConfig, R), String>
where
    F: FnOnce(&mut AppConfig) -> Result<R, String>,
{
    repository()?
        .update(prepare_dictionary, mutator)
        .map(|(snapshot, result)| (snapshot.config, result))
}

pub(crate) fn mutate_persisted_config<F>(mutator: F) -> Result<AppConfig, String>
where
    F: FnOnce(&mut AppConfig) -> Result<(), String>,
{
    mutate_persisted_config_with_result(|config| {
        mutator(config)?;
        Ok(())
    })
    .map(|(config, _)| config)
}

#[cfg(test)]
mod user_terms_sidecar_sync_tests {
    use super::*;

    #[test]
    fn sync_user_terms_sidecar_from_dictionary_at_path_hydrates_terms() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("personalization").join("user_terms.db");
        let dictionary = vec![
            "useState|auto".to_string(),
            "Claude Code|manual|product".to_string(),
            "  ".to_string(),
        ];

        let synced = sync_user_terms_sidecar_from_dictionary_at_path_or_warn(
            &dictionary,
            &path,
            "test success",
        );

        assert_eq!(synced, Some(2));

        let store = crate::personalization::UserTermStore::open(&path).expect("open synced store");
        let use_state = store
            .find_by_term("useState")
            .expect("find useState")
            .unwrap();
        assert_eq!(use_state.source, "auto");
        assert_eq!(use_state.category, "code_symbol");

        let claude_code = store
            .find_by_term("claude code")
            .expect("find Claude Code")
            .unwrap();
        assert_eq!(claude_code.source, "manual");
        assert_eq!(claude_code.category, "product");
    }

    #[test]
    fn sync_user_terms_sidecar_from_dictionary_at_path_or_warn_ignores_open_error() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("db_directory");
        std::fs::create_dir(&path).expect("create directory at db path");
        let dictionary = vec!["Claude Code|manual|product".to_string()];

        let synced = sync_user_terms_sidecar_from_dictionary_at_path_or_warn(
            &dictionary,
            &path,
            "test failure",
        );

        assert_eq!(synced, None);
    }
}

#[cfg(test)]
mod runtime_user_terms_dictionary_tests {
    use super::*;

    #[test]
    fn runtime_dictionary_merges_enabled_sidecar_entries_with_dynamic_runtime_entries() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("personalization").join("user_terms.db");
        let mut store = crate::personalization::UserTermStore::open(&path).expect("open store");
        store
            .hydrate_dictionary_entries(&[
                "Claude Code|manual|product".to_string(),
                "useState|auto".to_string(),
                "禁用短语|manual|phrase".to_string(),
            ])
            .expect("hydrate first snapshot");
        store
            .hydrate_dictionary_entries(&[
                "Claude Code|manual|product".to_string(),
                "useState|auto".to_string(),
            ])
            .expect("hydrate second snapshot");

        let entries = runtime_dictionary_entries_from_user_terms_or_input_at_path(
            &[
                "Claude Code|recent|generic".to_string(),
                "useState|recent|generic".to_string(),
                "领域术语|domain|domain_term".to_string(),
                "最近工具|recent|generic".to_string(),
            ],
            &path,
        );

        assert_eq!(
            entries,
            vec![
                "Claude Code|manual|product".to_string(),
                "useState|auto|code_symbol".to_string(),
                "领域术语|domain|domain_term".to_string(),
                "最近工具|recent|generic".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_dictionary_falls_back_to_config_when_sidecar_read_fails() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("db_directory");
        std::fs::create_dir(&path).expect("create directory at db path");

        let entries = runtime_dictionary_entries_from_user_terms_or_input_at_path(
            &[
                "useState|auto".to_string(),
                "Claude Code|manual|product".to_string(),
                "最近工具|recent|generic".to_string(),
                "领域术语|domain|domain_term".to_string(),
            ],
            &path,
        );

        assert_eq!(
            entries,
            vec![
                "useState|auto|code_symbol".to_string(),
                "Claude Code|manual|product".to_string(),
                "最近工具|recent|generic".to_string(),
                "领域术语|domain|domain_term".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_dictionary_falls_back_to_config_when_sidecar_is_empty() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("personalization").join("user_terms.db");

        let entries = runtime_dictionary_entries_from_user_terms_or_input_at_path(
            &[
                "Claude Code|manual|product".to_string(),
                "最近工具|recent|generic".to_string(),
            ],
            &path,
        );

        assert_eq!(
            entries,
            vec![
                "Claude Code|manual|product".to_string(),
                "最近工具|recent|generic".to_string(),
            ]
        );
    }
}

#[cfg(test)]
mod dictionary_sidecar_persistence_tests {
    use super::*;

    #[test]
    fn deleted_dictionary_stays_empty_when_a_stale_config_snapshot_is_loaded() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("user_terms.db");
        let stale = vec!["Deleted Term|manual|phrase".to_string()];
        let mut store = UserTermStore::open(&path).unwrap();
        store.hydrate_dictionary_entries(&stale).unwrap();
        store
            .disable_dictionary_entries(&["Deleted Term".to_string()])
            .unwrap();
        drop(store);
        assert!(dictionary_entries_from_user_terms_or_config_at_path(&stale, &path).is_empty());
        assert_eq!(
            runtime_dictionary_entries_from_user_terms_or_input_at_path(
                &[stale[0].clone(), "Runtime Word|recent|phrase".to_string()],
                &path,
            ),
            vec!["Runtime Word|recent|phrase".to_string()]
        );
    }

    #[test]
    fn dictionary_entries_bootstrap_empty_sidecar_from_config_snapshot() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("personalization").join("user_terms.db");

        let entries = dictionary_entries_from_user_terms_or_config_at_path(
            &[
                "Claude Code|manual|product".to_string(),
                "useState|auto".to_string(),
            ],
            &path,
        );

        assert_eq!(
            entries,
            vec![
                "Claude Code|manual|product".to_string(),
                "useState|auto|code_symbol".to_string(),
            ]
        );

        let store = UserTermStore::open(&path).expect("open hydrated store");
        assert_eq!(
            store
                .list_enabled_dictionary_entries()
                .expect("list hydrated entries"),
            entries
        );
    }

    #[test]
    fn dictionary_entries_prefer_existing_sidecar_over_config_snapshot() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("personalization").join("user_terms.db");
        let mut store = UserTermStore::open(&path).expect("open store");
        store
            .upsert_dictionary_entry("Claude Code", "manual", Some("product"))
            .expect("upsert sidecar term");

        let entries = dictionary_entries_from_user_terms_or_config_at_path(
            &["Old Config Term|manual|phrase".to_string()],
            &path,
        );

        assert_eq!(entries, vec!["Claude Code|manual|product".to_string()]);
    }

    #[test]
    fn dictionary_entries_fall_back_to_config_when_sidecar_open_fails() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("db_directory");
        std::fs::create_dir(&path).expect("create directory at db path");

        let entries = dictionary_entries_from_user_terms_or_config_at_path(
            &[
                "useState|auto".to_string(),
                "rust|manual|generic".to_string(),
            ],
            &path,
        );

        assert_eq!(
            entries,
            vec!["useState|auto|code_symbol".to_string(), "rust".to_string()]
        );
    }
}

pub(crate) fn normalize_dictionary_for_config_storage(mut dictionary: Vec<String>) -> Vec<String> {
    crate::dictionary_utils::backfill_inferred_categories(&mut dictionary);
    dictionary
}
