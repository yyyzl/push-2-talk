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
            .filter_map(content_token)
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

fn content_token(token: Token) -> Option<PhoneticToken> {
    let lang = match token.token_type {
        TokenType::Chinese => Lang::Cn,
        TokenType::Ascii => Lang::En,
        TokenType::Whitespace | TokenType::Symbol => return None,
    };

    Some(PhoneticToken {
        byte_range: token.start..token.end,
        lang,
    })
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
            vec!["我打开", "克劳德", "code"]
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
