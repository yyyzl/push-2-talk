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

    // Keep the legacy wire value, but never infer stutters or false starts from text alone.
    let cleaned = remove_leading_fillers(text);

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
        if next == current || !next.chars().any(char::is_alphanumeric) {
            break;
        }
        current = next;
    }

    current
}

fn strip_leading_filler(text: &str) -> Option<&str> {
    // Content words (这个/那个/就是说) and emotional interjections carry meaning.
    // Only a short hesitation followed by a soft pause and real content is eligible.
    for filler in ["嗯", "呃"] {
        if let Some(after) = text.strip_prefix(filler) {
            if after.chars().next().is_some_and(is_soft_pause) {
                return Some(after);
            }
        }
    }

    None
}

fn strip_leading_separators(text: &str) -> &str {
    text.trim_start_matches(is_soft_pause)
}

fn is_soft_pause(ch: char) -> bool {
    matches!(ch, ',' | '，' | ' ' | '\t' | '\u{3000}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_preserves_meaningful_speech_in_every_mode() {
        for mode in [DisfluencyMode::Conservative, DisfluencyMode::Aggressive] {
            for text in [
                "这个，给小王；那个，给小李。",
                "嗯。",
                "嗯，",
                "啊！太棒了",
                "哈哈哈，太好笑了",
                "好好好，就这么办",
                "我我我想打开设置",
                "我，那个，今天开会",
                "怎么说呢？请解释一下。",
                "嗯。明天再说。",
                "呃……",
            ] {
                let result = clean_disfluency(text, mode);
                assert_eq!(result.text, text, "mode={mode:?}, text={text}");
                assert!(!result.changed);
            }
        }
    }

    #[test]
    fn legacy_aggressive_setting_remains_readable_and_uses_safe_cleanup() {
        let mode: DisfluencyMode = serde_json::from_str("\"aggressive\"").unwrap();
        assert_eq!(serde_json::to_string(&mode).unwrap(), "\"aggressive\"");
        assert_eq!(clean_disfluency("呃，我准备好了", mode).text, "我准备好了");
        assert_eq!(clean_disfluency("哈哈哈", mode).text, "哈哈哈");
    }

    #[test]
    fn conservative_removes_sentence_start_filler_char() {
        let result = clean_disfluency("嗯，我准备好了", DisfluencyMode::Conservative);

        assert!(result.changed);
        assert_eq!(result.text, "我准备好了");
    }

    #[test]
    fn conservative_keeps_ambiguous_start_phrase() {
        let result = clean_disfluency("这个，我准备好了", DisfluencyMode::Conservative);

        assert!(!result.changed);
        assert_eq!(result.text, "这个，我准备好了");
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
}
