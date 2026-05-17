// 词库工具函数
//
// 独立模块，提供词库条目的解析和转换功能
// 被 ASR、LLM、Learning 等多个模块共享使用

use std::collections::HashSet;

const VALID_DICTIONARY_CATEGORIES: &[&str] = &[
    "person",
    "product",
    "tool",
    "phrase",
    "email",
    "url",
    "code_symbol",
    "domain_term",
    "generic",
];

/// 标准化词汇（去除首尾空格）
pub fn normalize_word(word: &str) -> String {
    word.trim().to_string()
}

fn normalize_source(source: &str) -> &'static str {
    if source == "auto" {
        "auto"
    } else {
        "manual"
    }
}

fn normalize_dictionary_category(category: Option<&str>) -> Option<&'static str> {
    let category = category?.trim();
    if category.is_empty() {
        return None;
    }

    if VALID_DICTIONARY_CATEGORIES.contains(&category) {
        return Some(match category {
            "person" => "person",
            "product" => "product",
            "tool" => "tool",
            "phrase" => "phrase",
            "email" => "email",
            "url" => "url",
            "code_symbol" => "code_symbol",
            "domain_term" => "domain_term",
            "generic" => "generic",
            _ => unreachable!(),
        });
    }

    match category {
        "proper_noun" => Some("product"),
        "term" => Some("domain_term"),
        "frequent" => Some("generic"),
        _ => None,
    }
}

/// 标准化词库分类 metadata。
pub fn normalize_category(category: Option<&str>) -> Option<&'static str> {
    normalize_dictionary_category(category)
}

fn has_whitespace(value: &str) -> bool {
    value.chars().any(char::is_whitespace)
}

fn is_email_like(value: &str) -> bool {
    if has_whitespace(value) {
        return false;
    }

    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return false;
    }

    domain
        .split_once('.')
        .is_some_and(|(prefix, suffix)| !prefix.is_empty() && !suffix.is_empty())
}

fn is_url_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    ["http://", "https://", "www."].iter().any(|prefix| {
        lower.starts_with(prefix)
            && value[prefix.len()..]
                .chars()
                .next()
                .is_some_and(|ch| !ch.is_whitespace())
    })
}

fn has_camel_case(value: &str) -> bool {
    let mut previous_lowercase = false;
    for ch in value.chars() {
        if previous_lowercase && ch.is_ascii_uppercase() {
            return true;
        }
        previous_lowercase = ch.is_ascii_lowercase();
    }
    false
}

fn has_alnum_hyphen_alnum(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    chars.windows(3).any(|window| {
        window[0].is_ascii_alphanumeric() && window[1] == '-' && window[2].is_ascii_alphanumeric()
    })
}

fn is_cjk_char(ch: char) -> bool {
    matches!(ch, '\u{4e00}'..='\u{9fff}')
}

/// 按前端同款规则推断词库分类。
pub fn infer_dictionary_category(word: &str) -> &'static str {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return "generic";
    }

    if is_email_like(trimmed) {
        return "email";
    }

    if is_url_like(trimmed) {
        return "url";
    }

    if trimmed.is_ascii()
        && (has_camel_case(trimmed)
            || trimmed.contains('_')
            || trimmed.contains('/')
            || trimmed.contains('\\')
            || has_alnum_hyphen_alnum(trimmed))
    {
        return "code_symbol";
    }

    let cjk_count = trimmed.chars().filter(|ch| is_cjk_char(*ch)).count();
    if cjk_count >= 2 && !has_whitespace(trimmed) {
        return "phrase";
    }

    "generic"
}

/// 标准化有效分类；缺省或无效时按词面推断分类。
pub fn normalize_or_infer_category(word: &str, category: Option<&str>) -> &'static str {
    normalize_dictionary_category(category).unwrap_or_else(|| infer_dictionary_category(word))
}

/// 格式化词条（添加来源标记）
///
/// - source = "manual" -> "word"
/// - source = "auto" -> "word|auto"
pub fn format_entry(word: &str, source: &str) -> String {
    let normalized = normalize_word(word);
    if normalize_source(source) == "auto" {
        format!("{}|auto", normalized)
    } else {
        normalized
    }
}

