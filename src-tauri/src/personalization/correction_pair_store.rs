use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::config::AppConfig;

use super::phonetic_keys::{build_key_bundle, normalize_surface};

const REJECT_CONFIDENCE_DELTA: f32 = 0.20;
const DISABLE_AFTER_REJECTS: u32 = 3;
const PERSONALIZATION_RISKY_SINGLE_WORDS: &[&str] = &["cloud"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionPair {
    pub id: String,
    pub original_text: String,
    pub corrected_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub en_phonetic_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zh_pinyin_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zh_pinyin_fuzzy_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixed_key: Option<String>,
    #[serde(default)]
    pub alias_keys: Vec<String>,
    #[serde(default)]
    pub length_chars: usize,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default = "default_frequency")]
    pub frequency: u32,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default)]
    pub accepted_count: u32,
    #[serde(default)]
    pub rejected_count: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_source() -> String {
    "learned".to_string()
}

fn default_frequency() -> u32 {
    1
}

fn default_confidence() -> f32 {
    0.5
}

fn default_enabled() -> bool {
    true
}

impl CorrectionPair {
    pub fn new(
        id: impl Into<String>,
        original_text: impl Into<String>,
        corrected_text: impl Into<String>,
    ) -> Self {
        let mut pair = Self {
            id: id.into(),
            original_text: original_text.into(),
            corrected_text: corrected_text.into(),
            en_phonetic_key: None,
            zh_pinyin_key: None,
            zh_pinyin_fuzzy_key: None,
            mixed_key: None,
            alias_keys: Vec::new(),
            length_chars: 0,
            source: default_source(),
            category: None,
            frequency: default_frequency(),
            confidence: default_confidence(),
            accepted_count: 0,
            rejected_count: 0,
            enabled: true,
        };
        pair.ensure_keys();
        pair
    }

    pub fn ensure_keys(&mut self) {
        let keys = build_key_bundle(&self.original_text);

        if self.en_phonetic_key.is_none() {
            self.en_phonetic_key = keys.en_phonetic_keys.first().cloned();
        }
        if self.zh_pinyin_key.is_none() {
            self.zh_pinyin_key = keys.zh_pinyin_key;
        }
        if self.zh_pinyin_fuzzy_key.is_none() {
            self.zh_pinyin_fuzzy_key = keys.zh_pinyin_fuzzy_key;
        }
        if self.mixed_key.is_none() {
            self.mixed_key = keys.mixed_keys.first().cloned();
        }

        let corrected_keys = build_key_bundle(&self.corrected_text);
        for key in corrected_keys.alias_keys {
            push_unique(&mut self.alias_keys, key);
        }
        for key in corrected_keys.mixed_keys {
            push_unique(&mut self.alias_keys, key);
        }

        self.length_chars = self.original_text.chars().count();
    }

    fn refresh_keys_for_text_update(&mut self, original_text: &str, corrected_text: &str) {
        let stale_aliases = generated_alias_keys_for_corrected_text(&self.corrected_text);
        self.original_text = original_text.to_string();
        self.corrected_text = corrected_text.to_string();
        self.en_phonetic_key = None;
        self.zh_pinyin_key = None;
        self.zh_pinyin_fuzzy_key = None;
        self.mixed_key = None;
        self.alias_keys.retain(|alias| {
            !stale_aliases
                .iter()
                .any(|stale_alias| stale_alias.eq_ignore_ascii_case(alias))
        });
        self.ensure_keys();
    }

    pub fn normalized_original(&self) -> String {
        normalize_surface(&self.original_text)
    }

    pub fn is_manual(&self) -> bool {
        self.source.eq_ignore_ascii_case("manual")
    }

    pub fn is_user_confirmed(&self) -> bool {
        self.is_manual() || self.accepted_count > 0
    }

