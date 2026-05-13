use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::phonetic_keys::{build_key_bundle, normalize_surface};

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

    pub fn normalized_original(&self) -> String {
        normalize_surface(&self.original_text)
    }

    pub fn is_manual(&self) -> bool {
        self.source.eq_ignore_ascii_case("manual")
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

    #[allow(dead_code)]
    pub fn save_json(&self, path: impl AsRef<Path>) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.pairs)?;
        fs::write(path, content)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn add(&mut self, mut pair: CorrectionPair) {
        pair.ensure_keys();
        self.pairs.retain(|existing| existing.id != pair.id);
        self.pairs.push(pair);
    }

    pub fn lookup_by_text(&self, original: &str) -> Vec<&CorrectionPair> {
        let normalized = normalize_surface(original);
        self.pairs
            .iter()
            .filter(|pair| pair.enabled && pair.normalized_original() == normalized)
            .collect()
    }

    pub fn lookup_by_en_phonetic(&self, key: &str) -> Vec<&CorrectionPair> {
        self.lookup_by_key(|pair| pair.en_phonetic_key.as_deref(), key)
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

fn push_unique(values: &mut Vec<String>, value: String) {
    if value.is_empty() || values.iter().any(|existing| existing == &value) {
        return;
    }
    values.push(value);
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
}