/// 格式化词条（添加来源和分类标记）
///
/// - generic 分类保持旧格式，最大限度兼容旧配置
/// - 非 generic 分类使用 "word|source|category"
pub fn format_entry_with_category(word: &str, source: &str, category: Option<&str>) -> String {
    let normalized = normalize_word(word);
    let normalized_source = normalize_source(source);
    let normalized_category = normalize_dictionary_category(category);

    if matches!(normalized_category, Some("generic") | None) {
        return format_entry(&normalized, normalized_source);
    }

    format!(
        "{}|{}|{}",
        normalized,
        normalized_source,
        normalized_category.unwrap()
    )
}

/// 解析词条，提取纯词汇（去除 |auto 后缀）
pub fn extract_word(entry: &str) -> &str {
    entry.split('|').next().unwrap_or(entry)
}

fn extract_source(entry: &str) -> &'static str {
    normalize_source(entry.split('|').nth(1).unwrap_or("manual"))
}

/// 解析词条，提取标准化后的分类 metadata。
pub fn extract_category(entry: &str) -> Option<&'static str> {
    normalize_dictionary_category(entry.split('|').nth(2))
}

/// 插入或更新词条（去重）
///
/// 如果词汇已存在：
/// - 如果新来源是 manual，则更新为 manual（优先级更高）
/// - 如果新来源是 auto，保持原来源不变
#[allow(dead_code)]
pub fn upsert_entry(entries: &mut Vec<String>, word: &str, source: &str) {
    upsert_entry_with_category(entries, word, source, None);
}

/// 插入或更新词条（保留可选分类 metadata）
pub fn upsert_entry_with_category(
    entries: &mut Vec<String>,
    word: &str,
    source: &str,
    category: Option<&str>,
) {
    let normalized = normalize_word(word);
    if normalized.is_empty() {
        return;
    }

    // 检查是否已存在
    if let Some(existing) = entries.iter_mut().find(|e| extract_word(e) == normalized) {
        let existing_source = extract_source(existing);
        let next_source = if normalize_source(source) == "manual" || existing_source == "manual" {
            "manual"
        } else {
            "auto"
        };
        let next_category = if category.is_some() {
            normalize_dictionary_category(category).or_else(|| extract_category(existing))
        } else {
            extract_category(existing)
        };

        *existing = format_entry_with_category(&normalized, next_source, next_category);
        return;
    }

    // 不存在，新增
    entries.push(format_entry_with_category(&normalized, source, category));
}

/// 插入或更新词条；当调用方没有提供有效分类时，按词面推断分类。
pub fn upsert_entry_with_inferred_category(
    entries: &mut Vec<String>,
    word: &str,
    source: &str,
    category: Option<&str>,
) {
    let normalized = normalize_word(word);
    if normalized.is_empty() {
        return;
    }

    let existing_category = entries
        .iter()
        .find(|entry| extract_word(entry) == normalized)
        .and_then(|entry| extract_category(entry));
    let next_category = if let Some(category) = normalize_dictionary_category(category) {
        category
    } else {
        existing_category.unwrap_or_else(|| normalize_or_infer_category(&normalized, category))
    };

    upsert_entry_with_category(entries, &normalized, source, Some(next_category));
}

/// 批量补齐/规范化词库分类 metadata。
pub fn backfill_inferred_categories(entries: &mut Vec<String>) -> bool {
    let mut changed = false;

    for entry in entries.iter_mut() {
        let normalized = normalize_word(extract_word(entry));
        let source = extract_source(entry);
        let category =
            extract_category(entry).unwrap_or_else(|| infer_dictionary_category(&normalized));
        let next_entry = format_entry_with_category(&normalized, source, Some(category));

        if *entry != next_entry {
            *entry = next_entry;
            changed = true;
        }
    }

    changed
}