    pub fn requires_manual_for_auto_apply(&self) -> bool {
        !self.is_manual() && is_single_common_english_word(&self.original_text)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CorrectionPairStore {
    pairs: Vec<CorrectionPair>,
}

impl CorrectionPairStore {
    pub fn new(mut pairs: Vec<CorrectionPair>) -> Self {
        for pair in &mut pairs {
            pair.ensure_keys();
        }
        Self { pairs }
    }

    pub fn load_json(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let pairs: Vec<CorrectionPair> = serde_json::from_str(&content)?;
        Ok(Self::new(pairs))
    }

    pub fn load_json_or_default(path: impl AsRef<Path>) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(content) => {
                let pairs: Vec<CorrectionPair> = serde_json::from_str(&content)?;
                Ok(Self::new(pairs))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    #[allow(dead_code)]
    pub fn save_json(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.pairs)?;
        write_text_atomically(path.as_ref(), &content)
    }

    #[allow(dead_code)]
    pub fn add(&mut self, mut pair: CorrectionPair) {
        pair.ensure_keys();
        self.pairs.retain(|existing| existing.id != pair.id);
        self.pairs.push(pair);
    }

    pub fn upsert_accepted_correction_json(
        path: impl AsRef<Path>,
        original_text: &str,
        corrected_text: &str,
        category: Option<&str>,
    ) -> Result<Option<CorrectionPair>> {
        let mut store = Self::load_json_or_default(&path)?;
        let pair = store.upsert_accepted_correction(original_text, corrected_text, category);
        if pair.is_some() {
            store.save_json(path)?;
        }
        Ok(pair)
    }

    pub fn upsert_accepted_correction(
        &mut self,
        original_text: &str,
        corrected_text: &str,
        category: Option<&str>,
    ) -> Option<CorrectionPair> {
        let original_text = original_text.trim();
        let corrected_text = corrected_text.trim();
        if original_text.is_empty()
            || corrected_text.is_empty()
            || normalize_surface(original_text) == normalize_surface(corrected_text)
        {
            return None;
        }

        let id = learned_pair_id(original_text, corrected_text);
        let normalized_original = normalize_surface(original_text);
        let category = category
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        if let Some(existing) = self.pairs.iter_mut().find(|pair| {
            pair.id == id || normalize_surface(&pair.original_text) == normalized_original
        }) {
            let existing_is_manual = existing.is_manual();
            if existing_is_manual
                && normalize_surface(&existing.corrected_text) != normalize_surface(corrected_text)
            {
                return None;
            }
            if !existing_is_manual {
                existing.id = id;
                existing.source = "learned".to_string();
                existing.rejected_count = 0;
            }
            existing.refresh_keys_for_text_update(original_text, corrected_text);
            existing.category = category;
            existing.frequency = existing.frequency.saturating_add(1).max(1);
            existing.accepted_count = existing.accepted_count.saturating_add(1);
            existing.confidence = existing.confidence.max(0.98);
            existing.enabled = true;
            return Some(existing.clone());
        }

        let mut pair = CorrectionPair::new(id, original_text, corrected_text);
        pair.source = "learned".to_string();
        pair.category = category;
        pair.frequency = 1;
        pair.confidence = 0.98;
        pair.accepted_count = 1;
        pair.enabled = true;
        pair.ensure_keys();
        self.pairs.push(pair.clone());
        Some(pair)
    }

    pub fn record_rejected_correction_json(
        path: impl AsRef<Path>,
        original_text: &str,
        corrected_text: &str,
    ) -> Result<Option<CorrectionPair>> {
        if !path.as_ref().exists() {
            return Ok(None);
        }

        let mut store = Self::load_json(&path)?;
        let pair = store.record_rejected_correction(original_text, corrected_text);
        if pair.is_some() {
            store.save_json(path)?;
        }
        Ok(pair)
    }

    pub fn record_rejected_correction(
        &mut self,
        original_text: &str,
        corrected_text: &str,
    ) -> Option<CorrectionPair> {
        let normalized_original = normalize_surface(original_text);
        let normalized_corrected = normalize_surface(corrected_text);
        if normalized_original.is_empty()
            || normalized_corrected.is_empty()
            || normalized_original == normalized_corrected
        {
            return None;
        }

        let pair = self.pairs.iter_mut().find(|pair| {
            pair.enabled
                && !pair.is_manual()
                && normalize_surface(&pair.original_text) == normalized_original
                && normalize_surface(&pair.corrected_text) == normalized_corrected
        })?;

        pair.rejected_count = pair.rejected_count.saturating_add(1);
        pair.confidence = (pair.confidence - REJECT_CONFIDENCE_DELTA).max(0.0);
        if pair.rejected_count >= DISABLE_AFTER_REJECTS {
            pair.enabled = false;
        }

        Some(pair.clone())
    }

    pub fn lookup_by_text(&self, original: &str) -> Vec<&CorrectionPair> {
        let normalized = normalize_surface(original);
        self.pairs
            .iter()
            .filter(|pair| pair.enabled && pair.normalized_original() == normalized)
            .collect()
    }

    pub fn lookup_by_en_phonetic(&self, key: &str) -> Vec<&CorrectionPair> {
        self.pairs
            .iter()
            .filter(|pair| {
                pair.enabled
                    && pair.original_text.is_ascii()
                    && pair
                        .en_phonetic_key
                        .as_deref()
                        .map(|candidate| candidate.eq_ignore_ascii_case(key))
                        .unwrap_or(false)
            })
            .collect()
    }

    pub fn lookup_by_zh_pinyin_fuzzy(&self, key: &str) -> Vec<&CorrectionPair> {
        self.lookup_by_key(|pair| pair.zh_pinyin_fuzzy_key.as_deref(), key)
    }

    pub fn lookup_by_mixed(&self, key: &str) -> Vec<&CorrectionPair> {
        self.lookup_by_key(|pair| pair.mixed_key.as_deref(), key)
    }

    pub fn lookup_by_alias_key(&self, key: &str) -> Vec<&CorrectionPair> {
        self.pairs
            .iter()
            .filter(|pair| {
                pair.enabled
                    && pair
                        .alias_keys
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(key))
            })
            .collect()
    }

    fn lookup_by_key(
        &self,
        key_selector: impl Fn(&CorrectionPair) -> Option<&str>,
        key: &str,
    ) -> Vec<&CorrectionPair> {
        self.pairs
            .iter()
            .filter(|pair| {
                pair.enabled
                    && key_selector(pair)
                        .map(|candidate| candidate.eq_ignore_ascii_case(key))
                        .unwrap_or(false)
            })
            .collect()
    }
}

pub fn default_correction_pairs_path() -> Result<PathBuf> {
    let config_path = AppConfig::config_path()?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
    Ok(config_dir
        .join("personalization")
        .join("correction_pairs.json"))
}

pub fn record_accepted_correction_pair(
    original_text: Option<&str>,
    corrected_text: Option<&str>,
    category: Option<&str>,
) -> Result<Option<CorrectionPair>> {
    let (Some(original_text), Some(corrected_text)) = (original_text, corrected_text) else {
        return Ok(None);
    };

    let path = default_correction_pairs_path()?;
    CorrectionPairStore::upsert_accepted_correction_json(
        path,
        original_text,
        corrected_text,
        category,
    )
}

pub fn record_rejected_correction_pair(
    original_text: Option<&str>,
    corrected_text: Option<&str>,
) -> Result<Option<CorrectionPair>> {
    let (Some(original_text), Some(corrected_text)) = (original_text, corrected_text) else {
        return Ok(None);
    };

    let path = default_correction_pairs_path()?;
    CorrectionPairStore::record_rejected_correction_json(path, original_text, corrected_text)
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if value.is_empty() || values.iter().any(|existing| existing == &value) {
        return;
    }
    values.push(value);
}

fn write_text_atomically(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = path.with_extension("json.tmp");
    let backup_path = path.with_extension("json.bak");
    fs::write(&temp_path, content)?;

    if path.exists() {
        if backup_path.exists() {
            fs::remove_file(&backup_path)?;
        }
        fs::rename(path, &backup_path)?;
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => {
            if backup_path.exists() {
                fs::remove_file(&backup_path)?;
            }
            Ok(())
        }
        Err(e) => {
            if backup_path.exists() && !path.exists() {
                let _ = fs::rename(&backup_path, path);
            }
            Err(e.into())
        }
    }
}

fn learned_pair_id(original_text: &str, corrected_text: &str) -> String {
    let fingerprint = format!(
        "{}=>{}",
        normalize_surface(original_text),
        normalize_surface(corrected_text)
    );
    format!("learned-{:x}", md5::compute(fingerprint))
}

fn generated_alias_keys_for_corrected_text(corrected_text: &str) -> Vec<String> {
    let corrected_keys = build_key_bundle(corrected_text);
    corrected_keys
        .alias_keys
        .into_iter()
        .chain(corrected_keys.mixed_keys)
        .collect()
}

fn is_single_common_english_word(text: &str) -> bool {
    let normalized = normalize_surface(text);
    let mut words = normalized.split_whitespace();
    let Some(word) = words.next() else {
        return false;
    };
    if words.next().is_some() || !word.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return false;
    }

