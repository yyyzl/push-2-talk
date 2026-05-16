use crate::dictionary_utils::extract_word;
use crate::personalization::correction_pair_store::CorrectionPair;
use serde_json::Value;
use std::collections::HashMap;

pub const QWEN_HTTP_MAX_HOTWORDS: usize = 50;
pub const QWEN_REALTIME_MAX_HOTWORDS: usize = 50;
pub const DOUBAO_HTTP_MAX_HOTWORDS: usize = 100;
pub const DOUBAO_REALTIME_MAX_HOTWORDS: usize = 100;

const MANUAL_USER_WEIGHT: i32 = 100;
const AUTO_USER_WEIGHT: i32 = 70;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrHotwordPack {
    pub words: Vec<AsrHotword>,
    pub max_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrHotword {
    pub text: String,
    pub weight: Option<i32>,
    pub source: HotwordSource,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotwordSource {
    ManualUser,
    AutoUser,
    CorrectionPair,
    Recent,
    AppContext,
    Domain,
    Builtin,
}

#[derive(Debug, Clone)]
pub struct TnlDictionaryPack {
    pub words: Vec<String>,
    pub correction_pairs: Vec<CorrectionPair>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmContextPack {
    pub lines: Vec<String>,
    pub correction_hints: Vec<CorrectionHint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionHint {
    pub original: String,
    pub corrected: String,
}

struct RankedHotword {
    hotword: AsrHotword,
    rank: i32,
    order: usize,
}

pub fn compile_user_dictionary_asr_pack(
    dictionary_entries: &[String],
    max_count: usize,
) -> AsrHotwordPack {
    let mut by_key: HashMap<String, RankedHotword> = HashMap::new();

    for (order, entry) in dictionary_entries.iter().enumerate() {
        let text = extract_word(entry).trim();
        if text.is_empty() {
            continue;
        }

        let source = hotword_source_from_entry(entry);
        let rank = hotword_rank(&source);
        let key = text.to_lowercase();
        let hotword = AsrHotword {
            text: text.to_string(),
            weight: Some(rank),
            source,
            aliases: aliases_for_entry(text, entry),
        };

        match by_key.get_mut(&key) {
            Some(existing) if rank > existing.rank => {
                *existing = RankedHotword {
                    hotword,
                    rank,
                    order: existing.order,
                };
            }
            Some(_) => {}
            None => {
                by_key.insert(
                    key,
                    RankedHotword {
                        hotword,
                        rank,
                        order,
                    },
                );
            }
        }
    }

    let mut ranked = by_key.into_values().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.rank.cmp(&a.rank).then_with(|| a.order.cmp(&b.order)));

    let words = ranked
        .into_iter()
        .take(max_count)
        .map(|ranked| ranked.hotword)
        .collect();

    AsrHotwordPack { words, max_count }
}

pub fn compile_tnl_dictionary_pack(dictionary_entries: &[String]) -> TnlDictionaryPack {
    TnlDictionaryPack {
        words: dictionary_entries
            .iter()
            .filter_map(|entry| {
                let word = extract_word(entry).trim();
                if word.is_empty() {
                    None
                } else {
                    Some(word.to_string())
                }
            })
            .collect(),
        correction_pairs: Vec::new(),
    }
}

pub fn compile_llm_context_pack(dictionary_entries: &[String], max_count: usize) -> LlmContextPack {
    let asr_pack = compile_user_dictionary_asr_pack(dictionary_entries, max_count);
    LlmContextPack {
        lines: asr_pack
            .words
            .iter()
            .map(|hotword| hotword.text.clone())
            .collect(),
        correction_hints: Vec::new(),
    }
}

pub fn render_qwen_corpus_text(pack: &AsrHotwordPack) -> String {
    pack.words
        .iter()
        .map(|hotword| hotword.text.as_str())
        .collect::<Vec<_>>()
        .join("、")
}

pub fn render_doubao_hotwords(pack: &AsrHotwordPack) -> Vec<Value> {
    pack.words
        .iter()
        .map(|hotword| serde_json::json!({ "word": hotword.text }))
        .collect()
}

fn hotword_source_from_entry(entry: &str) -> HotwordSource {
    if entry.split('|').nth(1).map(str::trim) == Some("auto") {
        HotwordSource::AutoUser
    } else {
        HotwordSource::ManualUser
    }
}

fn hotword_rank(source: &HotwordSource) -> i32 {
    match source {
        HotwordSource::ManualUser => MANUAL_USER_WEIGHT,
        HotwordSource::AutoUser => AUTO_USER_WEIGHT,
        HotwordSource::CorrectionPair => 90,
        HotwordSource::Recent => 80,
        HotwordSource::AppContext => 75,
        HotwordSource::Domain => 60,
        HotwordSource::Builtin => 30,
    }
}

fn aliases_for_entry(text: &str, entry: &str) -> Vec<String> {
    let category = entry.split('|').nth(2).map(str::trim);
    let mut aliases = Vec::new();

    if matches!(category, Some("code_symbol")) && text.contains('-') {
        aliases.push(text.replace('-', " "));
    }

    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_dictionary_entries_with_dedup_and_manual_priority() {
        let entries = vec![
            "Claude Code|auto|product".to_string(),
            "Rust|auto|tool".to_string(),
            "Claude Code|manual|product".to_string(),
            "团队约定|manual|phrase".to_string(),
            " ".to_string(),
        ];

        let pack = compile_user_dictionary_asr_pack(&entries, 10);

        assert_eq!(pack.words.len(), 3);
        assert_eq!(pack.words[0].text, "Claude Code");
        assert_eq!(pack.words[0].source, HotwordSource::ManualUser);
        assert_eq!(pack.words[0].weight, Some(MANUAL_USER_WEIGHT));
        assert_eq!(pack.words[1].text, "团队约定");
        assert_eq!(pack.words[2].text, "Rust");
    }

    #[test]
    fn compiles_dictionary_entries_with_provider_limit() {
        let entries = vec!["一号".to_string(), "二号".to_string(), "三号".to_string()];

        let pack = compile_user_dictionary_asr_pack(&entries, 2);

        assert_eq!(pack.max_count, 2);
        assert_eq!(
            pack.words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            vec!["一号", "二号"]
        );
    }

    #[test]
    fn renders_qwen_corpus_without_metadata() {
        let entries = vec![
            "Claude Code|manual|product".to_string(),
            "Rust|auto|tool".to_string(),
        ];
        let pack = compile_user_dictionary_asr_pack(&entries, 10);

        assert_eq!(render_qwen_corpus_text(&pack), "Claude Code、Rust");
    }

    #[test]
    fn renders_doubao_hotwords_with_legacy_word_shape() {
        let entries = vec!["Claude Code|manual|product".to_string()];
        let pack = compile_user_dictionary_asr_pack(&entries, 10);

        assert_eq!(
            render_doubao_hotwords(&pack),
            vec![serde_json::json!({ "word": "Claude Code" })]
        );
    }

    #[test]
    fn builds_tnl_and_llm_packs_from_same_pure_words() {
        let entries = vec![
            "Claude Code|manual|product".to_string(),
            "Rust|auto|tool".to_string(),
        ];

        let tnl_pack = compile_tnl_dictionary_pack(&entries);
        let llm_pack = compile_llm_context_pack(&entries, 10);

        assert_eq!(tnl_pack.words, vec!["Claude Code", "Rust"]);
        assert!(tnl_pack.correction_pairs.is_empty());
        assert_eq!(llm_pack.lines, vec!["Claude Code", "Rust"]);
        assert!(llm_pack.correction_hints.is_empty());
    }

    #[test]
    fn records_code_symbol_alias_for_future_provider_formats() {
        let entries = vec!["GPT-5.3-Codex|manual|code_symbol".to_string()];

        let pack = compile_user_dictionary_asr_pack(&entries, 10);

        assert_eq!(pack.words[0].aliases, vec!["GPT 5.3 Codex"]);
    }
}
