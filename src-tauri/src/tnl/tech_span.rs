//! TNL 技术片段识别
//!
//! 识别文件名、路径、版本号等技术串

use crate::tnl::is_ascii_digits;
use crate::tnl::rules::ExtensionWhitelist;
use crate::tnl::tokenizer::{Token, TokenType};
use crate::tnl::types::{Span, SpanType};
use jieba_rs::Jieba;
use std::collections::HashSet;

const USER_TERM_FREQUENCY: usize = 10_000_000;

/// 搜索方向
#[derive(Copy, Clone, Debug)]
enum Direction {
    Forward,
    Backward,
}

/// 技术片段识别器
pub struct TechSpanDetector {
    ext_whitelist: ExtensionWhitelist,
    jieba: Jieba,
    user_terms: HashSet<String>,
}

impl TechSpanDetector {
    pub fn new(ext_whitelist: ExtensionWhitelist) -> Self {
        Self::new_with_user_dictionary(ext_whitelist, &[])
    }

    pub fn new_with_user_dictionary(
        ext_whitelist: ExtensionWhitelist,
        dictionary: &[String],
    ) -> Self {
        let mut jieba = Jieba::new();
        let user_terms = Self::normalize_user_terms(dictionary);
        for term in &user_terms {
            jieba.add_word(term, Some(USER_TERM_FREQUENCY), Some("nz"));
        }

        Self {
            ext_whitelist,
            jieba,
            user_terms,
        }
    }

    /// 检测技术片段
    ///
    /// 返回识别到的技术片段列表
    pub fn detect(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 策略 1: 检测文件名模式 (xxx.ext)
        spans.extend(self.detect_file_names(text, tokens));

        // 策略 2: 检测路径模式 (包含 / 或 \)
        spans.extend(self.detect_paths(text, tokens));

        // 策略 3: 检测版本号模式 (v1.2.3 或 1.2.3)
        spans.extend(self.detect_versions(text, tokens));

        // 策略 4: 检测邮箱模式 (xxx@xxx.xxx 或 xxx艾特xxx点xxx)
        spans.extend(self.detect_emails(text, tokens));

        // 策略 5: 检测 URL/域名 (https://..., www..., github 点 com)
        spans.extend(self.detect_urls(text, tokens));

        // 策略 6: 检测 CLI flag (--verbose, -p)
        spans.extend(self.detect_cli_flags(text, tokens));

        // 策略 7: 检测驼峰/帕斯卡标识符
        spans.extend(self.detect_identifiers(text, tokens));

        // 策略 8: 检测十六进制/颜色码 (0xDEAD, #FF5733)
        spans.extend(self.detect_hex_values(text, tokens));

        // 策略 9: 检测包名/模块名 (@vue/cli, @types/node)
        spans.extend(self.detect_packages(text, tokens));

        // 策略 10: 使用注入用户词的分词器标记专名
        spans.extend(self.detect_user_named_entities(text));

        // 去重并合并重叠片段
        self.merge_overlapping(text, spans)
    }

