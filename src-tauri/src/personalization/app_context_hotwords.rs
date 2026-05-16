use std::collections::HashSet;

const APP_CONTEXT_SOURCE: &str = "app_context";
const APP_CONTEXT_HOTWORD_LIMIT: usize = 20;
const APP_CONTEXT_TEXT_LIMIT_CHARS: usize = 4_000;

#[derive(Debug, Clone)]
struct ContextToken {
    text: String,
    start: usize,
    end: usize,
}

pub(crate) fn build_app_context_hotword_entries(text: &str, limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let limited_text = limit_context_text(text);
    let tokens = tokenize_context_text(&limited_text);
    let mut entries = Vec::new();
    let mut seen_words = HashSet::new();
    let mut index = 0usize;

    while index < tokens.len() && entries.len() < limit {
        if let Some(phrase_len) = best_phrase_len(&tokens, index, &limited_text) {
            let phrase = tokens[index..index + phrase_len]
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            push_entry(&mut entries, &mut seen_words, &phrase, false, limit);
            index += phrase_len;
            continue;
        }

        let token = &tokens[index].text;
        if is_single_candidate(token) {
            push_entry(&mut entries, &mut seen_words, token, true, limit);
        }
        index += 1;
    }

    entries
}

pub(crate) fn augment_dictionary_with_app_context_hotwords(
    dictionary: Vec<String>,
    context_text: &str,
) -> Vec<String> {
    let entries = build_app_context_hotword_entries(context_text, APP_CONTEXT_HOTWORD_LIMIT);
    if entries.is_empty() {
        return dictionary;
    }

    let mut augmented = dictionary;
    augmented.extend(entries);
    augmented
}

fn limit_context_text(text: &str) -> String {
    text.chars().take(APP_CONTEXT_TEXT_LIMIT_CHARS).collect()
}

fn tokenize_context_text(text: &str) -> Vec<ContextToken> {
    let mut tokens = Vec::new();
    let mut start = None;

    for (index, ch) in text.char_indices() {
        if is_context_token_char(ch) {
            start.get_or_insert(index);
            continue;
        }

        if let Some(token_start) = start.take() {
            push_normalized_token(&mut tokens, text, token_start, index);
        }
    }

    if let Some(token_start) = start {
        push_normalized_token(&mut tokens, text, token_start, text.len());
    }

    tokens
}

fn push_normalized_token(tokens: &mut Vec<ContextToken>, text: &str, start: usize, end: usize) {
    let raw = &text[start..end];
    if let Some(normalized) = normalize_context_token(raw) {
        tokens.push(ContextToken {
            text: normalized,
            start,
            end,
        });
    }
}

fn is_context_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '+' | '#' | '/' | '\\' | ':' | '@')
}

fn normalize_context_token(raw: &str) -> Option<String> {
    let mut token = trim_token_edges(raw);
    if token.is_empty()
        || token.contains('@')
        || token.contains("://")
        || token.to_ascii_lowercase().starts_with("www.")
    {
        return None;
    }

    if token.contains('/') || token.contains('\\') {
        token = token
            .rsplit(['/', '\\'])
            .next()
            .map(trim_token_edges)
            .unwrap_or_default();
    }

    token = trim_token_edges(token);
    if token.is_empty() || token.chars().count() > 80 {
        return None;
    }

    Some(token.to_string())
}

fn trim_token_edges(token: &str) -> &str {
    token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '#' && ch != '+')
}

fn best_phrase_len(tokens: &[ContextToken], start: usize, source_text: &str) -> Option<usize> {
    let mut max_len = 0usize;
    for offset in 0..4 {
        let Some(token) = tokens.get(start + offset) else {
            break;
        };
        if offset > 0 {
            let previous = &tokens[start + offset - 1];
            if !has_soft_gap(source_text, previous.end, token.start) {
                break;
            }
        }
        if !is_phrase_word(&token.text) {
            break;
        }
        max_len += 1;
    }

    if max_len < 2 {
        return None;
    }

    (2..=max_len)
        .rev()
        .find(|len| !has_repeated_phrase_word(&tokens[start..start + len]))
}

fn has_soft_gap(text: &str, start: usize, end: usize) -> bool {
    text[start..end]
        .chars()
        .all(|ch| ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '\\'))
}

fn is_phrase_word(word: &str) -> bool {
    !is_stopword(word) && is_title_or_pascal_word(word) && !is_code_symbol_candidate(word)
}

fn has_repeated_phrase_word(tokens: &[ContextToken]) -> bool {
    let mut seen = HashSet::new();
    tokens
        .iter()
        .any(|token| !seen.insert(token.text.to_ascii_lowercase()))
}

fn is_single_candidate(word: &str) -> bool {
    let char_count = word.chars().count();
    if !(2..=80).contains(&char_count) || is_stopword(word) || !has_ascii_letter(word) {
        return false;
    }

    is_code_symbol_candidate(word)
        || is_all_caps_candidate(word)
        || is_camel_or_pascal_candidate(word)
}

