use crate::dictionary_utils::extract_word;
use crate::personalization::correction_pair_store::CorrectionPair;
use serde_json::Value;
use std::collections::HashMap;

pub const QWEN_HTTP_MAX_HOTWORDS: usize = 50;
pub const QWEN_REALTIME_MAX_HOTWORDS: usize = 50;
pub const DOUBAO_HTTP_MAX_HOTWORDS: usize = 100;
pub const DOUBAO_REALTIME_MAX_HOTWORDS: usize = 100;

const MANUAL_USER_WEIGHT: i32 = 100;
const CORRECTION_PAIR_WEIGHT: i32 = 90;
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

struct RankedHotword {
    hotword: AsrHotword,
    rank: i32,
    order: usize,
}

pub fn compile_asr_pack_with_correction_pairs(
    dictionary_entries: &[String],
    correction_pairs: &[CorrectionPair],
    max_count: usize,
) -> AsrHotwordPack {
    let mut by_key: HashMap<String, RankedHotword> = HashMap::new();

    for (order, entry) in dictionary_entries.iter().enumerate() {
        if let Some(hotword) = dictionary_entry_hotword(entry) {
            insert_ranked_hotword(&mut by_key, hotword, order);
        }
    }

    let pair_order_offset = dictionary_entries.len();
    for (index, pair) in correction_pairs.iter().enumerate() {
        if let Some(hotword) = correction_pair_hotword(pair) {
            insert_ranked_hotword(&mut by_key, hotword, pair_order_offset + index);
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
    match entry.split('|').nth(1).map(str::trim) {
        Some("auto") => HotwordSource::AutoUser,
        Some("recent") => HotwordSource::Recent,
        Some("app_context") => HotwordSource::AppContext,
        Some("domain") => HotwordSource::Domain,
        Some("builtin") => HotwordSource::Builtin,
        _ => HotwordSource::ManualUser,
    }
}

fn hotword_rank(source: &HotwordSource) -> i32 {
    match source {
        HotwordSource::ManualUser => MANUAL_USER_WEIGHT,
        HotwordSource::AutoUser => AUTO_USER_WEIGHT,
        HotwordSource::CorrectionPair => CORRECTION_PAIR_WEIGHT,
        HotwordSource::Recent => 80,
        HotwordSource::AppContext => 75,
        HotwordSource::Domain => 60,
        HotwordSource::Builtin => 30,
    }
}

fn dictionary_entry_hotword(entry: &str) -> Option<AsrHotword> {
    let text = extract_word(entry).trim();
    if text.is_empty() {
        return None;
    }

    let source = hotword_source_from_entry(entry);
    let rank = hotword_rank(&source);
    Some(AsrHotword {
        text: text.to_string(),
        weight: Some(rank),
        source,
        aliases: aliases_for_entry(text, entry),
    })
}

fn correction_pair_hotword(pair: &CorrectionPair) -> Option<AsrHotword> {
    let corrected_text = pair.corrected_text.trim();
    if !pair.enabled || corrected_text.is_empty() {
        return None;
    }

    let mut aliases = Vec::new();
    let original_text = pair.original_text.trim();
    if !original_text.is_empty() && !original_text.eq_ignore_ascii_case(corrected_text) {
        aliases.push(original_text.to_string());
    }
    aliases.extend(
        pair.alias_keys
            .iter()
            .map(|alias| alias.trim())
            .filter(|alias| !alias.is_empty())
            .map(ToOwned::to_owned),
    );

    Some(AsrHotword {
        text: corrected_text.to_string(),
        weight: Some(CORRECTION_PAIR_WEIGHT),
        source: HotwordSource::CorrectionPair,
        aliases,
    })
}

fn insert_ranked_hotword(
    by_key: &mut HashMap<String, RankedHotword>,
    hotword: AsrHotword,
    order: usize,
) {
    let rank = hotword.weight.unwrap_or_default();
    let key = hotword.text.to_lowercase();
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

        let pack = compile_asr_pack_with_correction_pairs(&entries, &[], 10);

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

        let pack = compile_asr_pack_with_correction_pairs(&entries, &[], 2);

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
        let pack = compile_asr_pack_with_correction_pairs(&entries, &[], 10);

        assert_eq!(render_qwen_corpus_text(&pack), "Claude Code、Rust");
    }

    #[test]
    fn renders_doubao_hotwords_with_legacy_word_shape() {
        let entries = vec!["Claude Code|manual|product".to_string()];
        let pack = compile_asr_pack_with_correction_pairs(&entries, &[], 10);

        assert_eq!(
            render_doubao_hotwords(&pack),
            vec![serde_json::json!({ "word": "Claude Code" })]
        );
    }

    #[test]
    fn records_code_symbol_alias_for_future_provider_formats() {
        let entries = vec!["GPT-5.3-Codex|manual|code_symbol".to_string()];

        let pack = compile_asr_pack_with_correction_pairs(&entries, &[], 10);

        assert_eq!(pack.words[0].aliases, vec!["GPT 5.3 Codex"]);
    }

    #[test]
    fn compiles_correction_pairs_between_manual_and_auto_sources() {
        let entries = vec![
            "Rust|auto|tool".to_string(),
            "Claude Code|manual|product".to_string(),
        ];
        let pairs = vec![CorrectionPair::new("windsurf", "winds surf", "Windsurf")];

        let pack = compile_asr_pack_with_correction_pairs(&entries, &pairs, 10);

        assert_eq!(
            pack.words
                .iter()
                .map(|word| (&word.text, &word.source))
                .collect::<Vec<_>>(),
            vec![
                (&"Claude Code".to_string(), &HotwordSource::ManualUser),
                (&"Windsurf".to_string(), &HotwordSource::CorrectionPair),
                (&"Rust".to_string(), &HotwordSource::AutoUser),
            ]
        );
        assert_eq!(pack.words[1].aliases, vec!["winds surf"]);
    }

    #[test]
    fn ranks_runtime_domain_and_builtin_sources_below_user_sources() {
        let entries = vec![
            "领域术语|domain|domain_term".to_string(),
            "最近工具|recent|product".to_string(),
            "窗口上下文|app_context|domain_term".to_string(),
            "Rust|auto|tool".to_string(),
            "用户短语|manual|phrase".to_string(),
            "内置兜底|builtin|domain_term".to_string(),
        ];
        let pairs = vec![CorrectionPair::new("windsurf", "winds surf", "Windsurf")];

        let pack = compile_asr_pack_with_correction_pairs(&entries, &pairs, 10);

        assert_eq!(
            pack.words
                .iter()
                .map(|word| (&word.text, &word.source))
                .collect::<Vec<_>>(),
            vec![
                (&"用户短语".to_string(), &HotwordSource::ManualUser),
                (&"Windsurf".to_string(), &HotwordSource::CorrectionPair),
                (&"最近工具".to_string(), &HotwordSource::Recent),
                (&"窗口上下文".to_string(), &HotwordSource::AppContext),
                (&"Rust".to_string(), &HotwordSource::AutoUser),
                (&"领域术语".to_string(), &HotwordSource::Domain),
                (&"内置兜底".to_string(), &HotwordSource::Builtin),
            ]
        );
        assert_eq!(
            render_qwen_corpus_text(&pack),
            "用户短语、Windsurf、最近工具、窗口上下文、Rust、领域术语、内置兜底"
        );
        assert_eq!(
            render_doubao_hotwords(&pack)[5],
            serde_json::json!({ "word": "领域术语" })
        );
    }

    #[test]
    fn manual_dictionary_word_beats_same_correction_pair() {
        let entries = vec!["Claude Code|manual|product".to_string()];
        let pairs = vec![CorrectionPair::new(
            "cloud-code",
            "cloud code",
            "Claude Code",
        )];

        let pack = compile_asr_pack_with_correction_pairs(&entries, &pairs, 10);

        assert_eq!(pack.words.len(), 1);
        assert_eq!(pack.words[0].text, "Claude Code");
        assert_eq!(pack.words[0].source, HotwordSource::ManualUser);
    }

    #[test]
    fn skips_disabled_or_empty_correction_pairs() {
        let mut disabled = CorrectionPair::new("disabled", "old", "Disabled Term");
        disabled.enabled = false;
        let empty = CorrectionPair::new("empty", "old", " ");

        let pack = compile_asr_pack_with_correction_pairs(&[], &[disabled, empty], 10);

        assert!(pack.words.is_empty());
    }
}