    fn normalize_user_terms(dictionary: &[String]) -> HashSet<String> {
        dictionary
            .iter()
            .filter_map(|entry| {
                let word = entry.split('|').next().unwrap_or(entry.as_str()).trim();
                if Self::is_user_term_candidate(word) {
                    Some(word.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn is_user_term_candidate(word: &str) -> bool {
        word.chars().count() >= 2
            && word
                .chars()
                .any(|ch| !ch.is_whitespace() && !ch.is_ascii_punctuation())
    }

    fn detect_user_named_entities(&self, text: &str) -> Vec<Span> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut spans = Vec::new();
        let mut cursor = 0;
        for tagged_word in self.jieba.tag(text, false) {
            let word = tagged_word.word;
            if word.is_empty() {
                continue;
            }

            let Some(relative_start) = text[cursor..].find(word) else {
                continue;
            };
            let start = cursor + relative_start;
            let end = start + word.len();
            if self.should_emit_named_entity(word, tagged_word.tag) {
                spans.push(Span {
                    text: word.to_string(),
                    start,
                    end,
                    span_type: SpanType::NamedEntity,
                });
            }
            cursor = end;
        }

        spans
    }

    fn should_emit_named_entity(&self, word: &str, tag: &str) -> bool {
        Self::is_user_term_candidate(word)
            && (self.user_terms.contains(word) || Self::is_named_entity_tag(tag))
    }

    fn is_named_entity_tag(tag: &str) -> bool {
        matches!(tag, "nr" | "ns" | "nt" | "nz")
    }

    /// 检测文件名模式
    fn detect_file_names(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找 "xxx . ext" 或 "xxx 点 ext" 模式
        for (i, token) in tokens.iter().enumerate() {
            // 查找点号或口语"点"
            let is_dot = token.text == "." || token.text == "点" || token.text == "點";
            if !is_dot {
                continue;
            }

            // 向前查找文件名部分（跳过空白）
            let prev_idx = self.find_adjacent_ascii(tokens, i, Direction::Backward);
            // 向后查找扩展名部分（跳过空白）
            let next_idx = self.find_adjacent_ascii(tokens, i, Direction::Forward);

            if let (Some(prev), Some(next)) = (prev_idx, next_idx) {
                let ext = &tokens[next].text;
                if self.ext_whitelist.contains(ext) {
                    let start = tokens[prev].start;
                    let end = tokens[next].end;
                    spans.push(Span {
                        text: text[start..end].to_string(),
                        start,
                        end,
                        span_type: SpanType::FileName,
                    });
                }
            }
        }

        spans
    }

    /// 检测路径模式
    fn detect_paths(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找包含 / 或 \ 或 口语"斜杠" 的序列
        let mut path_start: Option<usize> = None;
        let mut last_path_end: Option<usize> = None;

        for (i, token) in tokens.iter().enumerate() {
            let is_path_sep = token.text == "/"
                || token.text == "\\"
                || token.text == "斜杠"
                || token.text == "斜线";

            if is_path_sep {
                if path_start.is_none() {
                    // 向前扩展到 ASCII token
                    if let Some(prev) = self.find_adjacent_ascii(tokens, i, Direction::Backward) {
                        path_start = Some(tokens[prev].start);
                    }
                }
                // 向后扩展
                if let Some(next) = self.find_adjacent_ascii(tokens, i, Direction::Forward) {
                    last_path_end = Some(tokens[next].end);
                }
            } else if path_start.is_some()
                && token.token_type != TokenType::Ascii
                && token.token_type != TokenType::Whitespace
                && token.text != "."
                && token.text != "点"
            {
                // 遇到非路径字符，结束当前路径
                if let (Some(start), Some(end)) = (path_start, last_path_end) {
                    if end > start {
                        spans.push(Span {
                            text: text[start..end].to_string(),
                            start,
                            end,
                            span_type: SpanType::Path,
                        });
                    }
                }
                path_start = None;
                last_path_end = None;
            }
        }

        // 处理末尾的路径
        if let (Some(start), Some(end)) = (path_start, last_path_end) {
            if end > start {
                spans.push(Span {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    span_type: SpanType::Path,
                });
            }
        }

        spans
    }

    /// 检测版本号模式
    fn detect_versions(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找 v1.2.3 或 1.2.3 模式
        for (i, token) in tokens.iter().enumerate() {
            // 检查是否以 v/V 开头或纯数字
            let starts_version = token.text.starts_with('v')
                || token.text.starts_with('V')
                || token
                    .text
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false);

            if !starts_version || token.token_type != TokenType::Ascii {
                continue;
            }

            // 向后查找 .数字 或 点 数字 序列
            let mut version_end = token.end;
            let mut j = i + 1;
            let mut has_dot = false;

            while j < tokens.len() {
                let t = &tokens[j];

                // 跳过空白
                if t.token_type == TokenType::Whitespace {
                    j += 1;
                    continue;
                }

                // 检查点号
                if t.text == "." || t.text == "点" || t.text == "點" {
                    has_dot = true;
                    j += 1;
                    continue;
                }

                // 检查数字
                if has_dot
                    && t.token_type == TokenType::Ascii
                    && t.text
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '-' || c.is_ascii_alphabetic())
                {
                    version_end = t.end;
                    has_dot = false; // 重置，等待下一个点
                    j += 1;
                    continue;
                }

                break;
            }

            // 如果有扩展（至少一个点+数字），则认为是版本号
            if version_end > token.end {
                spans.push(Span {
                    text: text[token.start..version_end].to_string(),
                    start: token.start,
                    end: version_end,
                    span_type: SpanType::Version,
                });
            }
        }

        spans
    }

    /// 检测邮箱模式
    ///
    /// 识别 xxx@xxx.xxx 或 xxx艾特xxx点xxx 模式
    fn detect_emails(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找 @ 或 "艾特" 或 "at"
        for (i, token) in tokens.iter().enumerate() {
            let is_at =
                token.text == "@" || token.text == "艾特" || token.text.eq_ignore_ascii_case("at");
            if !is_at {
                continue;
            }

            // 向前查找用户名部分（扩展到所有连续 ASCII，跳过空白）
            let prev_idx = self.find_email_username_start(tokens, i);
            // 向后查找域名部分
            let domain_result = self.find_email_domain(tokens, i);

            if let (Some(prev), Some((domain_end_idx, has_dot))) = (prev_idx, domain_result) {
                // 邮箱必须有点号（xxx.com）
                if has_dot {
                    let start = tokens[prev].start;
                    let end = tokens[domain_end_idx].end;
                    spans.push(Span {
                        text: text[start..end].to_string(),
                        start,
                        end,
                        span_type: SpanType::Email,
                    });
                }
            }
        }

        spans
    }

    /// 检测 URL/域名模式
    fn detect_urls(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 场景 1：协议开头（http:// 或 https://）
        for (i, token) in tokens.iter().enumerate() {
            if token.token_type != TokenType::Ascii {
                continue;
            }
            if !token.text.eq_ignore_ascii_case("http") && !token.text.eq_ignore_ascii_case("https")
            {
                continue;
            }

            let Some(domain_start_idx) = self.find_ascii_after_scheme(tokens, i) else {
                continue;
            };

            if let Some(domain_end_idx) = self.find_domain_end(tokens, domain_start_idx) {
                let start = token.start;
                let end = tokens[domain_end_idx].end;
                spans.push(Span {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    span_type: SpanType::Url,
                });
            }
        }

        // 场景 2：域名/口语域名（www.example.com / github 点 com）
        for (i, token) in tokens.iter().enumerate() {
            if token.token_type != TokenType::Ascii {
                continue;
            }

            // 避免与邮箱域名冲突（example.com in test@example.com）
            if self.is_after_email_at(tokens, i) {
                continue;
            }

            if let Some(domain_end_idx) = self.find_domain_end(tokens, i) {
                let start = token.start;
                let end = tokens[domain_end_idx].end;
                spans.push(Span {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    span_type: SpanType::Url,
                });
            }
        }

        spans
    }

    /// 检测 CLI flag（--verbose / -p）
    fn detect_cli_flags(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        for (i, token) in tokens.iter().enumerate() {
            // 形式 A："--" + "verbose"（例如 "--verbose"）
            if token.text == "--" {
                let Some(flag_idx) = self.find_next_non_whitespace(tokens, i) else {
                    continue;
                };
                let flag_token = &tokens[flag_idx];
                if flag_token.token_type == TokenType::Ascii
                    && flag_token.text.len() >= 2
                    && flag_token
                        .text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    let start = token.start;
                    let end = flag_token.end;
                    spans.push(Span {
                        text: text[start..end].to_string(),
                        start,
                        end,
                        span_type: SpanType::CliFlag,
                    });
                }
                continue;
            }

            if token.text != "-" {
                continue;
            }

            let Some(next_idx) = self.find_next_non_whitespace(tokens, i) else {
                continue;
            };

            // 形式 B："-" + "-" + "verbose"（含可选空格）
            if tokens[next_idx].text == "-" {
                let Some(flag_idx) = self.find_next_non_whitespace(tokens, next_idx) else {
                    continue;
                };

                let flag_token = &tokens[flag_idx];
                if flag_token.token_type == TokenType::Ascii
                    && flag_token.text.len() >= 2
                    && flag_token
                        .text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    let start = token.start;
                    let end = flag_token.end;
                    spans.push(Span {
                        text: text[start..end].to_string(),
                        start,
                        end,
                        span_type: SpanType::CliFlag,
                    });
                }
                continue;
            }

            // 形式 C："-" + "p"
            let flag_token = &tokens[next_idx];
            if flag_token.token_type == TokenType::Ascii
                && flag_token.text.len() == 1
                && flag_token.text.chars().all(|c| c.is_ascii_alphabetic())
            {
                let start = token.start;
                let end = flag_token.end;
                spans.push(Span {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    span_type: SpanType::CliFlag,
                });
            }
        }

        spans
    }

