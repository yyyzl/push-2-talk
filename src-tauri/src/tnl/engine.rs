//! TNL 主引擎
//!
//! 组合分词、技术片段识别、口语符号映射、模糊匹配

use std::collections::HashSet;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

use crate::tnl::disfluency::{clean_disfluency, DisfluencyMode};
use crate::tnl::fuzzy::{is_tech_token, FuzzyMatcher};
use crate::tnl::is_ascii_digits;
use crate::tnl::rules::{ExtensionWhitelist, SpokenSymbolMap};
use crate::tnl::tech_span::TechSpanDetector;
use crate::tnl::tokenizer::{Token, TokenType, Tokenizer};
use crate::tnl::types::{
    NormalizationResult, Replacement, ReplacementReason, Span, TnlCandidate, TnlCandidateDecision,
    TnlCandidateRisk, TnlCandidateSource, TnlDiagnostics,
};

const PENDING_LLM_MIN_SCORE: f32 = 0.68;
const LOCAL_APPLY_SCORE_CEILING: f32 = 0.88;
const REJECTED_LOCAL_MIN_SCORE: f32 = 0.55;
const MAX_DIAGNOSTIC_CANDIDATES: usize = 8;

/// 合并连续的空格分隔单字母为大写缩写
///
/// 规则：
/// - 检测 ≥2 个连续的、被空格分隔的单英文字母
/// - 合并为大写缩写（如 "T N L" → "TNL"，"U S B" → "USB"）
/// - 前后不能是字母（避免误匹配多字母单词的一部分）
///
/// 安全性：
/// - "I am here" → 不变（"I" 后面是多字母词 "am"）
/// - "LongCat Flash" → 不变（都是多字母词）
/// - "A I model" → "AI model"（连续两个单字母）
fn merge_spaced_letters(text: &str) -> (String, Vec<Replacement>) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();

    // 至少需要 "X Y"（3个字符）才可能有2个字母+空格
    if n < 3 {
        return (text.to_string(), Vec::new());
    }

    // 预计算每个字符位置的字节偏移
    let mut byte_offsets: Vec<usize> = Vec::with_capacity(n + 1);
    let mut offset = 0;
    for &ch in &chars {
        byte_offsets.push(offset);
        offset += ch.len_utf8();
    }
    byte_offsets.push(offset);

    let mut result = String::with_capacity(text.len());
    let mut replacements = Vec::new();
    let mut i = 0;

    while i < n {
        let ch = chars[i];

        // 检查是否是一个独立的单英文字母
        let prev_is_letter = i > 0 && chars[i - 1].is_ascii_alphabetic();
        let next_is_letter = i + 1 < n && chars[i + 1].is_ascii_alphabetic();

        if ch.is_ascii_alphabetic() && !prev_is_letter && !next_is_letter {
            // 找到一个单字母，尝试收集连续序列
            let seq_start = i;
            let mut letters = vec![ch];
            let mut end = i + 1; // 已确认消费的字符位置（不含）
            let mut j = i + 1;

            loop {
                // 跳过空格
                let space_start = j;
                while j < n && chars[j] == ' ' {
                    j += 1;
                }
                if j == space_start || j >= n {
                    break;
                }

                // 检查是否为单字母
                let next_after = j + 1 < n && chars[j + 1].is_ascii_alphabetic();
                let at_end = j + 1 >= n;
                if chars[j].is_ascii_alphabetic() && (at_end || !next_after) {
                    letters.push(chars[j]);
                    end = j + 1;
                    j += 1;
                } else {
                    break;
                }
            }

            if letters.len() >= 2 {
                let merged: String = letters.iter().map(|c| c.to_ascii_uppercase()).collect();
                let original: String = chars[seq_start..end].iter().collect();

                replacements.push(Replacement {
                    original,
                    replaced: merged.clone(),
                    start: byte_offsets[seq_start],
                    end: byte_offsets[end],
                    confidence: 1.0,
                    reason: ReplacementReason::LetterMerge,
                });

                result.push_str(&merged);
                i = end;
                continue;
            }
        }

        result.push(ch);
        i += 1;
    }

    (result, replacements)
}

/// 预计算每个 token 位置的"下一个非空白 token 是否为纯数字"
///
/// 复杂度 O(n)，从后向前扫描一次
fn precompute_next_is_digit(tokens: &[Token]) -> Vec<bool> {
    let n = tokens.len();
    let mut result = vec![false; n];
    let mut next_non_ws_is_digit = false;

    for i in (0..n).rev() {
        let t = &tokens[i];
        if t.token_type == TokenType::Whitespace {
            // 空白 token：继承后面的结果
            result[i] = next_non_ws_is_digit;
        } else {
            // 非空白 token：先记录当前结果，再更新状态
            result[i] = next_non_ws_is_digit;
            next_non_ws_is_digit = t.token_type == TokenType::Ascii && is_ascii_digits(&t.text);
        }
    }

    result
}

/// 连字符词库重写规则
///
/// 用于将词库中的连字符词（如 `GPT-5.3-Codex`）匹配输入中的空格/连字符混合形式，
/// 并重写为词库原词。
#[derive(Debug, Clone)]
struct HyphenDictionaryRule {
    canonical: String,
    segments: Vec<String>,
}

/// TNL 引擎（可复用，预编译规则）
pub struct TnlEngine {
    /// 口语流畅化清洗模式
    disfluency_mode: DisfluencyMode,
    /// 口语符号映射
    spoken_symbol_map: SpokenSymbolMap,
    /// 技术片段检测器
    tech_span_detector: TechSpanDetector,
    /// 模糊匹配器（可选）
    fuzzy_matcher: Option<FuzzyMatcher>,
    /// 连字符词库重写规则（仅针对包含 `-` 的词条）
    hyphen_rules: Vec<HyphenDictionaryRule>,
}

impl TnlEngine {
    /// 创建 TNL 引擎
    ///
    /// # Arguments
    /// * `dictionary` - 已提纯的词库（用于模糊匹配）
    pub fn new(dictionary: Vec<String>) -> Self {
        Self::new_with_disfluency_mode(dictionary, DisfluencyMode::Conservative)
    }

    pub fn new_with_disfluency_mode(
        dictionary: Vec<String>,
        disfluency_mode: DisfluencyMode,
    ) -> Self {
        let spoken_symbol_map = SpokenSymbolMap::new();
        let ext_whitelist = ExtensionWhitelist::new();
        let tech_span_detector = TechSpanDetector::new(ext_whitelist);
        let hyphen_rules = Self::build_hyphen_rules(&dictionary);
        let fuzzy_matcher = if dictionary.is_empty() {
            None
        } else {
            Some(FuzzyMatcher::new(dictionary))
        };

        Self {
            disfluency_mode,
            spoken_symbol_map,
            tech_span_detector,
            fuzzy_matcher,
            hyphen_rules,
        }
    }

