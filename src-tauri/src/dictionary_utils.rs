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

fn extract_category(entry: &str) -> Option<&'static str> {
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
}