    /// 检测十六进制值和颜色哈希
    ///
    /// 模式：
    /// - 0x 前缀：0x + 2~16 位十六进制字符（0xDEAD, 0xFF）
    /// - # 前缀：# + 恰好 3/4/6/8 位十六进制字符（#FF5733, #FFF）
    fn detect_hex_values(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        for (i, token) in tokens.iter().enumerate() {
            // 场景 1：0x 前缀（整个 token 就是一个 Ascii token，如 0xDEADBEEF）
            if token.token_type == TokenType::Ascii {
                let t = &token.text;
                if (t.starts_with("0x") || t.starts_with("0X")) && t.len() >= 4 {
                    // 0x 后面至少 2 位，最多 16 位
                    let hex_part = &t[2..];
                    if hex_part.len() >= 2
                        && hex_part.len() <= 16
                        && hex_part.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        spans.push(Span {
                            text: text[token.start..token.end].to_string(),
                            start: token.start,
                            end: token.end,
                            span_type: SpanType::Technical,
                        });
                    }
                }
            }

            // 场景 2：# 前缀颜色码
            if token.text != "#" {
                continue;
            }

            // 向后找紧邻的 Ascii token（跳过空白）
            let Some(next_idx) = self.find_adjacent_ascii(tokens, i, Direction::Forward) else {
                continue;
            };

            let hex_token = &tokens[next_idx];
            let hex_text = &hex_token.text;

            // 检查是否全部为十六进制字符，且长度为 3/4/6/8
            let valid_len = matches!(hex_text.len(), 3 | 4 | 6 | 8);
            if valid_len && hex_text.chars().all(|c| c.is_ascii_hexdigit()) {
                let start = token.start;
                let end = hex_token.end;
                spans.push(Span {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    span_type: SpanType::Technical,
                });
            }
        }