    /// 创建无词库的 TNL 引擎
    pub fn new_without_dictionary() -> Self {
        Self::new(Vec::new())
    }

    /// 规范化文本
    ///
    /// 纯函数，不可失败（失败时返回原文）
    pub fn normalize(&self, text: &str) -> NormalizationResult {
        let start = Instant::now();

        if text.is_empty() {
            return NormalizationResult::unchanged(String::new(), 0);
        }

        let disfluency = clean_disfluency(text, self.disfluency_mode);
        let input = disfluency.text;
        if input.is_empty() {
            let elapsed_us = start.elapsed().as_micros() as u64;
            return NormalizationResult {
                text: input,
                changed: disfluency.changed,
                applied: Vec::new(),
                technical_spans: Vec::new(),
                elapsed_us,
                diagnostics: None,
            };
        }

        // 1. Unicode 归一化 (NFC) + 空白折叠
        let normalized = self.unicode_normalize(&input);

        // 1.5. 合并连续的空格分隔单字母（如 "T N L" → "TNL"）
        let (normalized, letter_merge_replacements) = merge_spaced_letters(&normalized);

        // 2. 分词
        let tokens = Tokenizer::tokenize(&normalized);

        // 3. 检测技术片段
        let tech_spans = self.tech_span_detector.detect(&normalized, &tokens);

        // 4. 应用口语符号映射（仅在技术片段内）
        let (mapped_text, symbol_replacements) =
            self.apply_spoken_symbol_mapping(&normalized, &tokens, &tech_spans);

        // 5. 拼音词库替换（精确匹配，带声调）
        let (pinyin_text, pinyin_replacements) = self.apply_pinyin_replacement(&mapped_text);

        // 5.5 连字符词库定向重写（如 "GPT 5.3 Codex" -> "GPT-5.3-Codex"）
        let (hyphen_text, hyphen_replacements) = self.apply_hyphen_dictionary_rewrite(&pinyin_text);

        // 6. 音标词库替换（英文复合词匹配）
        let (replaced_text, phonetic_replacements) = self.apply_phonetic_replacement(&hyphen_text);

        // 合并替换记录
        let mut applied = letter_merge_replacements;
        applied.extend(symbol_replacements);
        applied.extend(pinyin_replacements);
        applied.extend(hyphen_replacements);
        applied.extend(phonetic_replacements);

        let mut diagnostic_candidates: Vec<TnlCandidate> = applied
            .iter()
            .enumerate()
            .map(|(idx, replacement)| TnlCandidate::from_replacement(idx, replacement))
            .collect();
        let next_candidate_index = diagnostic_candidates.len();
        diagnostic_candidates.extend(self.collect_phonetic_diagnostic_candidates(
            &replaced_text,
            &applied,
            next_candidate_index,
        ));
        let diagnostics = TnlDiagnostics::from_candidates(diagnostic_candidates);

        let elapsed_us = start.elapsed().as_micros() as u64;
        let changed = replaced_text != text;

        NormalizationResult {
            text: replaced_text,
            changed,
            applied,
            technical_spans: tech_spans,
            elapsed_us,
            diagnostics,
        }
    }

    /// Unicode 归一化 + 空白折叠
    fn unicode_normalize(&self, text: &str) -> String {
        // NFC 归一化
        let nfc: String = text.nfc().collect();

        // 空白折叠：多个连续空白 -> 单个空格
        let mut result = String::with_capacity(nfc.len());
        let mut prev_whitespace = false;

        for ch in nfc.chars() {
            if ch.is_whitespace() {
                if !prev_whitespace {
                    result.push(' ');
                    prev_whitespace = true;
                }
            } else {
                result.push(ch);
                prev_whitespace = false;
            }
        }

        result.trim().to_string()
    }

    /// 应用口语符号映射
    ///
    /// 仅在技术片段内进行映射，同时吞掉符号相邻的空格
    ///
    /// 复杂度优化：使用游标线性扫描 O(tokens+spans)，而非每 token 都 find O(tokens×spans)
    fn apply_spoken_symbol_mapping(
        &self,
        text: &str,
        tokens: &[Token],
        tech_spans: &[Span],
    ) -> (String, Vec<Replacement>) {
        let mut result = String::with_capacity(text.len());
        let mut replacements = Vec::new();
        let mut last_end = 0;
        // 记录当前 tech span 的结束位置，只在 span 内跳过空格
        let mut skip_next_space_end: Option<usize> = None;
        // span 游标：利用 spans 已排序且不重叠的特性，线性前进
        let mut span_idx = 0;
        // 预计算"下一个非空白 token 是否为纯数字"，O(n) 一次扫描
        let next_is_digit = precompute_next_is_digit(tokens);
        // 追踪上一个输出的 token 是否为纯数字（用于数字间空格判断，避免 UTF-8 末字节误判）
        let mut last_emitted_is_digit = false;

        for (current_idx, token) in tokens.iter().enumerate() {
            // 添加 token 之前的文本
            if token.start > last_end {
                result.push_str(&text[last_end..token.start]);
            }

            // 游标前进：跳过已经过去的 spans（span.end <= token.start）
            while span_idx < tech_spans.len() && tech_spans[span_idx].end <= token.start {
                span_idx += 1;
            }

            // 判断当前 token 是否在 span 内
            let current_span_end = if span_idx < tech_spans.len() {
                let span = &tech_spans[span_idx];
                if token.start >= span.start && token.end <= span.end {
                    Some(span.end)
                } else {
                    None
                }
            } else {
                None
            };
            let in_tech_span = current_span_end.is_some();

            // 退出 tech span 时强制复位
            if !in_tech_span {
                skip_next_space_end = None;
            }

            // 如果上一个是口语符号或需要去空格的符号，跳过当前单个空格 token（仅在 tech span 内）
            if let Some(span_end) = skip_next_space_end {
                if token.token_type == TokenType::Whitespace
                    && token.text == " "
                    && token.end <= span_end
                {
                    skip_next_space_end = None;
                    last_end = token.end;
                    continue;
                }
            }
            skip_next_space_end = None;

            // tech span 内已有符号去空格（如 `src / lib . rs` → `src/lib.rs`）
            if in_tech_span && self.spoken_symbol_map.is_trim_symbol(&token.text) {
                // 吞掉符号前的空格
                while result.ends_with(' ') {
                    result.pop();
                }
                result.push_str(&token.text);
                skip_next_space_end = current_span_end;
                last_end = token.end;
                last_emitted_is_digit = false; // 符号不是数字
                continue;
            }

            // tech span 内连续数字间空格去除（如 `10455 3588` → `104553588`）
            // 使用 last_emitted_is_digit 状态而非 UTF-8 字节检查，避免多字节字符末字节误判
            if in_tech_span
                && token.token_type == TokenType::Whitespace
                && token.text == " "
                && last_emitted_is_digit
                && next_is_digit[current_idx]
            {
                last_end = token.end;
                continue;
            }

            // 尝试映射口语符号
            if in_tech_span && token.token_type == TokenType::Chinese {
                if let Some(symbol) = self.spoken_symbol_map.try_map(&token.text) {
                    // 吞掉符号前的空格（如果有）
                    while result.ends_with(' ') {
                        result.pop();
                    }

                    result.push(symbol);
                    replacements.push(Replacement {
                        original: token.text.clone(),
                        replaced: symbol.to_string(),
                        start: token.start,
                        end: token.end,
                        confidence: 1.0,
                        reason: ReplacementReason::SpokenSymbol,
                    });
                    last_end = token.end;
                    skip_next_space_end = current_span_end; // 标记跳过下一个空格
                    last_emitted_is_digit = false; // 映射后的符号不是数字
                    continue;
                }
            }

            result.push_str(&token.text);
            last_end = token.end;
            // 更新 last_emitted_is_digit 状态
            last_emitted_is_digit =
                token.token_type == TokenType::Ascii && is_ascii_digits(&token.text);
        }

        // 添加剩余文本
        if last_end < text.len() {
            result.push_str(&text[last_end..]);
        }

        (result, replacements)
    }

