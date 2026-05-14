use std::ops::Range;

use crate::personalization::phonetic_keys::{build_key_bundle, PhoneticKeyBundle};

use super::tokenizer::{Token, TokenType, Tokenizer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    Cn,
    En,
}

#[derive(Debug, Clone)]
pub(crate) struct PhoneticToken {
    pub byte_range: Range<usize>,
    pub lang: Lang,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowKey {
    pub byte_range: Range<usize>,
    pub text: String,
    pub keys: PhoneticKeyBundle,
    pub has_chinese: bool,
    pub has_ascii: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SyllableLattice {
    pub source_text: String,
    pub tokens: Vec<PhoneticToken>,
}

impl SyllableLattice {
    pub(crate) fn from_asr_text(text: &str) -> Self {
        let tokens = Tokenizer::tokenize(text)
            .into_iter()
            .flat_map(content_tokens)
            .collect();

        Self {
            source_text: text.to_string(),
            tokens,
        }
    }

    pub(crate) fn windows(&self, max_size: usize) -> Vec<WindowKey> {
        if max_size == 0 || self.tokens.is_empty() {
            return Vec::new();
        }

        let mut windows = Vec::new();
        for start_idx in 0..self.tokens.len() {
            let end_limit = (start_idx + max_size).min(self.tokens.len());
            for end_idx in start_idx..end_limit {
                if end_idx > start_idx
                    && has_blocking_separator(
                        &self.source_text[self.tokens[end_idx - 1].byte_range.end
                            ..self.tokens[end_idx].byte_range.start],
                    )
                {
                    break;
                }

                let start = self.tokens[start_idx].byte_range.start;
                let end = self.tokens[end_idx].byte_range.end;
                if start >= end || end > self.source_text.len() {
                    continue;
                }

                let text = self.source_text[start..end].to_string();
                let token_slice = &self.tokens[start_idx..=end_idx];
                let has_chinese = token_slice.iter().any(|token| token.lang == Lang::Cn);
                let has_ascii = token_slice.iter().any(|token| token.lang == Lang::En);

                windows.push(WindowKey {
                    byte_range: start..end,
                    keys: build_key_bundle(&text),
                    text,
                    has_chinese,
                    has_ascii,
                });
            }
        }

        windows
    }
}

fn content_tokens(token: Token) -> Vec<PhoneticToken> {
    match token.token_type {
        TokenType::Chinese => token
            .text
            .char_indices()
            .map(|(offset, ch)| {
                let start = token.start + offset;
                PhoneticToken {
                    byte_range: start..start + ch.len_utf8(),
                    lang: Lang::Cn,
                }
            })
            .collect(),
        TokenType::Ascii => vec![PhoneticToken {
            byte_range: token.start..token.end,
            lang: Lang::En,
        }],
        TokenType::Whitespace | TokenType::Symbol => Vec::new(),
    }
}

fn has_blocking_separator(gap: &str) -> bool {
    gap.chars()
        .any(|ch| !ch.is_whitespace() && !is_safe_window_joiner(ch))
}

fn is_safe_window_joiner(ch: char) -> bool {
    matches!(ch, '-' | '_' | '/' | '\\' | '\'' | '’')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_content_tokens_without_whitespace_or_symbols() {
        let lattice = SyllableLattice::from_asr_text("我打开 克劳德 code。");

        assert_eq!(
            lattice
                .tokens
                .iter()
                .map(|token| &lattice.source_text[token.byte_range.clone()])
                .collect::<Vec<_>>(),
            vec!["我", "打", "开", "克", "劳", "德", "code"]
        );
    }

    #[test]
    fn windows_include_cross_language_alias_keys() {
        let lattice = SyllableLattice::from_asr_text("我打开 克劳德 code");
        let windows = lattice.windows(5);
        let window = windows
            .iter()
            .find(|window| window.text == "克劳德 code")
            .expect("mixed window");

        assert!(window.has_chinese);
        assert!(window.has_ascii);
        assert!(window
            .keys
            .alias_keys
            .iter()
            .any(|key| key == "kelaode|code"));
        assert!(window
            .keys
            .mixed_keys
            .iter()
            .any(|key| key == "kelaode|code"));
    }

    #[test]
    fn windows_include_cross_language_alias_without_whitespace_before_chinese_name() {
        let lattice = SyllableLattice::from_asr_text("我打开克劳德 code");
        let windows = lattice.windows(5);
        let window = windows
            .iter()
            .find(|window| window.text == "克劳德 code")
            .expect("mixed window without leading whitespace");

        assert!(window.has_chinese);
        assert!(window.has_ascii);
        assert!(window
            .keys
            .alias_keys
            .iter()
            .any(|key| key == "kelaode|code"));
    }

    #[test]
    fn windows_do_not_cross_sentence_punctuation() {
        let lattice = SyllableLattice::from_asr_text("先说 cloud。code 再继续");
        let windows = lattice.windows(5);

        assert!(!windows.iter().any(|window| window.text == "cloud。code"));
        assert!(windows.iter().any(|window| window.text == "cloud"));
        assert!(windows.iter().any(|window| window.text == "code"));
    }

    #[test]
    fn windows_can_cross_safe_joiners() {
        let lattice = SyllableLattice::from_asr_text("打开 cloud-code");
        let windows = lattice.windows(5);

        assert!(windows.iter().any(|window| window.text == "cloud-code"));
    }

    #[test]
    fn pure_ascii_windows_include_english_phonetic_keys() {
        let lattice = SyllableLattice::from_asr_text("我打开 claud code");
        let windows = lattice.windows(5);
        let window = windows
            .iter()
            .find(|window| window.text == "claud code")
            .expect("ascii window");

        assert!(window.has_ascii);
        assert!(!window.has_chinese);
        assert!(!window.keys.en_phonetic_keys.is_empty());
    }

    #[test]
    fn window_count_is_bounded_by_max_size() {
        let lattice = SyllableLattice::from_asr_text("one two three four five six");
        let windows = lattice.windows(3);

        assert_eq!(windows.len(), 15);
        assert!(windows
            .iter()
            .all(|window| window.text.split_whitespace().count() <= 3));
    }
}