        spans
    }

    /// 检测包名/模块名
    ///
    /// 模式：
    /// - @scope/name: 符号@ + ASCII + 符号/ + ASCII（如 @vue/cli, @types/node）
    ///   区分邮箱：包名的 @ 前面没有紧邻的 ASCII token
    fn detect_packages(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        for (i, token) in tokens.iter().enumerate() {
            if token.text != "@" {
                continue;
            }

            // 区分邮箱：如果 @ 前面紧邻 ASCII token，则是邮箱，跳过
            let has_prev_ascii = if i > 0 {
                self.find_adjacent_ascii(tokens, i, Direction::Backward)
                    .is_some()
            } else {
                false
            };
            if has_prev_ascii {
                continue;
            }

            // 向后查找 scope（跳过空白）
            let Some(scope_idx) = self.find_adjacent_ascii(tokens, i, Direction::Forward) else {
                continue;
            };

            // 查找 / 分隔符
            let Some(slash_idx) = self.find_next_non_whitespace(tokens, scope_idx) else {
                continue;
            };
            if tokens[slash_idx].text != "/" {
                continue;
            }

            // 查找 name
            let Some(name_idx) = self.find_adjacent_ascii(tokens, slash_idx, Direction::Forward)
            else {
                continue;
            };

            let start = token.start;
            let end = tokens[name_idx].end;
            spans.push(Span {
                text: text[start..end].to_string(),
                start,
                end,
                span_type: SpanType::Technical,
            });
        }

        spans
    }

    /// 检测驼峰/帕斯卡命名标识符
    fn detect_identifiers(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        for token in tokens {
            if token.token_type != TokenType::Ascii {
                continue;
            }

            if self.is_identifier_token(&token.text) {
                spans.push(Span {
                    text: text[token.start..token.end].to_string(),
                    start: token.start,
                    end: token.end,
                    span_type: SpanType::Identifier,
                });
            }
        }

        spans
    }

    /// 查找邮箱用户名的起始位置（向前扩展连续数字段）
    ///
    /// 支持 "10455 3588 艾特" 这种多个数字段的口语输入
    /// 但仅当紧邻 @ 前的 ASCII token 为纯数字时才向前扩展
    fn find_email_username_start(&self, tokens: &[Token], from: usize) -> Option<usize> {
        if from == 0 {
            return None;
        }

        // 先找到紧邻 @ 前的 ASCII token
        let first_ascii_idx = self.find_adjacent_ascii(tokens, from, Direction::Backward)?;
        let first_token = &tokens[first_ascii_idx];

        // 如果不是纯数字，直接返回（不向前扩展）
        if !is_ascii_digits(&first_token.text) {
            return Some(first_ascii_idx);
        }

        // 是纯数字，继续向前扩展到所有连续的纯数字 ASCII token
        let mut earliest_ascii_idx = first_ascii_idx;

        if first_ascii_idx == 0 {
            return Some(earliest_ascii_idx);
        }

        for i in (0..first_ascii_idx).rev() {
            let t = &tokens[i];

            // 跳过空白
            if t.token_type == TokenType::Whitespace {
                continue;
            }

            // 纯数字 ASCII token，记录并继续向前
            if t.token_type == TokenType::Ascii && is_ascii_digits(&t.text) {
                earliest_ascii_idx = i;
                continue;
            }

            // 遇到非数字/非空白，停止
            break;
        }

        Some(earliest_ascii_idx)
    }

    /// 查找邮箱域名部分
    ///
    /// 返回 (结束 token 索引, 是否包含点号)
    fn find_email_domain(&self, tokens: &[Token], from: usize) -> Option<(usize, bool)> {
        let mut last_ascii_idx: Option<usize> = None;
        let mut has_dot = false;
        let mut j = from + 1;

        while j < tokens.len() {
            let t = &tokens[j];

            // 跳过空白
            if t.token_type == TokenType::Whitespace {
                j += 1;
                continue;
            }

            // 检查点号或口语"点"
            if t.text == "." || t.text == "点" || t.text == "點" {
                has_dot = true;
                j += 1;
                continue;
            }

            // 检查 ASCII（域名部分）
            if t.token_type == TokenType::Ascii {
                last_ascii_idx = Some(j);
                j += 1;
                continue;
            }

            // 遇到其他字符，结束
            break;
        }

        last_ascii_idx.map(|idx| (idx, has_dot))
    }

    /// 查找邻近的 ASCII token（跳过空白）
    fn find_adjacent_ascii(
        &self,
        tokens: &[Token],
        from: usize,
        direction: Direction,
    ) -> Option<usize> {
        match direction {
            Direction::Backward => {
                if from == 0 {
                    return None;
                }
                for i in (0..from).rev() {
                    if tokens[i].token_type == TokenType::Whitespace {
                        continue;
                    }
                    if tokens[i].token_type == TokenType::Ascii {
                        return Some(i);
                    }
                    break;
                }
                None
            }
            Direction::Forward => {
                for i in (from + 1)..tokens.len() {
                    if tokens[i].token_type == TokenType::Whitespace {
                        continue;
                    }
                    if tokens[i].token_type == TokenType::Ascii {
                        return Some(i);
                    }
                    break;
                }
                None
            }
        }
    }

    fn find_next_non_whitespace(&self, tokens: &[Token], from: usize) -> Option<usize> {
        for i in (from + 1)..tokens.len() {
            if tokens[i].token_type != TokenType::Whitespace {
                return Some(i);
            }
        }
        None
    }

    fn find_prev_non_whitespace(&self, tokens: &[Token], from: usize) -> Option<usize> {
        if from == 0 {
            return None;
        }

        for i in (0..from).rev() {
            if tokens[i].token_type != TokenType::Whitespace {
                return Some(i);
            }
        }
        None
    }

    fn is_dot_text(&self, text: &str) -> bool {
        text == "." || text == "点" || text == "點"
    }

    fn is_after_email_at(&self, tokens: &[Token], idx: usize) -> bool {
        let Some(prev_idx) = self.find_prev_non_whitespace(tokens, idx) else {
            return false;
        };

        let prev = &tokens[prev_idx].text;
        prev == "@" || prev == "艾特" || prev.eq_ignore_ascii_case("at")
    }

    fn is_url_tld(&self, s: &str) -> bool {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "com"
                | "org"
                | "net"
                | "io"
                | "dev"
                | "app"
                | "ai"
                | "co"
                | "cn"
                | "edu"
                | "gov"
                | "tech"
                | "xyz"
                | "me"
                | "us"
                | "uk"
        )
    }

    fn find_ascii_after_scheme(&self, tokens: &[Token], from: usize) -> Option<usize> {
        let mut i = from + 1;
        while i < tokens.len() {
            let t = &tokens[i];

            if t.token_type == TokenType::Whitespace {
                i += 1;
                continue;
            }

            // 允许 http(s) 后的分隔符（: /）
            if t.text == ":" || t.text == "/" {
                i += 1;
                continue;
            }

            if t.token_type == TokenType::Ascii {
                return Some(i);
            }

            break;
        }

        None
    }

    fn find_domain_end(&self, tokens: &[Token], start: usize) -> Option<usize> {
        if start >= tokens.len() || tokens[start].token_type != TokenType::Ascii {
            return None;
        }

        let mut j = start + 1;
        let mut last_ascii_idx = start;
        let mut saw_dot = false;
        let mut last_label_is_tld = self.is_url_tld(&tokens[start].text);

        while j < tokens.len() {
            // 跳过空白
            if tokens[j].token_type == TokenType::Whitespace {
                j += 1;
                continue;
            }

            // 允许点分隔（. 或 口语点）
            if self.is_dot_text(&tokens[j].text) {
                saw_dot = true;
                j += 1;

                while j < tokens.len() && tokens[j].token_type == TokenType::Whitespace {
                    j += 1;
                }

                if j >= tokens.len() || tokens[j].token_type != TokenType::Ascii {
                    break;
                }

                last_ascii_idx = j;
                last_label_is_tld = self.is_url_tld(&tokens[j].text);
                j += 1;
                continue;
            }

            // 允许域名 label 内部连字符
            if tokens[j].text == "-" {
                j += 1;
                while j < tokens.len() && tokens[j].token_type == TokenType::Whitespace {
                    j += 1;
                }

                if j < tokens.len() && tokens[j].token_type == TokenType::Ascii {
                    last_ascii_idx = j;
                    // 连字符表示仍在同一 label 内，结尾已不再是纯 TLD
                    last_label_is_tld = false;
                    j += 1;
                    continue;
                }
            }

            break;
        }

        if saw_dot && last_label_is_tld {
            Some(last_ascii_idx)
        } else {
            None
        }
    }

    fn is_identifier_token(&self, token: &str) -> bool {
        if token.len() < 6 {
            return false;
        }

        if !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }

        let mut has_upper = false;
        let mut has_lower = false;
        let mut transitions = 0;
        let mut has_camel_boundary = false;
        let mut prev_alpha_is_upper: Option<bool> = None;

        for ch in token.chars() {
            let is_upper = if ch.is_ascii_uppercase() {
                has_upper = true;
                Some(true)
            } else if ch.is_ascii_lowercase() {
                has_lower = true;
                Some(false)
            } else {
                None
            };

            if let (Some(prev), Some(curr)) = (prev_alpha_is_upper, is_upper) {
                if prev != curr {
                    transitions += 1;
                }
                if !prev && curr {
                    has_camel_boundary = true;
                }
            }

            if is_upper.is_some() {
                prev_alpha_is_upper = is_upper;
            }
        }

        let upper_prefix_len = token.chars().take_while(|c| c.is_ascii_uppercase()).count();
        let has_acronym_prefix = upper_prefix_len >= 2
            && token
                .chars()
                .nth(upper_prefix_len)
                .map(|c| c.is_ascii_lowercase())
                .unwrap_or(false);

        has_upper && has_lower && (transitions >= 2 || has_camel_boundary || has_acronym_prefix)
    }

    fn span_priority(&self, span_type: &SpanType) -> u8 {
        match span_type {
            SpanType::Path => 8,
            SpanType::Url => 7,
            SpanType::FileName => 6,
            SpanType::Email => 5,
            SpanType::CliFlag => 4,
            SpanType::Identifier => 3,
            SpanType::Version => 2,
            SpanType::NamedEntity => 2,
            SpanType::Technical => 1,
        }
    }

    /// 合并重叠片段
    fn merge_overlapping(&self, text: &str, mut spans: Vec<Span>) -> Vec<Span> {
        if spans.is_empty() {
            return spans;
        }

        // 按起始位置排序
        spans.sort_by_key(|s| (s.start, s.end));

        let mut merged = Vec::new();
        let mut current = spans.remove(0);

        for span in spans {
            if span.start <= current.end {
                // 重叠，合并
                if span.end > current.end {
                    current.end = span.end;
                }

                if self.span_priority(&span.span_type) > self.span_priority(&current.span_type) {
                    current.span_type = span.span_type.clone();
                }

                current.text = text[current.start..current.end].to_string();
            } else {
                current.text = text[current.start..current.end].to_string();
                merged.push(current);
                current = span;
            }
        }
        current.text = text[current.start..current.end].to_string();
        merged.push(current);

        merged
    }
}