    /// 应用拼音词库替换
    ///
    /// 对连续中文片段尝试精确拼音匹配替换
    ///
    /// 约束条件：
    /// - 拼音 + 声调 100% 完全匹配
    /// - 原词 ≥2 个汉字
    /// - 同键冲突时跳过替换
    fn apply_pinyin_replacement(&self, text: &str) -> (String, Vec<Replacement>) {
        let Some(matcher) = &self.fuzzy_matcher else {
            return (text.to_string(), Vec::new());
        };

        let tokens = Tokenizer::tokenize(text);
        let mut result = String::with_capacity(text.len());
        let mut replacements: Vec<Replacement> = Vec::new();
        let mut last_end = 0;

        for token in tokens {
            // 添加 token 之前的文本
            if token.start > last_end {
                result.push_str(&text[last_end..token.start]);
            }

            if token.token_type == TokenType::Chinese {
                // 对中文 token 尝试拼音替换
                let mut local_idx = 0;
                while local_idx < token.text.len() {
                    let remaining = &token.text[local_idx..];

                    if let Some((replace_word, consumed)) =
                        matcher.try_exact_pinyin_replace(remaining)
                    {
                        let end = local_idx + consumed;
                        let original = &token.text[local_idx..end];

                        result.push_str(&replace_word);

                        // 仅当实际发生替换时记录
                        if replace_word != original {
                            replacements.push(Replacement {
                                original: original.to_string(),
                                replaced: replace_word,
                                start: token.start + local_idx,
                                end: token.start + end,
                                confidence: 1.0,
                                reason: ReplacementReason::DictionaryPinyin,
                            });
                        }

                        local_idx = end;
                        continue;
                    }

                    // 无匹配：复制单个字符
                    if let Some(ch) = remaining.chars().next() {
                        result.push(ch);
                        local_idx += ch.len_utf8();
                    }
                }
            } else {
                result.push_str(&token.text);
            }

            last_end = token.end;
        }

        // 添加剩余文本
        if last_end < text.len() {
            result.push_str(&text[last_end..]);
        }

        (result, replacements)
    }

