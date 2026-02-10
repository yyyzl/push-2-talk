// 词库工具函数
//
// 独立模块，提供词库条目的解析和转换功能
// 被 ASR、LLM、Learning 等多个模块共享使用

use std::collections::HashSet;

/// 标准化词汇（去除首尾空格）
pub fn normalize_word(word: &str) -> String {
    word.trim().to_string()
}

/// 格式化词条（添加来源标记）
///
/// - source = "manual" -> "word"
/// - source = "auto" -> "word|auto"
pub fn format_entry(word: &str, source: &str) -> String {
    let normalized = normalize_word(word);
    if source == "auto" {
        format!("{}|auto", normalized)
    } else {
        normalized
    }
}

/// 解析词条，提取纯词汇（去除 |auto 后缀）
pub fn extract_word(entry: &str) -> &str {
    entry.split('|').next().unwrap_or(entry)
}

/// 插入或更新词条（去重）
///
/// 如果词汇已存在：
/// - 如果新来源是 manual，则更新为 manual（优先级更高）
/// - 如果新来源是 auto，保持原来源不变
pub fn upsert_entry(entries: &mut Vec<String>, word: &str, source: &str) {
    let normalized = normalize_word(word);
    if normalized.is_empty() {
        return;
    }

    // 检查是否已存在
    if let Some(existing) = entries.iter_mut().find(|e| extract_word(e) == normalized) {
        // 已存在，如果是手动添加，更新为 manual（去除 |auto 后缀）
        if source == "manual" {
            *existing = normalized.clone();
        }
        return;
    }

    // 不存在，新增
    entries.push(format_entry(&normalized, source));
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
}