impl Default for TechSpanDetector {
    fn default() -> Self {
        Self::new(ExtensionWhitelist::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tnl::tokenizer::Tokenizer;

    #[test]
    fn test_detect_filename() {
        let detector = TechSpanDetector::default();
        let text = "修改了 readme 点 md 文件";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.span_type == SpanType::FileName));
    }

    #[test]
    fn test_detect_path() {
        let detector = TechSpanDetector::default();
        let text = "打开 src 斜杠 lib 点 rs";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
    }

    #[test]
    fn test_detect_version() {
        let detector = TechSpanDetector::default();
        let text = "升级到 v1 点 2 点 3";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.span_type == SpanType::Version));
    }

    #[test]
    fn test_detect_email() {
        let detector = TechSpanDetector::default();

        // 测试口语邮箱
        let text = "1045535878 艾特 qq 点 com";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.span_type == SpanType::Email));

        // 测试标准邮箱
        let text2 = "test@example.com";
        let tokens2 = Tokenizer::tokenize(text2);
        let spans2 = detector.detect(text2, &tokens2);
        assert!(!spans2.is_empty());
        assert!(spans2.iter().any(|s| s.span_type == SpanType::Email));
    }

    #[test]
    fn test_detect_email_username_boundary() {
        let detector = TechSpanDetector::default();

        // 全英文句子中的邮箱，用户名不应向前扩展到整个句子
        let text = "my email is test @ example . com";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        // 应该检测到邮箱
        assert!(!spans.is_empty());
        let email_span = spans
            .iter()
            .find(|s| s.span_type == SpanType::Email)
            .unwrap();

        // 邮箱起点应该是 "test"，不是 "my"
        assert!(
            email_span.text.starts_with("test"),
            "邮箱应该从 'test' 开始，实际: {}",
            email_span.text
        );
    }

    #[test]
    fn test_detect_email_multi_digit_username() {
        let detector = TechSpanDetector::default();

        // 多段数字用户名
        let text = "10455 3588 艾特 qq 点 com";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
        let email_span = spans
            .iter()
            .find(|s| s.span_type == SpanType::Email)
            .unwrap();

        // 强化断言：span 起点应该从 "10455" 开始
        let expected_start = text.find("10455").unwrap();
        assert_eq!(
            email_span.start, expected_start,
            "邮箱 span 起点应为 {}，实际: {}",
            expected_start, email_span.start
        );

        // 强化断言：span 应该覆盖第二段数字 "3588"
        assert!(
            email_span.text.contains("3588"),
            "邮箱应该包含 '3588'，实际: {}",
            email_span.text
        );

        // 强化断言：span 应该覆盖到域名结尾
        assert!(
            email_span.text.ends_with("com"),
            "邮箱应该以 'com' 结尾，实际: {}",
            email_span.text
        );
    }

    #[test]
    fn test_detect_url() {
        let detector = TechSpanDetector::default();
        let text = "打开 github 点 com";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(spans.iter().any(|s| s.span_type == SpanType::Url));
    }

    #[test]
    fn test_detect_cli_flag() {
        let detector = TechSpanDetector::default();
        let text = "使用 --verbose 参数";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(spans.iter().any(|s| s.span_type == SpanType::CliFlag));
    }

    #[test]
    fn test_detect_identifier() {
        let detector = TechSpanDetector::default();
        let text = "调用 getElementById 获取节点";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(spans.iter().any(|s| s.span_type == SpanType::Identifier));
    }

    #[test]
    fn test_detect_identifier_with_acronym_prefix() {
        let detector = TechSpanDetector::default();
        let text = "解析 JSONParser 的输出";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(spans.iter().any(|s| s.span_type == SpanType::Identifier));
    }

    #[test]
    fn test_do_not_detect_plain_title_case_word_as_identifier() {
        let detector = TechSpanDetector::default();
        let text = "打开 Github 页面";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.iter().any(|s| s.span_type == SpanType::Identifier));
    }

    #[test]
    fn test_url_requires_tld_on_final_label() {
        let detector = TechSpanDetector::default();
        let text = "访问 com 点 internal";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.iter().any(|s| s.span_type == SpanType::Url));
    }

    // ===== P1-2: 包名检测测试 =====

    #[test]
    fn test_detect_package_scoped() {
        let detector = TechSpanDetector::default();
        let text = "安装 @vue/cli 工具";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        // @vue/cli 应被某个 span 覆盖（可能是 Path 或 Technical，因含 /）
        assert!(
            spans
                .iter()
                .any(|s| s.text.contains("@vue") || s.text.contains("vue/cli")),
            "@vue/cli 应被识别为技术片段，实际 spans: {:?}",
            spans
        );
    }

    #[test]
    fn test_detect_package_types() {
        let detector = TechSpanDetector::default();
        let text = "需要 @types/node 类型定义";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            spans
                .iter()
                .any(|s| s.text.contains("@types") || s.text.contains("types/node")),
            "@types/node 应被识别为技术片段"
        );
    }

    #[test]
    fn test_detect_package_does_not_match_email() {
        let detector = TechSpanDetector::default();
        let text = "test@example.com";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        // 应识别为邮箱，不应被包名检测器误匹配
        assert!(
            !spans
                .iter()
                .any(|s| s.span_type == SpanType::Technical && s.text.contains("@example")),
            "邮箱不应被误识别为包名"
        );
    }

    #[test]
    fn test_detect_user_dictionary_named_entity_with_jieba() {
        let detector = TechSpanDetector::new_with_user_dictionary(
            ExtensionWhitelist::new(),
            &["深度求索".to_string()],
        );
        let text = "我在用深度求索写代码";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(spans
            .iter()
            .any(|s| s.span_type == SpanType::NamedEntity && s.text == "深度求索"));
    }

    #[test]
    fn test_default_detector_does_not_mark_common_nouns_as_named_entity() {
        let detector = TechSpanDetector::default();
        let text = "今天天气很好";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.iter().any(|s| s.span_type == SpanType::NamedEntity));
    }

    #[test]
    fn test_detect_jieba_pos_named_entity_without_user_dictionary() {
        let detector = TechSpanDetector::default();
        let text = "我去了北京开会";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(spans
            .iter()
            .any(|s| s.span_type == SpanType::NamedEntity && s.text == "北京"));
    }

    #[test]
    fn test_named_entity_loses_to_existing_technical_span_priority() {
        let detector = TechSpanDetector::default();
        let text = "请发到 test@example.com";
        let start = text.find("test@example.com").unwrap();
        let end = start + "test@example.com".len();
        let spans = detector.merge_overlapping(
            text,
            vec![
                Span {
                    text: "test@example.com".to_string(),
                    start,
                    end,
                    span_type: SpanType::Email,
                },
                Span {
                    text: "test@example.com".to_string(),
                    start,
                    end,
                    span_type: SpanType::NamedEntity,
                },
            ],
        );

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].span_type, SpanType::Email);
    }

    // ===== P1-2: 十六进制检测测试 =====

    #[test]
    fn test_detect_hex_0x() {
        let detector = TechSpanDetector::default();
        let text = "地址是 0xDEADBEEF";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        // 0xDEADBEEF 可能被 Identifier 或 Technical 覆盖，只要被 span 保护即可
        assert!(
            spans.iter().any(|s| s.text.contains("0xDEADBEEF")),
            "0xDEADBEEF 应被识别为技术片段，实际 spans: {:?}",
            spans
        );
    }

    #[test]
    fn test_detect_hex_0x_short() {
        let detector = TechSpanDetector::default();
        let text = "值为 0xFF";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            spans.iter().any(|s| s.span_type == SpanType::Technical),
            "0xFF 应被识别为技术片段"
        );
    }

    #[test]
    fn test_detect_hex_color_6() {
        let detector = TechSpanDetector::default();
        let text = "颜色 #FF5733 很好看";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            spans.iter().any(|s| s.span_type == SpanType::Technical),
            "#FF5733 应被识别为技术片段，实际 spans: {:?}",
            spans
        );
    }

    #[test]
    fn test_detect_hex_color_3() {
        let detector = TechSpanDetector::default();
        let text = "用 #FFF 白色";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            spans.iter().any(|s| s.span_type == SpanType::Technical),
            "#FFF 应被识别为技术片段"
        );
    }

    #[test]
    fn test_detect_hex_color_8_rgba() {
        let detector = TechSpanDetector::default();
        let text = "透明色 #FF573380";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            spans.iter().any(|s| s.span_type == SpanType::Technical),
            "#FF573380 应被识别为技术片段"
        );
    }

    #[test]
    fn test_hex_color_invalid_length_not_detected() {
        let detector = TechSpanDetector::default();
        // 5 位十六进制不是有效颜色码
        let text = "测试 #ABCDE 值";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            !spans
                .iter()
                .any(|s| s.span_type == SpanType::Technical && s.text.contains("#ABCDE")),
            "#ABCDE (5位) 不应被识别为颜色码"
        );
    }

    #[test]
    fn test_hash_chinese_not_detected() {
        let detector = TechSpanDetector::default();
        // # 后面是中文，不应被误识别
        let text = "#标题内容";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(
            !spans
                .iter()
                .any(|s| s.span_type == SpanType::Technical && s.text.starts_with('#')),
            "#标题 不应被误识别为十六进制"
        );
    }
}