    /// 应用连字符词库定向重写
    ///
    /// 仅匹配词库中显式包含连字符的词条，并允许输入中的分隔符是：
    /// - 连字符（可带空格）
    /// - 空格
    ///
    /// 示例：
    /// - GPT 5.3 Codex -> GPT-5.3-Codex
    /// - gpt-5.3 codex -> GPT-5.3-Codex
    fn apply_hyphen_dictionary_rewrite(&self, text: &str) -> (String, Vec<Replacement>) {
        if text.is_empty() || self.hyphen_rules.is_empty() {
            return (text.to_string(), Vec::new());
        }

        let bytes = text.as_bytes();
        let mut candidates: Vec<(usize, usize, &HyphenDictionaryRule)> = Vec::new();

        for (start, _) in text.char_indices() {
            let mut best: Option<(usize, &HyphenDictionaryRule)> = None;

            for rule in &self.hyphen_rules {
                if let Some(end) = Self::try_match_hyphen_rule(bytes, start, rule) {
                    if best.map_or(true, |(best_end, _)| end > best_end) {
                        best = Some((end, rule));
                    }
                }
            }

            if let Some((end, rule)) = best {
                candidates.push((start, end, rule));
            }
        }

        if candidates.is_empty() {
            return (text.to_string(), Vec::new());
        }

        // 先按起始位置排序，再按长度降序，便于贪心选择不重叠匹配
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| (b.1 - b.0).cmp(&(a.1 - a.0))));

        let mut selected: Vec<(usize, usize, &HyphenDictionaryRule)> = Vec::new();
        let mut cursor = 0usize;
        for candidate in candidates {
            if candidate.0 < cursor {
                continue;
            }
            cursor = candidate.1;
            selected.push(candidate);
        }

        if selected.is_empty() {
            return (text.to_string(), Vec::new());
        }

        let mut result = String::with_capacity(text.len());
        let mut replacements = Vec::new();
        let mut last_end = 0usize;

        for (start, end, rule) in selected {
            result.push_str(&text[last_end..start]);
            let original = &text[start..end];
            result.push_str(&rule.canonical);

            if original != rule.canonical {
                replacements.push(Replacement {
                    original: original.to_string(),
                    replaced: rule.canonical.clone(),
                    start,
                    end,
                    confidence: 1.0,
                    reason: ReplacementReason::DictionaryExact,
                });
            }

            last_end = end;
        }

        result.push_str(&text[last_end..]);

        (result, replacements)
    }

    /// 应用音标词库替换
    ///
    /// 对连续英文单词尝试音标匹配替换（复合词匹配）
    ///
    /// 例如：open cloud → OpenClaude
    ///
    /// 使用最长匹配策略：从最长前缀开始尝试匹配
    fn apply_phonetic_replacement(&self, text: &str) -> (String, Vec<Replacement>) {
        let Some(matcher) = &self.fuzzy_matcher else {
            return (text.to_string(), Vec::new());
        };

        let tokens = Tokenizer::tokenize(text);
        let mut result = String::with_capacity(text.len());
        let mut replacements: Vec<Replacement> = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = &tokens[i];

            // 检查是否是英文单词的开始
            let is_english_word = token.token_type == TokenType::Ascii
                && token.text.len() >= 2
                && (token.text.chars().all(|c| c.is_ascii_alphabetic())
                    || is_tech_token(&token.text));

            if !is_english_word {
                result.push_str(&token.text);
                i += 1;
                continue;
            }

            // 收集从当前位置开始的连续英文单词序列
            // 格式: [英文, 空格?, 英文, 空格?, ...]
            let mut english_run: Vec<(usize, &Token)> = vec![(i, token)];
            let mut j = i + 1;

            while j < tokens.len() {
                let next = &tokens[j];

                // 跳过空格
                if next.token_type == TokenType::Whitespace {
                    j += 1;
                    // 检查空格后是否还有英文单词
                    if j < tokens.len() {
                        let after_space = &tokens[j];
                        let is_eng = after_space.token_type == TokenType::Ascii
                            && after_space.text.len() >= 2
                            && (after_space.text.chars().all(|c| c.is_ascii_alphabetic())
                                || is_tech_token(&after_space.text));
                        if is_eng {
                            english_run.push((j, after_space));
                            j += 1;
                            continue;
                        }
                    }
                    break;
                } else {
                    break;
                }
            }

            // 尝试从最长到最短的前缀进行匹配（含单词级音标匹配）
            let mut matched = false;

            if !english_run.is_empty() {
                for len in (1..=english_run.len()).rev() {
                    let subset: Vec<&str> = english_run[..len]
                        .iter()
                        .map(|(_, t)| t.text.as_str())
                        .collect();

                    let first_idx = english_run[0].0;
                    let last_idx = english_run[len - 1].0;
                    let first_token = &tokens[first_idx];
                    let last_token = &tokens[last_idx];
                    let original = &text[first_token.start..last_token.end];

                    if matcher.has_exact_dictionary_match(original) {
                        result.push_str(original);
                        i = last_idx + 1;
                        matched = true;
                        break;
                    }

                    if let Some(fuzzy_match) = matcher.try_phonetic_match_tokens(&subset) {
                        // 匹配成功
                        result.push_str(&fuzzy_match.word);
                        replacements.push(Replacement {
                            original: original.to_string(),
                            replaced: fuzzy_match.word,
                            start: first_token.start,
                            end: last_token.end,
                            confidence: fuzzy_match.confidence,
                            reason: ReplacementReason::DictionaryPhonetic,
                        });

                        // 跳过已匹配的 token（包括中间的空格）
                        i = last_idx + 1;
                        // 如果下一个是空格，也跳过
                        if i < tokens.len() && tokens[i].token_type == TokenType::Whitespace {
                            // 不跳过，让下次循环处理
                        }
                        matched = true;
                        break;
                    }
                }
            }

            if !matched {
                // 无匹配：输出当前 token
                result.push_str(&token.text);
                i += 1;
            }
        }

        (result, replacements)
    }

    /// 收集中低置信英文音近候选，不直接改写文本。
    fn collect_phonetic_diagnostic_candidates(
        &self,
        text: &str,
        applied: &[Replacement],
        start_index: usize,
    ) -> Vec<TnlCandidate> {
        let Some(matcher) = &self.fuzzy_matcher else {
            return Vec::new();
        };

        let tokens = Tokenizer::tokenize(text);
        let mut candidates = Vec::new();
        let mut i = 0usize;
        let mut candidate_index = start_index;

        while i < tokens.len() && candidates.len() < MAX_DIAGNOSTIC_CANDIDATES {
            let token = &tokens[i];
            let is_english_word = token.token_type == TokenType::Ascii
                && token.text.len() >= 2
                && (token.text.chars().all(|c| c.is_ascii_alphabetic())
                    || is_tech_token(&token.text));

            if !is_english_word {
                i += 1;
                continue;
            }

            let mut english_run: Vec<(usize, &Token)> = vec![(i, token)];
            let mut j = i + 1;

            while j < tokens.len() {
                let next = &tokens[j];
                if next.token_type == TokenType::Whitespace {
                    j += 1;
                    if j < tokens.len() {
                        let after_space = &tokens[j];
                        let is_eng = after_space.token_type == TokenType::Ascii
                            && after_space.text.len() >= 2
                            && (after_space.text.chars().all(|c| c.is_ascii_alphabetic())
                                || is_tech_token(&after_space.text));
                        if is_eng {
                            english_run.push((j, after_space));
                            j += 1;
                            continue;
                        }
                    }
                }
                break;
            }

            let mut matched = false;
            for len in (1..=english_run.len()).rev() {
                let subset: Vec<&str> = english_run[..len]
                    .iter()
                    .map(|(_, t)| t.text.as_str())
                    .collect();

                let first_idx = english_run[0].0;
                let last_idx = english_run[len - 1].0;
                let first_token = &tokens[first_idx];
                let last_token = &tokens[last_idx];

                if Self::overlaps_applied(first_token.start, last_token.end, applied) {
                    continue;
                }

                let original = &text[first_token.start..last_token.end];
                if matcher.has_exact_dictionary_match(original) {
                    i = last_idx + 1;
                    matched = true;
                    break;
                }

                if let Some(candidate) = Self::build_phonetic_candidate(
                    matcher,
                    &subset,
                    text,
                    first_token.start,
                    last_token.end,
                    candidate_index,
                    TnlCandidateDecision::PendingLlm,
                    TnlCandidateRisk::Medium,
                    PENDING_LLM_MIN_SCORE,
                    LOCAL_APPLY_SCORE_CEILING,
                ) {
                    candidates.push(candidate);
                    candidate_index += 1;
                    i = last_idx + 1;
                    matched = true;
                    break;
                }

                if let Some(candidate) = Self::build_phonetic_candidate(
                    matcher,
                    &subset,
                    text,
                    first_token.start,
                    last_token.end,
                    candidate_index,
                    TnlCandidateDecision::RejectedLocal,
                    TnlCandidateRisk::High,
                    REJECTED_LOCAL_MIN_SCORE,
                    PENDING_LLM_MIN_SCORE,
                ) {
                    candidates.push(candidate);
                    candidate_index += 1;
                    i = last_idx + 1;
                    matched = true;
                    break;
                }
            }

            if !matched {
                i += 1;
            }
        }

        candidates
    }

    fn build_phonetic_candidate(
        matcher: &FuzzyMatcher,
        subset: &[&str],
        text: &str,
        start: usize,
        end: usize,
        index: usize,
        decision: TnlCandidateDecision,
        risk: TnlCandidateRisk,
        min_score: f32,
        max_score_exclusive: f32,
    ) -> Option<TnlCandidate> {
        let fuzzy_match =
            matcher.try_phonetic_candidate_tokens(subset, min_score, max_score_exclusive)?;
        let original = text[start..end].to_string();
        if original.eq_ignore_ascii_case(&fuzzy_match.word) {
            return None;
        }

        Some(TnlCandidate {
            id: format!("candidate-{}-{}-{}", start, end, index),
            original,
            target: fuzzy_match.word,
            start,
            end,
            score: fuzzy_match.confidence,
            risk,
            source: TnlCandidateSource::DictionaryPhonetic,
            evidence: vec![
                "phonetic_match".to_string(),
                format!("score={:.2}", fuzzy_match.confidence),
            ],
            decision,
        })
    }

    fn overlaps_applied(start: usize, end: usize, applied: &[Replacement]) -> bool {
        applied
            .iter()
            .any(|r| start < r.end && end > r.start && r.original != r.replaced)
    }

    /// 从词库构建连字符重写规则
    ///
    /// 只保留 ASCII 连字符词条，降低误命中风险。
    fn build_hyphen_rules(dictionary: &[String]) -> Vec<HyphenDictionaryRule> {
        let mut dedup = HashSet::new();
        let mut rules = Vec::new();

        for word in dictionary {
            let candidate = word.trim();
            if !candidate.is_ascii() || !candidate.contains('-') {
                continue;
            }

            let segments: Vec<String> = candidate
                .split('-')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            if segments.len() < 2 {
                continue;
            }

            let compact_len: usize = segments.iter().map(|s| s.len()).sum();
            if compact_len < 5 {
                continue;
            }

            let has_alpha = segments
                .iter()
                .any(|s| s.bytes().any(|b| b.is_ascii_alphabetic()));
            if !has_alpha {
                continue;
            }

            let valid_segments = segments.iter().all(|s| {
                s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
            });
            if !valid_segments {
                continue;
            }

            let canonical = segments.join("-");
            let dedup_key = canonical.to_ascii_lowercase();
            if dedup.insert(dedup_key) {
                rules.push(HyphenDictionaryRule {
                    canonical,
                    segments,
                });
            }
        }

        // 最长优先，避免短词抢占长词
        rules.sort_by(|a, b| {
            b.canonical
                .len()
                .cmp(&a.canonical.len())
                .then_with(|| a.canonical.cmp(&b.canonical))
        });
        rules
    }

    /// 尝试在 `start` 位置匹配连字符规则，返回匹配结束字节位置
    fn try_match_hyphen_rule(
        bytes: &[u8],
        start: usize,
        rule: &HyphenDictionaryRule,
    ) -> Option<usize> {
        if start >= bytes.len() {
            return None;
        }

        // 左边界保护：前一个字符不能是 ASCII 单词字符
        if start > 0 && Self::is_ascii_word_byte(bytes[start - 1]) {
            return None;
        }

        let mut pos = start;
        for (idx, segment) in rule.segments.iter().enumerate() {
            pos = Self::match_ascii_segment(bytes, pos, segment.as_bytes())?;
            if idx + 1 < rule.segments.len() {
                pos = Self::consume_hyphen_separator(bytes, pos)?;
            }
        }

        // 右边界保护：后一个字符不能是 ASCII 单词字符
        if pos < bytes.len() && Self::is_ascii_word_byte(bytes[pos]) {
            return None;
        }

        Some(pos)
    }

    /// 匹配一个 ASCII 段（大小写不敏感）
    fn match_ascii_segment(bytes: &[u8], start: usize, segment: &[u8]) -> Option<usize> {
        let end = start.checked_add(segment.len())?;
        if end > bytes.len() {
            return None;
        }

        for (offset, expected) in segment.iter().enumerate() {
            let actual = bytes[start + offset];
            if !actual.is_ascii() {
                return None;
            }
            if actual.to_ascii_lowercase() != expected.to_ascii_lowercase() {
                return None;
            }
        }

        Some(end)
    }

    /// 消费段间分隔符
    ///
    /// 合法分隔符：
    /// - 一个或多个空格
    /// - 连字符（前后允许空格）
    fn consume_hyphen_separator(bytes: &[u8], start: usize) -> Option<usize> {
        let mut pos = start;
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }

        if pos < bytes.len() && bytes[pos] == b'-' {
            pos += 1;
            while pos < bytes.len() && bytes[pos] == b' ' {
                pos += 1;
            }
            return Some(pos);
        }

        if pos > start {
            return Some(pos);
        }

        None
    }

    #[inline]
    fn is_ascii_word_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }
}