/// 删除指定词汇（按 word 匹配，不区分来源）
pub fn remove_entries(entries: &mut Vec<String>, words: &[String]) {
    let words_set: HashSet<&str> = words.iter().map(|s| s.as_str()).collect();
    entries.retain(|e| {
        let word = extract_word(e);
        !words_set.contains(word)
    });
}

/// 将词条列表转换为纯词汇列表（用于 ASR API）
///
/// 去除所有 |auto 后缀，只保留纯词汇
pub fn entries_to_words(entries: &[String]) -> Vec<String> {
    entries
        .iter()
        .map(|e| extract_word(e).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_entry() {
        assert_eq!(format_entry("claude code", "manual"), "claude code");
        assert_eq!(format_entry("claude code", "auto"), "claude code|auto");
        assert_eq!(format_entry("  claude code  ", "manual"), "claude code");
    }

    #[test]
    fn test_extract_word() {
        assert_eq!(extract_word("claude code"), "claude code");
        assert_eq!(extract_word("claude code|auto"), "claude code");
        assert_eq!(extract_word("CLAUDE.md|auto"), "CLAUDE.md");
        assert_eq!(extract_word("word|auto|extra"), "word"); // 只取第一段
        assert_eq!(extract_word(""), "");
    }

    #[test]
    fn test_extract_category() {
        assert_eq!(normalize_category(Some("code_symbol")), Some("code_symbol"));
        assert_eq!(normalize_category(Some("proper_noun")), Some("product"));
        assert_eq!(normalize_category(Some("frequent")), Some("generic"));
        assert_eq!(
            extract_category("Claude Code|manual|product"),
            Some("product")
        );
        assert_eq!(
            extract_category("Claude Code|manual|term"),
            Some("domain_term")
        );
        assert_eq!(extract_category("Claude Code|manual|unknown"), None);
        assert_eq!(extract_category("Claude Code|manual"), None);
    }

    #[test]
    fn test_infer_dictionary_category() {
        assert_eq!(infer_dictionary_category("user@example.com"), "email");
        assert_eq!(infer_dictionary_category("https://example.com/docs"), "url");
        assert_eq!(infer_dictionary_category("www.example.com"), "url");
        assert_eq!(infer_dictionary_category("useState"), "code_symbol");
        assert_eq!(infer_dictionary_category("async_await"), "code_symbol");
        assert_eq!(infer_dictionary_category("src/main.rs"), "code_symbol");
        assert_eq!(infer_dictionary_category("GPT-5.3-Codex"), "code_symbol");
        assert_eq!(infer_dictionary_category("团队约定"), "phrase");
        assert_eq!(infer_dictionary_category("rust"), "generic");
        assert_eq!(infer_dictionary_category(""), "generic");
    }

    #[test]
    fn test_normalize_or_infer_category_prefers_valid_or_legacy_category() {
        assert_eq!(
            normalize_or_infer_category("useState", Some("tool")),
            "tool"
        );
        assert_eq!(
            normalize_or_infer_category("useState", Some("term")),
            "domain_term"
        );
        assert_eq!(
            normalize_or_infer_category("useState", Some("unknown")),
            "code_symbol"
        );
        assert_eq!(normalize_or_infer_category("团队约定", None), "phrase");
    }

    #[test]
    fn test_upsert_entry() {
        let mut entries = vec![];

        // 添加 manual
        upsert_entry(&mut entries, "claude", "manual");
        assert_eq!(entries, vec!["claude"]);

        // 添加 auto
        upsert_entry(&mut entries, "rust", "auto");
        assert_eq!(entries, vec!["claude", "rust|auto"]);

        // 重复添加 auto（不更新）
        upsert_entry(&mut entries, "rust", "auto");
        assert_eq!(entries, vec!["claude", "rust|auto"]);

        // 重复添加 manual（更新为 manual）
        upsert_entry(&mut entries, "rust", "manual");
        assert_eq!(entries, vec!["claude", "rust"]);
    }

    #[test]
    fn test_remove_entries() {
        let mut entries = vec![
            "claude".to_string(),
            "rust|auto".to_string(),
            "python".to_string(),
        ];

        remove_entries(&mut entries, &vec!["rust".to_string()]);
        assert_eq!(entries, vec!["claude", "python"]);
    }

    #[test]
    fn test_entries_to_words() {
        let entries = vec![
            "claude".to_string(),
            "rust|auto".to_string(),
            "python".to_string(),
        ];

        let words = entries_to_words(&entries);
        assert_eq!(words, vec!["claude", "rust", "python"]);
    }

    #[test]
    fn test_category_metadata_round_trip() {
        assert_eq!(
            format_entry_with_category("Claude Code", "auto", Some("product")),
            "Claude Code|auto|product"
        );
        assert_eq!(
            format_entry_with_category("团队约定", "manual", Some("phrase")),
            "团队约定|manual|phrase"
        );
        assert_eq!(
            format_entry_with_category("rust", "manual", Some("generic")),
            "rust"
        );

        let entries = vec![
            "Claude Code|auto|product".to_string(),
            "团队约定|manual|phrase".to_string(),
            "rust".to_string(),
        ];
        assert_eq!(
            entries_to_words(&entries),
            vec!["Claude Code", "团队约定", "rust"]
        );
    }

    #[test]
    fn test_upsert_entry_with_category_preserves_source_priority() {
        let mut entries = vec![];

        upsert_entry_with_category(&mut entries, "Claude Code", "auto", Some("product"));
        assert_eq!(entries, vec!["Claude Code|auto|product"]);

        upsert_entry_with_category(&mut entries, "Claude Code", "manual", Some("tool"));
        assert_eq!(entries, vec!["Claude Code|manual|tool"]);

        upsert_entry_with_category(&mut entries, "Claude Code", "auto", Some("domain_term"));
        assert_eq!(entries, vec!["Claude Code|manual|domain_term"]);
    }

    #[test]
    fn test_upsert_entry_with_inferred_category_infers_missing_and_invalid_category() {
        let mut entries = vec![];

        upsert_entry_with_inferred_category(&mut entries, "useState", "auto", None);
        assert_eq!(entries, vec!["useState|auto|code_symbol"]);

        upsert_entry_with_inferred_category(&mut entries, "团队约定", "manual", Some("unknown"));
        assert_eq!(
            entries,
            vec!["useState|auto|code_symbol", "团队约定|manual|phrase"]
        );

        upsert_entry_with_inferred_category(&mut entries, "rust", "auto", None);
        assert_eq!(
            entries,
            vec![
                "useState|auto|code_symbol",
                "团队约定|manual|phrase",
                "rust|auto"
            ]
        );
    }

    #[test]
    fn test_upsert_entry_with_inferred_category_preserves_existing_category() {
        let mut entries = vec!["Claude Code|manual|product".to_string()];

        upsert_entry_with_inferred_category(&mut entries, "Claude Code", "auto", None);

        assert_eq!(entries, vec!["Claude Code|manual|product"]);
    }

    #[test]
    fn test_backfill_inferred_categories_updates_legacy_entries() {
        let mut entries = vec![
            "useState|auto".to_string(),
            "团队约定".to_string(),
            "rust|auto".to_string(),
            "Claude Code|manual|product".to_string(),
            "深度求索|auto|term".to_string(),
            "legacy@example.com|manual|unknown".to_string(),
        ];

        assert!(backfill_inferred_categories(&mut entries));
        assert_eq!(
            entries,
            vec![
                "useState|auto|code_symbol",
                "团队约定|manual|phrase",
                "rust|auto",
                "Claude Code|manual|product",
                "深度求索|auto|domain_term",
                "legacy@example.com|manual|email",
            ]
        );
    }

    #[test]
    fn test_backfill_inferred_categories_reports_no_change_when_canonical() {
        let mut entries = vec![
            "useState|auto|code_symbol".to_string(),
            "团队约定|manual|phrase".to_string(),
            "rust|auto".to_string(),
        ];

        assert!(!backfill_inferred_categories(&mut entries));
        assert_eq!(
            entries,
            vec![
                "useState|auto|code_symbol",
                "团队约定|manual|phrase",
                "rust|auto",
            ]
        );
    }
}