    crate::tnl::is_common_english_word(word) || PERSONALIZATION_RISKY_SINGLE_WORDS.contains(&word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_phonetic_keys_for_minimal_json_pair() {
        let store = CorrectionPairStore::new(vec![CorrectionPair {
            id: "claude".to_string(),
            original_text: "cloud code".to_string(),
            corrected_text: "Claude Code".to_string(),
            en_phonetic_key: None,
            zh_pinyin_key: None,
            zh_pinyin_fuzzy_key: None,
            mixed_key: None,
            alias_keys: vec!["kelaode|code".to_string()],
            length_chars: 0,
            source: "manual".to_string(),
            category: Some("product".to_string()),
            frequency: 1,
            confidence: 0.98,
            accepted_count: 0,
            rejected_count: 0,
            enabled: true,
        }]);

        let pair = store.lookup_by_text("Cloud Code")[0];
        let key = pair.en_phonetic_key.as_deref().expect("phonetic key");
        assert!(!key.is_empty());
        assert_eq!(store.lookup_by_en_phonetic(key).len(), 1);
        assert_eq!(store.lookup_by_alias_key("kelaode|code").len(), 1);
    }

    #[test]
    fn accepted_correction_pair_persists_and_reloads() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp
            .path()
            .join("personalization")
            .join("correction_pairs.json");

        let pair = CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save pair")
        .expect("pair should be stored");

        assert_eq!(pair.source, "learned");
        assert_eq!(pair.accepted_count, 1);
        assert!(pair.confidence >= 0.98);
        assert!(path.exists());

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        let loaded = store.lookup_by_text("Cloud Code")[0];
        assert_eq!(loaded.corrected_text, "Claude Code");
        assert_eq!(loaded.category.as_deref(), Some("proper_noun"));
    }

