//! TNL 分词器
//!
//! 将输入文本分割为：汉字、ASCII 词、空白、符号

/// Token 类型
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    /// 汉字序列
    Chinese,
    /// ASCII 字母/数字序列
    Ascii,
    /// 空白符序列
    Whitespace,
    /// 标点/符号
    Symbol,
}

/// Token
#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub token_type: TokenType,
    /// 在原文中的起始字符索引
    pub start: usize,
    /// 在原文中的结束字符索引（不含）
    pub end: usize,
}

/// 分词器
pub struct Tokenizer;

impl Tokenizer {
    /// 分词
    ///
    /// 按字符类型将文本分割为 Token 序列
    pub fn tokenize(text: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut current_type: Option<TokenType> = None;
        let mut current_start = 0;
        let mut current_text = String::new();

        for (idx, ch) in text.char_indices() {
            let char_type = Self::classify_char(ch);

            if let Some(ref ct) = current_type {
                if ct == &char_type {
                    current_text.push(ch);
                } else {
                    // 类型切换，保存当前 token
                    tokens.push(Token {
                        text: current_text.clone(),
                        token_type: ct.clone(),
                        start: current_start,
                        end: idx,
                    });
                    current_text.clear();
                    current_text.push(ch);
                    current_start = idx;
                    current_type = Some(char_type);
                }
            } else {
                current_text.push(ch);
                current_start = idx;
                current_type = Some(char_type);
            }
        }

        // 处理最后一个 token
        if !current_text.is_empty() {
            if let Some(ct) = current_type {
                tokens.push(Token {
                    text: current_text,
                    token_type: ct,
                    start: current_start,
                    end: text.len(),
                });
            }
        }

        tokens
    }

    /// 字符分类
    fn classify_char(ch: char) -> TokenType {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            TokenType::Ascii
        } else if ch.is_whitespace() {
            TokenType::Whitespace
        } else if Self::is_cjk(ch) {
            TokenType::Chinese
        } else {
            TokenType::Symbol
        }
    }

    /// 判断是否为 CJK 字符
    fn is_cjk(ch: char) -> bool {
        let code = ch as u32;
        // CJK Unified Ideographs
        (0x4E00..=0x9FFF).contains(&code)
            // CJK Unified Ideographs Extension A
            || (0x3400..=0x4DBF).contains(&code)
            // CJK Unified Ideographs Extension B-F
            || (0x20000..=0x2CEAF).contains(&code)
            // CJK Compatibility Ideographs
            || (0xF900..=0xFAFF).contains(&code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_mixed() {
        let tokens = Tokenizer::tokenize("readme 点 md");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].text, "readme");
        assert_eq!(tokens[0].token_type, TokenType::Ascii);
        assert_eq!(tokens[1].token_type, TokenType::Whitespace);
        assert_eq!(tokens[2].text, "点");
        assert_eq!(tokens[2].token_type, TokenType::Chinese);
    }

    #[test]
    fn test_tokenize_path() {
        let tokens = Tokenizer::tokenize("src/lib.rs");
        // src / lib . rs
        assert!(tokens.iter().any(|t| t.text == "src"));
        assert!(tokens.iter().any(|t| t.text == "/"));
        assert!(tokens.iter().any(|t| t.text == "lib"));
    }
}