impl Default for TnlEngine {
    fn default() -> Self {
        Self::new_without_dictionary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_filename() {
        let engine = TnlEngine::default();

        let result = engine.normalize("readme 点 md");
        assert!(result.changed);
        assert_eq!(result.text, "readme.md");
    }

    #[test]
    fn test_normalize_path() {
        let engine = TnlEngine::default();

        let result = engine.normalize("src 斜杠 lib 点 rs");
        assert!(result.changed);
        assert_eq!(result.text, "src/lib.rs");
    }

    #[test]
    fn test_normalize_version() {
        let engine = TnlEngine::default();

        let result = engine.normalize("v1 点 2 点 3");
        assert!(result.changed);
        // 应该包含点号
        assert!(result.text.contains('.'));
    }

    #[test]
    fn test_no_change_natural_language() {
        let engine = TnlEngine::default();

        let result = engine.normalize("一点都不好");
        // "一点都不好" 中的"点"不在技术片段内，不应转换
        assert!(!result.changed);
        assert_eq!(result.text, "一点都不好");
    }

    #[test]
    fn test_unicode_normalization() {
        let engine = TnlEngine::default();

        // 测试多个空格折叠
        let result = engine.normalize("hello    world");
        assert_eq!(result.text, "hello world");
    }

    #[test]
    fn test_performance() {
        let engine = TnlEngine::default();

        let text = "我修改了 src 斜杠 tauri 斜杠 src 斜杠 tnl 斜杠 engine 点 rs 文件";

        let result = engine.normalize(text);

        // 目标 <10ms = 10000us
        assert!(
            result.elapsed_us < 10000,
            "耗时 {}us 超过 10ms",
            result.elapsed_us
        );
    }

    #[test]
    fn test_normalize_email() {
        let engine = TnlEngine::default();

        // 测试口语邮箱
        let result = engine.normalize("1045535878 艾特 qq 点 com");
        assert!(result.changed);
        assert_eq!(result.text, "1045535878@qq.com");

        // 测试带空格的邮箱
        let result2 = engine.normalize("test 艾特 example 点 com");
        assert!(result2.changed);
        assert_eq!(result2.text, "test@example.com");
    }

    #[test]
    fn test_no_false_positive_at() {
        let engine = TnlEngine::default();

        // "I AM At the school" 不应该被转换
        let result = engine.normalize("I AM At the school");
        assert!(!result.changed);
        assert_eq!(result.text, "I AM At the school");

        // "Look at this" 不应该被转换
        let result2 = engine.normalize("Look at this");
        assert!(!result2.changed);
        assert_eq!(result2.text, "Look at this");
    }

    // === 拼音词库替换集成测试 ===

    #[test]
    fn test_normalize_with_pinyin_replacement() {
        // 使用同音词：事例 (shi4li4) → 示例 (shi4li4)
        let engine = TnlEngine::new(vec!["示例".to_string()]);

        let result = engine.normalize("今天看了一个事例");
        assert!(result.changed);
        assert_eq!(result.text, "今天看了一个示例");
        assert!(result
            .applied
            .iter()
            .any(|r| matches!(r.reason, ReplacementReason::DictionaryPinyin)));
    }

    #[test]
    fn test_normalize_pinyin_tone_strict() {
        // 声调不同不应替换："妈妈" (ma1ma1) vs "骂骂" (ma4ma4)
        let engine = TnlEngine::new(vec!["骂骂".to_string()]);

        let result = engine.normalize("妈妈来了");
        assert!(!result.changed);
        assert_eq!(result.text, "妈妈来了");
    }

    #[test]
    fn test_normalize_pinyin_min_length() {
        // 单字不替换
        let engine = TnlEngine::new(vec!["马".to_string()]);

        let result = engine.normalize("一匹麻");
        // "麻" 是单字，不应被替换
        assert!(!result.changed || !result.text.contains("马"));
    }

    #[test]
    fn test_normalize_pinyin_conflict() {
        // 同音词冲突：公式 vs 公事 (gong1shi4)
        let engine = TnlEngine::new(vec!["公式".to_string(), "公事".to_string()]);

        let result = engine.normalize("处理攻势");
        // 冲突时不替换
        assert!(!result.changed);
        assert_eq!(result.text, "处理攻势");
    }

    #[test]
    fn test_normalize_combined_replacements() {
        // 同时测试口语符号映射和拼音替换
        let engine = TnlEngine::new(vec!["示例".to_string()]);

        let result = engine.normalize("查看 readme 点 md 中的事例");
        assert!(result.changed);
        // 应该同时包含口语符号替换和拼音替换
        assert!(result.text.contains("readme.md"));
        assert!(result.text.contains("示例"));
    }

    // === 空格吞并集成测试（口语符号映射时处理） ===

    #[test]
    fn test_space_swallowing_basic() {
        let engine = TnlEngine::default();

        // 空格在口语符号前后被吞掉
        let result = engine.normalize("readme 点 md");
        assert_eq!(result.text, "readme.md");

        // 符号两侧空格都被吞掉
        let result = engine.normalize("src 斜杠 lib 点 rs");
        assert_eq!(result.text, "src/lib.rs");
    }

    #[test]
    fn test_space_swallowing_email() {
        let engine = TnlEngine::default();

        // 邮箱场景
        let result = engine.normalize("1045535878 艾特 qq 点 com");
        assert_eq!(result.text, "1045535878@qq.com");
    }

    #[test]
    fn test_space_preserved_outside_tech_span() {
        let engine = TnlEngine::default();

        // 技术片段外的空格保持不变
        let result = engine.normalize("hello world");
        assert_eq!(result.text, "hello world");
    }

    // === 新增回归测试：tech span 内已有符号去空格 ===

    #[test]
    fn test_trim_spaces_around_existing_symbols_path() {
        let engine = TnlEngine::default();

        // 路径中已有的符号周围空格也去除
        let result = engine.normalize("src / lib . rs");
        assert!(result.changed);
        assert_eq!(result.text, "src/lib.rs");
    }

    #[test]
    fn test_trim_spaces_around_existing_symbols_email() {
        let engine = TnlEngine::default();

        // 邮箱中已有的符号周围空格也去除
        let result = engine.normalize("a @ b . com");
        assert!(result.changed);
        assert_eq!(result.text, "a@b.com");
    }

    #[test]
    fn test_symbols_outside_tech_span_not_trimmed() {
        let engine = TnlEngine::default();

        // 技术片段外的符号（没有域名，不是邮箱）不去空格
        let result = engine.normalize("a @ b");
        assert!(!result.changed);
        assert_eq!(result.text, "a @ b");
    }

    #[test]
    fn test_trim_spaces_between_digits() {
        let engine = TnlEngine::default();

        // tech span 内连续数字间空格去除
        let result = engine.normalize("10455 3588 艾特 qq 点 com");
        assert!(result.changed);
        assert_eq!(result.text, "104553588@qq.com");
    }

    #[test]
    fn test_digits_outside_tech_span_not_trimmed() {
        let engine = TnlEngine::default();

        // 技术片段外的数字间空格保持不变
        let result = engine.normalize("我有 10 个");
        assert!(!result.changed);
        assert_eq!(result.text, "我有 10 个");
    }

    // === UTF-8 末字节误判回归测试 ===

    #[test]
    fn test_utf8_last_byte_not_misidentified_as_digit() {
        let engine = TnlEngine::default();

        // 中文字符的 UTF-8 末字节可能落在 0x30-0x39 范围
        // 例如 "中" = E4 B8 AD，末字节 0xAD 不在数字范围
        // 但某些字符可能有末字节在数字范围的情况
        // 这个测试确保不会因为 UTF-8 末字节误判而错误吞并空格

        // "测" 的 UTF-8 是 E6 B5 8B，末字节 0x8B 不是数字
        // "试" 的 UTF-8 是 E8 AF 95，末字节 0x95 不是数字
        // 但我们需要确保逻辑正确，不依赖字节检查

        // 在 tech span 内，中文后面的空格不应被当作"数字间空格"吞掉
        let result = engine.normalize("测试 123 艾特 qq 点 com");
        assert!(result.changed);
        // "测试" 后的空格应该保留（因为"测试"不是数字）
        assert!(result.text.contains("测试 "));
    }

    #[test]
    fn test_mixed_chinese_digit_space_handling() {
        let engine = TnlEngine::default();

        // 混合场景：中文 + 数字 + 空格
        // 只有数字间的空格才应该被吞掉
        let result = engine.normalize("用户 10455 3588 艾特 qq 点 com");
        assert!(result.changed);
        // "用户" 后的空格应保留，"10455 3588" 间的空格应吞掉
        assert!(result.text.contains("用户 "));
        assert_eq!(result.text, "用户 104553588@qq.com");
    }

    // === 复杂度 O(n) 验证测试 ===

    #[test]
    fn test_linear_complexity_many_digit_spaces() {
        let engine = TnlEngine::default();

        // 构造大量数字间空格的输入，验证不会因 O(n²) 而超时
        // 格式：1 2 3 4 5 ... 艾特 qq 点 com
        let digits: Vec<&str> = (0..100)
            .map(|i| if i % 10 == 0 { "0" } else { "1" })
            .collect();
        let input = format!("{} 艾特 qq 点 com", digits.join(" "));

        let result = engine.normalize(&input);
        assert!(result.changed);
        // 应该在合理时间内完成（<10ms）
        assert!(
            result.elapsed_us < 10000,
            "耗时 {}us 超过 10ms，可能存在 O(n²) 复杂度问题",
            result.elapsed_us
        );
    }

    // === 音标词库替换集成测试 ===

    #[test]
    fn test_normalize_phonetic_compound_word() {
        // "open cloud" → "OpenClaude"
        let engine = TnlEngine::new(vec!["OpenClaude".to_string()]);

        let result = engine.normalize("使用 open cloud 进行开发");
        assert!(result.changed);
        assert!(result.text.contains("OpenClaude"));
        assert!(result
            .applied
            .iter()
            .any(|r| matches!(r.reason, ReplacementReason::DictionaryPhonetic)));
    }

    #[test]
    fn test_normalize_phonetic_clawed() {
        // "open clawed" → "OpenClaude"
        let engine = TnlEngine::new(vec!["OpenClaude".to_string()]);

        let result = engine.normalize("open clawed is great");
        assert!(result.changed);
        assert!(result.text.contains("OpenClaude"));
    }

    #[test]
    fn test_normalize_phonetic_no_match_claw() {
        // "open claw" ≠ "OpenClaude" (claw 编码为 KL，Claude 编码为 KLT)
        let engine = TnlEngine::new(vec!["OpenClaude".to_string()]);

        let result = engine.normalize("open claw");
        // claw 音标不匹配，不应替换
        assert!(!result.text.contains("OpenClaude"));
    }

    #[test]
    fn test_normalize_phonetic_single_word() {
        // "cloud" → "Claude"（单词级音标匹配）
        let engine = TnlEngine::new(vec!["Claude".to_string()]);

        let result = engine.normalize("I like cloud computing");
        assert!(result.changed);
        assert!(result.text.contains("Claude"));
        assert!(result
            .applied
            .iter()
            .any(|r| matches!(r.reason, ReplacementReason::DictionaryPhonetic)));
    }

    #[test]
    fn test_normalize_phonetic_single_word_chinese_context() {
        // 中文语境中的单词音标匹配：ASR 把 "Claude" 识别成 "cloud"
        let engine = TnlEngine::new(vec!["Claude".to_string()]);

        let result = engine.normalize("嗯，我最近学习了他们的那个标准产品 cloud");
        assert!(result.changed);
        assert!(result.text.contains("Claude"));
        assert!(!result.text.contains("cloud"));
    }

    #[test]
    fn test_disfluency_runs_before_phonetic_replacement() {
        let engine = TnlEngine::new(vec!["Claude".to_string()]);

        let result = engine.normalize("嗯，我最近学习了他们的那个标准产品 cloud");

        assert_eq!(result.text, "我最近学习了他们的那个标准产品 Claude");
    }

    #[test]
    fn test_disfluency_mode_off_keeps_leading_filler() {
        let engine =
            TnlEngine::new_with_disfluency_mode(vec!["Claude".to_string()], DisfluencyMode::Off);

        let result = engine.normalize("嗯，我最近学习了他们的那个标准产品 cloud");

        assert_eq!(result.text, "嗯，我最近学习了他们的那个标准产品 Claude");
    }

    #[test]
    fn test_normalize_phonetic_combined_with_symbol() {
        // 同时测试口语符号映射和音标替换
        let engine = TnlEngine::new(vec!["OpenClaude".to_string()]);

        let result = engine.normalize("open cloud 点 ai");
        assert!(result.changed);
        // 应该同时包含音标替换和口语符号替换
        assert!(result.text.contains("OpenClaude"));
        // 注意：.ai 可能不会被识别为技术片段，取决于 tech_span_detector
    }

    #[test]
    fn test_normalize_phonetic_tech_token() {
        // 技术 token：gpt4 -> gpt4o
        let engine = TnlEngine::new(vec!["gpt4o".to_string()]);

        let result = engine.normalize("请使用 gpt4 模型");
        assert!(result.changed);
        assert!(result.text.contains("gpt4o"));
    }

    #[test]
    fn test_hyphen_dictionary_rewrite_space_separated_version_term() {
        let engine = TnlEngine::new(vec!["GPT-5.3-Codex".to_string()]);

        let result = engine.normalize("请切换到 GPT 5.3 Codex 模型");
        assert!(result.changed);
        assert_eq!(result.text, "请切换到 GPT-5.3-Codex 模型");
        assert!(result
            .applied
            .iter()
            .any(|r| matches!(r.reason, ReplacementReason::DictionaryExact)));
    }

    #[test]
    fn test_hyphen_dictionary_rewrite_mixed_separator_and_case() {
        let engine = TnlEngine::new(vec!["GPT-5.3-Codex".to_string()]);

        let result = engine.normalize("请切换到 gpt-5.3 codex 模型");
        assert!(result.changed);
        assert_eq!(result.text, "请切换到 GPT-5.3-Codex 模型");
    }

    #[test]
    fn test_hyphen_dictionary_rewrite_respects_word_boundary() {
        let engine = TnlEngine::new(vec!["GPT-5.3-Codex".to_string()]);

        let result = engine.normalize("请切换到 GPT 5.3 CodexX 模型");
        assert!(!result.text.contains("GPT-5.3-Codex"));
        assert_eq!(result.text, "请切换到 GPT 5.3 CodexX 模型");
    }

    // === 连续单字母合并测试 ===

    #[test]
    fn test_merge_letters_basic() {
        let engine = TnlEngine::default();

        // 基本合并：3个字母
        let result = engine.normalize("T N L");
        assert!(result.changed);
        assert_eq!(result.text, "TNL");
        assert!(result
            .applied
            .iter()
            .any(|r| matches!(r.reason, ReplacementReason::LetterMerge)));
    }

    #[test]
    fn test_merge_letters_two() {
        let engine = TnlEngine::default();

        // 最少2个字母即合并
        let result = engine.normalize("A I");
        assert!(result.changed);
        assert_eq!(result.text, "AI");
    }

    #[test]
    fn test_merge_letters_usb() {
        let engine = TnlEngine::default();

        let result = engine.normalize("U S B");
        assert!(result.changed);
        assert_eq!(result.text, "USB");
    }

    #[test]
    fn test_merge_letters_lowercase() {
        let engine = TnlEngine::default();

        // 小写字母也合并，结果统一大写
        let result = engine.normalize("t n l");
        assert!(result.changed);
        assert_eq!(result.text, "TNL");
    }

    #[test]
    fn test_merge_letters_in_chinese_context() {
        let engine = TnlEngine::default();

        // 中文语境中的字母合并
        let result = engine.normalize("我说了 T N L 三个字母");
        assert!(result.changed);
        assert!(result.text.contains("TNL"));
        assert_eq!(result.text, "我说了 TNL 三个字母");
    }

    #[test]
    fn test_merge_letters_no_merge_single() {
        let engine = TnlEngine::default();

        // 单个字母不合并
        let result = engine.normalize("I am here");
        assert!(!result.changed);
        assert_eq!(result.text, "I am here");
    }

    #[test]
    fn test_merge_letters_no_merge_multiword() {
        let engine = TnlEngine::default();

        // 多字母单词不受影响
        let result = engine.normalize("LongCat Flash Lite");
        assert!(!result.changed);
        assert_eq!(result.text, "LongCat Flash Lite");
    }

    #[test]
    fn test_merge_letters_mixed_single_and_multi() {
        let engine = TnlEngine::default();

        // 混合场景：单字母后跟多字母单词
        let result = engine.normalize("A I model");
        assert!(result.changed);
        assert_eq!(result.text, "AI model");
    }

    #[test]
    fn test_merge_letters_with_phonetic_dict() {
        // 合并后的缩写可以被音标匹配命中
        let engine = TnlEngine::new(vec!["TNL".to_string()]);

        let result = engine.normalize("这是 T N L 层");
        assert!(result.changed);
        assert!(result.text.contains("TNL"));
    }

    #[test]
    fn test_merge_letters_vip() {
        let engine = TnlEngine::default();

        let result = engine.normalize("他是 V I P 用户");
        assert!(result.changed);
        assert_eq!(result.text, "他是 VIP 用户");
    }

    #[test]
    fn test_merge_letters_number_breaks_sequence() {
        let engine = TnlEngine::default();

        // 数字打断字母序列
        let result = engine.normalize("A 2 B");
        // A 后面是数字 "2"，不是单字母，不合并
        assert!(!result.changed);
        assert_eq!(result.text, "A 2 B");
    }

    // === Bug 回归测试：常见英文单词不应被词库误替换 ===

    #[test]
    fn test_windows_not_replaced_by_windsurf() {
        // 词库中有 "Windsurf" 时，"windows" 不应被替换
        let engine = TnlEngine::new(vec!["Windsurf".to_string()]);

        let result = engine.normalize("我在用 windows 系统");
        assert!(
            !result.text.contains("Windsurf"),
            "BUG: 'windows' was replaced with 'Windsurf' in: {}",
            result.text
        );
        assert!(result.text.contains("windows"));
    }

    #[test]
    fn test_windows_not_replaced_by_windsurf_uppercase() {
        let engine = TnlEngine::new(vec!["Windsurf".to_string()]);

        let result = engine.normalize("Windows is great");
        assert!(
            !result.text.contains("Windsurf"),
            "BUG: 'Windows' was replaced with 'Windsurf' in: {}",
            result.text
        );
    }

    #[test]
    fn test_diagnostics_records_local_phonetic_replacement() {
        let engine = TnlEngine::new(vec!["Claude Code".to_string()]);
        let result = engine.normalize("打开 Cloud Code");

        assert_eq!(result.text, "打开 Claude Code");
        let diagnostics = result.diagnostics.expect("应生成 TNL 诊断");
        let candidate = diagnostics
            .candidates
            .iter()
            .find(|c| c.original == "Cloud Code")
            .expect("应记录本地热词修正");

        assert_eq!(candidate.target, "Claude Code");
        assert_eq!(
            candidate.decision,
            crate::tnl::TnlCandidateDecision::AppliedLocal
        );
        assert_eq!(
            candidate.source,
            crate::tnl::TnlCandidateSource::DictionaryPhonetic
        );
    }

    #[test]
    fn test_phonetic_preserves_exact_dictionary_word_when_nearby_word_exists() {
        let engine = TnlEngine::new(vec![
            "Grok".to_string(),
            "Groq".to_string(),
            "Claude".to_string(),
        ]);
        let input = "就像 Grok 以及 Claude 相关的内容";

        let result = engine.normalize(input);

        assert_eq!(result.text, input);
        assert!(
            !result.applied.iter().any(|replacement| {
                replacement.original == "Grok"
                    && replacement.replaced == "Groq"
                    && matches!(replacement.reason, ReplacementReason::DictionaryPhonetic)
            }),
            "Grok 不应被本地音近替换为 Groq: {:?}",
            result.applied
        );

        if let Some(diagnostics) = result.diagnostics {
            assert!(
                !diagnostics.candidates.iter().any(|candidate| {
                    candidate.original == "Grok" && candidate.target == "Groq"
                }),
                "精确词库命中的 Grok 不应产生 Groq 音近候选: {:?}",
                diagnostics.candidates
            );
        }
    }

    #[test]
    fn test_diagnostics_collects_pending_llm_candidate_without_replacing() {
        let engine = TnlEngine::new(vec!["Their".to_string()]);
        let result = engine.normalize("open there");

        assert_eq!(result.text, "open there");
        let diagnostics = result.diagnostics.expect("应生成候选诊断");
        let candidate = diagnostics
            .candidates
            .iter()
            .find(|c| c.target == "Their")
            .expect("应召回中置信候选");

        assert_eq!(candidate.original, "there");
        assert_eq!(
            candidate.decision,
            crate::tnl::TnlCandidateDecision::PendingLlm,
            "score={}",
            candidate.score
        );
    }
}