    #[test]
    fn save_json_replaces_file_and_cleans_stale_backup() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");
        let backup_path = path.with_extension("json.bak");
        let temp_path = path.with_extension("json.tmp");
        std::fs::write(&path, "[]").expect("write existing file");
        std::fs::write(&backup_path, "stale backup").expect("write stale backup");

        let store = CorrectionPairStore::new(vec![CorrectionPair::new(
            "claude-code",
            "cloud code",
            "Claude Code",
        )]);

        store.save_json(&path).expect("save json");

        assert!(path.exists());
        assert!(!backup_path.exists());
        assert!(!temp_path.exists());
        let reloaded = CorrectionPairStore::load_json(&path).expect("reload");
        assert_eq!(reloaded.lookup_by_text("cloud code").len(), 1);
    }

    #[test]
    fn accepted_mixed_language_pair_is_confident_enough_to_apply_after_reload() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "克劳德 code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save pair");

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        let engine = crate::personalization::PersonalizationEngine::new(store);

        assert_eq!(
            engine.convert("我打开 克劳德 code").text,
            "我打开 Claude Code"
        );
    }

    #[test]
    fn accepted_cloud_code_pair_generates_cross_language_alias_after_reload() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save pair");

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        assert_eq!(store.lookup_by_alias_key("kelaode|code").len(), 1);
        assert_eq!(store.lookup_by_alias_key("kelaode|KT").len(), 1);

        let engine = crate::personalization::PersonalizationEngine::new(store);
        assert_eq!(
            engine.convert("我打开 克劳德 code").text,
            "我打开 Claude Code"
        );
    }

    #[test]
    fn accepted_correction_update_removes_stale_generated_aliases() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save initial pair");
        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Cloud IDE",
            Some("proper_noun"),
        )
        .expect("update pair");

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        assert_eq!(store.lookup_by_text("cloud code").len(), 1);
        let updated = store.lookup_by_text("cloud code")[0];
        assert_eq!(updated.corrected_text, "Cloud IDE");
        assert_eq!(updated.id, learned_pair_id("cloud code", "Cloud IDE"));
        assert!(store.lookup_by_alias_key("kelaode|code").is_empty());
        assert!(store.lookup_by_alias_key("kelaode|KT").is_empty());

        let engine = crate::personalization::PersonalizationEngine::new(store);
        assert_eq!(engine.convert("我打开 cloud code").text, "我打开 Cloud IDE");
        assert_eq!(
            engine.convert("我打开 克劳德 code").text,
            "我打开 克劳德 code"
        );
    }

    #[test]
    fn accepted_correction_does_not_overwrite_conflicting_manual_pair() {
        let mut manual_pair = CorrectionPair::new("manual-claude", "cloud code", "Claude Code");
        manual_pair.source = "manual".to_string();
        manual_pair.confidence = 1.0;
        manual_pair.alias_keys.push("kelaode|code".to_string());
        let mut store = CorrectionPairStore::new(vec![manual_pair]);

        let updated =
            store.upsert_accepted_correction("cloud code", "Cloud IDE", Some("proper_noun"));

        assert!(updated.is_none());
        let pair = store.lookup_by_text("cloud code")[0];
        assert_eq!(pair.id, "manual-claude");
        assert_eq!(pair.source, "manual");
        assert_eq!(pair.corrected_text, "Claude Code");
        assert_eq!(pair.accepted_count, 0);
        assert_eq!(store.lookup_by_alias_key("kelaode|code").len(), 1);

        let engine = crate::personalization::PersonalizationEngine::new(store);
        assert_eq!(
            engine.convert("我打开 cloud code").text,
            "我打开 Claude Code"
        );
    }

    #[test]
    fn accepted_correction_pair_ignores_empty_or_identity_text() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        let empty = CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            " ",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("empty original should not fail");
        let identity = CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "Claude Code",
            " claude code ",
            Some("proper_noun"),
        )
        .expect("identity text should not fail");

        assert!(empty.is_none());
        assert!(identity.is_none());
        assert!(!path.exists());
    }

    #[test]
    fn rejected_correction_pair_lowers_confidence_without_creating_new_pair() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save accepted pair");

        let rejected = CorrectionPairStore::record_rejected_correction_json(
            &path,
            "Cloud Code",
            "Claude Code",
        )
        .expect("record reject")
        .expect("existing pair should be updated");

        assert_eq!(rejected.rejected_count, 1);
        assert!(rejected.confidence < 0.88);
        assert!(rejected.enabled);

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        let pair = store.lookup_by_text("cloud code")[0];
        assert_eq!(pair.rejected_count, 1);
        assert!(pair.confidence < 0.88);

        let missing_path = temp.path().join("missing.json");
        let missing = CorrectionPairStore::record_rejected_correction_json(
            &missing_path,
            "claud code",
            "Claude Code",
        )
        .expect("missing pair should not fail");
        assert!(missing.is_none());
        assert!(!missing_path.exists());
    }

    #[test]
    fn rejected_learned_pair_no_longer_auto_applies_exact_text() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save accepted pair");
        CorrectionPairStore::record_rejected_correction_json(&path, "cloud code", "Claude Code")
            .expect("record reject");

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        let engine = crate::personalization::PersonalizationEngine::new(store);

        assert_eq!(
            engine.convert("我打开 cloud code").text,
            "我打开 cloud code"
        );
    }

    #[test]
    fn repeated_rejections_disable_pair_and_exclude_lookup() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save accepted pair");

        for _ in 0..3 {
            CorrectionPairStore::record_rejected_correction_json(
                &path,
                "cloud code",
                "Claude Code",
            )
            .expect("record reject")
            .expect("pair should exist");
        }

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        assert!(store.lookup_by_text("cloud code").is_empty());

        let raw_pairs: Vec<CorrectionPair> =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read pairs"))
                .expect("parse pairs");
        assert_eq!(raw_pairs[0].rejected_count, 3);
        assert!(!raw_pairs[0].enabled);
    }

    #[test]
    fn accepted_correction_clears_prior_reject_streak() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let path = temp.path().join("correction_pairs.json");

        CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("save accepted pair");
        for _ in 0..3 {
            CorrectionPairStore::record_rejected_correction_json(
                &path,
                "cloud code",
                "Claude Code",
            )
            .expect("record reject")
            .expect("pair should exist");
        }

        let accepted = CorrectionPairStore::upsert_accepted_correction_json(
            &path,
            "cloud code",
            "Claude Code",
            Some("proper_noun"),
        )
        .expect("accept again")
        .expect("pair should be re-accepted");

        assert!(accepted.enabled);
        assert_eq!(accepted.rejected_count, 0);
        assert!(accepted.confidence >= 0.98);

        let store = CorrectionPairStore::load_json(&path).expect("reload store");
        let pair = store.lookup_by_text("cloud code")[0];
        assert_eq!(pair.rejected_count, 0);
        assert!(pair.enabled);

        let engine = crate::personalization::PersonalizationEngine::new(store);
        assert_eq!(
            engine.convert("我打开 cloud code").text,
            "我打开 Claude Code"
        );
    }

    #[test]
    fn reject_feedback_does_not_weaken_manual_pair() {
        let mut manual_pair = CorrectionPair::new("manual-claude", "cloud code", "Claude Code");
        manual_pair.source = "manual".to_string();
        manual_pair.confidence = 1.0;
        let mut store = CorrectionPairStore::new(vec![manual_pair]);

        for _ in 0..3 {
            let rejected = store.record_rejected_correction("cloud code", "Claude Code");
            assert!(rejected.is_none());
        }

        let pair = store.lookup_by_text("cloud code")[0];
        assert_eq!(pair.id, "manual-claude");
        assert_eq!(pair.source, "manual");
        assert_eq!(pair.rejected_count, 0);
        assert_eq!(pair.confidence, 1.0);
        assert!(pair.enabled);

        let engine = crate::personalization::PersonalizationEngine::new(store);
        assert_eq!(
            engine.convert("我打开 cloud code").text,
            "我打开 Claude Code"
        );
    }

    #[test]
    fn mixed_language_pair_does_not_match_ascii_tail_by_english_phonetic_key() {
        let mixed_pair = CorrectionPair::new("openai-mixed", "欧喷 ai", "OpenAI");
        let ascii_pair = CorrectionPair::new("openai-ascii", "open ai", "OpenAI");
        let store = CorrectionPairStore::new(vec![mixed_pair, ascii_pair]);
        let ai_key = build_key_bundle("ai")
            .en_phonetic_keys
            .first()
            .cloned()
            .expect("ai phonetic key");

        let matches = store.lookup_by_en_phonetic(&ai_key);

        assert!(matches.iter().all(|pair| pair.original_text.is_ascii()));
        assert!(!matches.iter().any(|pair| pair.id == "openai-mixed"));
    }
}
