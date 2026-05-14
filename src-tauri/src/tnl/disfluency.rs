use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisfluencyMode {
    Off,
    Conservative,
    Aggressive,
}

impl Default for DisfluencyMode {
    fn default() -> Self {
        Self::Conservative
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisfluencyResult {
    pub text: String,
    pub changed: bool,
}

pub fn clean_disfluency(text: &str, mode: DisfluencyMode) -> DisfluencyResult {
    if mode == DisfluencyMode::Off || text.is_empty() {
        return DisfluencyResult {
            text: text.to_string(),
            changed: false,
        };
    }

    let mut cleaned = text.to_string();
    if mode == DisfluencyMode::Aggressive {
        cleaned = remove_simple_false_start(&cleaned);
        cleaned = collapse_repeated_cjk_chars(&cleaned);
    }
    cleaned = remove_leading_fillers(&cleaned);

    DisfluencyResult {
        changed: cleaned != text,
        text: cleaned,
    }
}

fn remove_leading_fillers(text: &str) -> String {
    let mut current = text.to_string();

    loop {
        let trimmed = current.trim_start();
        let Some(after) = strip_leading_filler(trimmed) else {
            break;
        };

        let next = strip_leading_separators(after).to_string();
        if next == current {
            break;
        }
        current = next;
    }

    current
}

fn strip_leading_filler(text: &str) -> Option<&str> {
    for phrase in ["怎么说呢", "就是说", "这个", "那个"] {
        if let Some(after) = strip_prefix_when_isolated(text, phrase) {
            return Some(after);
        }
    }

    for filler in ["嗯", "啊", "呃", "唉", "哎", "诶"] {
        if let Some(after) = strip_prefix_when_isolated(text, filler) {
            return Some(after);
        }
    }

    None
}

fn strip_prefix_when_isolated<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let after = text.strip_prefix(prefix)?;
    if after.is_empty() || after.chars().next().is_some_and(is_separator_or_whitespace) {
        Some(after)
    } else {
        None
    }
}

fn strip_leading_separators(text: &str) -> &str {
    text.trim_start_matches(is_separator_or_whitespace)
}

fn remove_simple_false_start(text: &str) -> String {
    let trimmed = text.trim_start();
    let Some((first_segment, after_first)) = split_once_separator(trimmed) else {
        return text.to_string();
    };

    if first_segment.chars().count() > 2 || first_segment.trim().is_empty() {
        return text.to_string();
    }

    let after_first = strip_leading_separators(after_first);
    let Some(after_filler) = strip_leading_filler(after_first) else {
        return text.to_string();
    };
    let after_filler = strip_leading_separators(after_filler);
    if after_filler.is_empty() {
        text.to_string()
    } else {
        after_filler.to_string()
    }
}

fn split_once_separator(text: &str) -> Option<(&str, &str)> {
    for (idx, ch) in text.char_indices() {
        if is_separator(ch) {
            let after = idx + ch.len_utf8();
            return Some((&text[..idx], &text[after..]));
        }
    }
    None
}

fn collapse_repeated_cjk_chars(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        let mut count = 1usize;
        while chars.peek().is_some_and(|next| *next == ch) {
            chars.next();
            count += 1;
        }

        if is_cjk_char(ch) && count >= 3 {
            output.push(ch);
        } else {
            for _ in 0..count {
                output.push(ch);
            }
        }
    }

    output
}

fn is_separator_or_whitespace(ch: char) -> bool {
    ch.is_whitespace() || is_separator(ch)
}

fn is_separator(ch: char) -> bool {
    matches!(
        ch,
        ',' | '，' | '.' | '。' | '!' | '！' | '?' | '？' | '、' | ';' | '；' | ':' | '：'
    )
}

fn is_cjk_char(ch: char) -> bool {
    let code = ch as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0x20000..=0x2CEAF).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conservative_removes_sentence_start_filler_char() {
        let result = clean_disfluency("嗯，我准备好了", DisfluencyMode::Conservative);

        assert!(result.changed);
        assert_eq!(result.text, "我准备好了");
    }

    #[test]
    fn conservative_removes_isolated_start_phrase() {
        let result = clean_disfluency("这个，我准备好了", DisfluencyMode::Conservative);

        assert!(result.changed);
        assert_eq!(result.text, "我准备好了");
    }

    #[test]
    fn conservative_preserves_non_filler_words() {
        assert_eq!(
            clean_disfluency("这个东西很重要", DisfluencyMode::Conservative).text,
            "这个东西很重要"
        );
        assert_eq!(
            clean_disfluency("嗯哼，我准备好了", DisfluencyMode::Conservative).text,
            "嗯哼，我准备好了"
        );
    }

    #[test]
    fn off_preserves_text() {
        let result = clean_disfluency("嗯，我准备好了", DisfluencyMode::Off);

        assert!(!result.changed);
        assert_eq!(result.text, "嗯，我准备好了");
    }

    #[test]
    fn aggressive_collapses_repeated_chars_and_long_fillers() {
        assert_eq!(
            clean_disfluency("我我我想打开设置", DisfluencyMode::Aggressive).text,
            "我想打开设置"
        );
        assert_eq!(
            clean_disfluency("嗯嗯嗯，我准备好了", DisfluencyMode::Aggressive).text,
            "我准备好了"
        );
    }

    #[test]
    fn aggressive_removes_simple_false_start() {
        let result = clean_disfluency("我，那个，今天开会", DisfluencyMode::Aggressive);

        assert_eq!(result.text, "今天开会");
    }
}