fn push_entry(
    entries: &mut Vec<String>,
    seen_words: &mut HashSet<String>,
    word: &str,
    prefer_code_symbol: bool,
    limit: usize,
) {
    if entries.len() >= limit {
        return;
    }

    let normalized = word.trim();
    if normalized.is_empty() {
        return;
    }

    let key = normalized.to_ascii_lowercase();
    if !seen_words.insert(key) {
        return;
    }

    let category = if prefer_code_symbol && is_code_symbol_candidate(normalized) {
        "code_symbol"
    } else {
        "generic"
    };
    entries.push(format!(
        "{}|{}|{}",
        normalized, APP_CONTEXT_SOURCE, category
    ));
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "a" | "an"
            | "and"
            | "app"
            | "application"
            | "account"
            | "add"
            | "cancel"
            | "close"
            | "copy"
            | "delete"
            | "edit"
            | "editing"
            | "file"
            | "folder"
            | "for"
            | "from"
            | "hello"
            | "help"
            | "home"
            | "login"
            | "main"
            | "new"
            | "no"
            | "none"
            | "ok"
            | "open"
            | "or"
            | "paste"
            | "project"
            | "recent"
            | "remove"
            | "save"
            | "screen"
            | "search"
            | "settings"
            | "shows"
            | "that"
            | "the"
            | "this"
            | "true"
            | "window"
            | "with"
            | "world"
            | "yes"
    )
}

fn has_ascii_letter(word: &str) -> bool {
    word.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn is_title_or_pascal_word(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase() && chars.any(|ch| ch.is_ascii_lowercase())
}

fn is_camel_or_pascal_candidate(word: &str) -> bool {
    let char_count = word.chars().count();
    char_count >= 5 && is_title_or_pascal_word(word)
}

fn is_all_caps_candidate(word: &str) -> bool {
    let letters = word
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<Vec<_>>();
    (2..=10).contains(&letters.len()) && letters.iter().all(|ch| ch.is_ascii_uppercase())
}

fn is_code_symbol_candidate(word: &str) -> bool {
    if word.contains('.')
        && !has_known_file_extension(word)
        && !word.chars().any(|ch| ch.is_ascii_digit())
    {
        return false;
    }

    word.chars().any(|ch| ch.is_ascii_digit())
        || word.contains('_')
        || word.contains('+')
        || word.contains('#')
        || (word.contains('-')
            && (word.chars().any(|ch| ch.is_ascii_digit())
                || word.chars().any(|ch| ch.is_ascii_uppercase())))
        || has_known_file_extension(word)
}

fn has_known_file_extension(word: &str) -> bool {
    let Some(extension) = word.rsplit('.').next() else {
        return false;
    };
    if extension == word {
        return false;
    }

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "bat"
            | "css"
            | "dll"
            | "exe"
            | "html"
            | "js"
            | "json"
            | "jsx"
            | "md"
            | "proto"
            | "ps1"
            | "py"
            | "rs"
            | "sql"
            | "toml"
            | "ts"
            | "tsx"
            | "xml"
            | "yaml"
            | "yml"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_product_phrase_and_code_symbols_from_context_text() {
        let entries = build_app_context_hotword_entries(
            "当前窗口正在编辑 Claude Code、GPT-5.3-Codex 和 ASR_PERSONALIZATION_QUALITY_LEAP.md",
            10,
        );

        assert!(entries.contains(&"Claude Code|app_context|generic".to_string()));
        assert!(entries.contains(&"GPT-5.3-Codex|app_context|code_symbol".to_string()));
        assert!(entries
            .contains(&"ASR_PERSONALIZATION_QUALITY_LEAP.md|app_context|code_symbol".to_string()));
    }

    #[test]
    fn ignores_ordinary_text_and_sensitive_tokens() {
        let entries = build_app_context_hotword_entries(
            "This screen shows login settings account save cancel hello world user@example.com https://example.com",
            10,
        );

        assert!(entries.is_empty(), "{entries:?}");
    }

    #[test]
    fn dedupes_candidates_and_respects_limit() {
        let entries = build_app_context_hotword_entries(
            "Claude Code Claude Code GPT-5.3-Codex TypeScript OpenAI Cursor",
            2,
        );

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], "Claude Code|app_context|generic");
        assert_eq!(entries[1], "GPT-5.3-Codex|app_context|code_symbol");
    }

    #[test]
    fn augment_appends_runtime_entries_without_persisted_dedupe() {
        let dictionary = vec![
            "Claude Code|domain|domain_term".to_string(),
            "用户短语|manual|phrase".to_string(),
        ];

        let augmented =
            augment_dictionary_with_app_context_hotwords(dictionary, "Claude Code GPT-5.3-Codex");

        assert!(augmented.contains(&"Claude Code|domain|domain_term".to_string()));
        assert!(augmented.contains(&"Claude Code|app_context|generic".to_string()));
        assert!(augmented.contains(&"GPT-5.3-Codex|app_context|code_symbol".to_string()));
    }
}
