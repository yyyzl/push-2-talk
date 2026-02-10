//! TNL 模糊匹配
//!
//! 支持编辑距离、拼音相似度和英文音标匹配

use std::collections::{hash_map::Entry, HashMap, HashSet};

use pinyin::ToPinyin;
use rphonetic::DoubleMetaphone;
use strsim::levenshtein;

/// 音标匹配最低置信度阈值
const MIN_CONFIDENCE: f32 = 0.65;

/// 模糊匹配器
pub struct FuzzyMatcher {
    /// 词库（已提纯）
    dictionary: Vec<String>,
    /// 词库拼音缓存（无声调）
    pinyin_cache: Vec<String>,
    /// 拼音（带声调）→ 词库索引映射，None 表示同键冲突
    pinyin_tone_map: HashMap<String, Option<usize>>,
    /// 词库中参与声调匹配的最大词长（字符数）
    max_word_len: usize,
    /// 音标索引：主编码组合 → 词库索引列表
    /// 例如 "OpenClaude" 拆分为 ["Open", "Claude"]，编码为 "APN|KLT"
    phonetic_index: HashMap<String, Vec<usize>>,
    /// 单词音标索引：单个单词的主编码 → 词库索引列表
    /// 用于快速查找单词级音标匹配
    single_word_phonetic_index: HashMap<String, Vec<usize>>,
    /// Double Metaphone 编码器
    dm_encoder: DoubleMetaphone,
}

/// 匹配结果
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FuzzyMatch {
    /// 匹配到的词库词
    pub word: String,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
    /// 匹配类型
    pub match_type: FuzzyMatchType,
}

/// 匹配类型
#[derive(Debug, Clone, PartialEq)]
pub enum FuzzyMatchType {
    /// 精确匹配
    Exact,
    /// 编辑距离匹配
    EditDistance,
    /// 拼音匹配（中文）
    Pinyin,
    /// 音标匹配（英文）
    Phonetic,
}

impl FuzzyMatcher {
    /// 创建模糊匹配器
    ///
    /// # Arguments
    /// * `dictionary` - 已提纯的词库
    pub fn new(dictionary: Vec<String>) -> Self {
        let dm_encoder = DoubleMetaphone::default();

        // 预计算词库拼音（无声调）
        let pinyin_cache: Vec<String> = dictionary
            .iter()
            .map(|word| Self::to_pinyin_str(word))
            .collect();

        // 预计算带声调拼音映射
        let mut pinyin_tone_map: HashMap<String, Option<usize>> = HashMap::new();
        let mut max_word_len: usize = 0;

        for (i, word) in dictionary.iter().enumerate() {
            let key = Self::to_pinyin_with_tone(word);

            let word_len = word.chars().count();
            // 仅处理 ≥2 字的词
            if word_len < 2 || key.is_empty() {
                continue;
            }

            max_word_len = std::cmp::max(max_word_len, word_len);

            match pinyin_tone_map.entry(key) {
                Entry::Vacant(e) => {
                    e.insert(Some(i));
                }
                Entry::Occupied(mut e) => {
                    // 同键冲突：标记为 None，跳过替换
                    e.insert(None);
                }
            }
        }

        // 预计算音标索引（支持 primary + alternate）
        let mut phonetic_index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut single_word_phonetic_index: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, word) in dictionary.iter().enumerate() {
            // 仅对包含 ASCII 字母的词建立音标索引
            if !word.chars().any(|c| c.is_ascii_alphabetic()) {
                continue;
            }

            // 拆分复合词并计算音标编码
            let parts = Self::split_compound_word(word);
            if parts.is_empty() {
                continue;
            }

            // 复合词索引：写入 primary key 和 alternate key
            let primary_key = Self::compute_phonetic_key_with_mode(&dm_encoder, &parts, false);
            let alternate_key = Self::compute_phonetic_key_with_mode(&dm_encoder, &parts, true);
            if !primary_key.is_empty() {
                phonetic_index
                    .entry(primary_key.clone())
                    .or_default()
                    .push(i);
            }
            if !alternate_key.is_empty() && alternate_key != primary_key {
                phonetic_index.entry(alternate_key).or_default().push(i);
            }

            // 对纯英文单词建立单词级索引（支持 primary + alternate）
            if word.chars().all(|c| c.is_ascii_alphabetic()) && word.len() >= 2 {
                let codes = Self::get_phonetic_codes(&dm_encoder, word);
                for code in codes {
                    single_word_phonetic_index.entry(code).or_default().push(i);
                }
            }
        }

        Self {
            dictionary,
            pinyin_cache,
            pinyin_tone_map,
            max_word_len,
            phonetic_index,
            single_word_phonetic_index,
            dm_encoder,
        }
    }

    /// 拆分复合词为多个部分
    ///
    /// 支持：
    /// - 驼峰命名：OpenClaude → ["Open", "Claude"]
    /// - 下划线分隔：open_claude → ["open", "claude"]
    /// - 连字符分隔：open-claude → ["open", "claude"]
    /// - 纯小写/大写：保持原样
    fn split_compound_word(word: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();

        for ch in word.chars() {
            match ch {
                '_' | '-' | ' ' => {
                    // 分隔符：保存当前部分
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                c if c.is_ascii_uppercase() => {
                    // 大写字母：可能是驼峰边界
                    if !current.is_empty() && current.chars().last().map_or(false, |p| p.is_ascii_lowercase())
                    {
                        // 前一个是小写，当前是大写 → 驼峰边界
                        parts.push(current.clone());
                        current.clear();
                    }
                    current.push(c);
                }
                c => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        // 过滤掉非纯英文部分和过短部分
        parts
            .into_iter()
            .filter(|p| p.len() >= 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
            .collect()
    }

    /// 获取单词的所有音标编码（primary + alternate）
    fn get_phonetic_codes(encoder: &DoubleMetaphone, word: &str) -> Vec<String> {
        let dm = encoder.double_metaphone(word);
        let primary = dm.primary();
        if primary.is_empty() {
            return Vec::new();
        }

        let alternate = dm.alternate();
        if !alternate.is_empty() && alternate != primary {
            vec![primary, alternate]
        } else {
            vec![primary]
        }
    }

    /// 按模式计算复合词音标键
    fn compute_phonetic_key_with_mode(
        encoder: &DoubleMetaphone,
        parts: &[String],
        use_alternate: bool,
    ) -> String {
        let mut codes = Vec::with_capacity(parts.len());
        for part in parts {
            let dm = encoder.double_metaphone(part);
            let mut code = if use_alternate {
                dm.alternate()
            } else {
                dm.primary()
            };
            // alternate 为空时回退到 primary
            if use_alternate && code.is_empty() {
                code = dm.primary();
            }
            if code.is_empty() {
                return String::new();
            }
            codes.push(code);
        }
        codes.join("|")
    }

    /// 按模式计算查询键
    fn compute_query_key(encoder: &DoubleMetaphone, tokens: &[&str], use_alternate: bool) -> String {
        let mut codes = Vec::with_capacity(tokens.len());
        for token in tokens {
            let dm = encoder.double_metaphone(token);
            let mut code = if use_alternate {
                dm.alternate()
            } else {
                dm.primary()
            };
            // alternate 为空时回退到 primary
            if use_alternate && code.is_empty() {
                code = dm.primary();
            }
            if code.is_empty() {
                return String::new();
            }
            codes.push(code);
        }
        codes.join("|")
    }

    /// 计算候选词评分
    ///
    /// 综合因子：编辑距离相似度、长度相似度、首字母奖励
    pub fn calculate_candidate_score(input: &str, candidate: &str, base_confidence: f32) -> f32 {
        if input.is_empty() || candidate.is_empty() {
            return 0.0;
        }

        let input_lower = input.to_lowercase();
        let candidate_lower = candidate.to_lowercase();

        let input_len = input_lower.chars().count();
        let candidate_len = candidate_lower.chars().count();
        let max_len = std::cmp::max(input_len, candidate_len);
        let min_len = std::cmp::min(input_len, candidate_len);
        if max_len == 0 {
            return 0.0;
        }

        // 编辑距离相似度
        let distance = levenshtein(&input_lower, &candidate_lower);
        let edit_sim = (1.0 - (distance as f32 / max_len as f32)).clamp(0.0, 1.0);

        // 长度相似度
        let len_sim = (min_len as f32 / max_len as f32).clamp(0.0, 1.0);

        // 编辑距离因子：保持温和惩罚，使典型音标命中保持在 MIN_CONFIDENCE 之上
        let edit_factor = 0.55 + 0.45 * edit_sim;
        let len_factor = 0.55 + 0.45 * len_sim;

        let mut score = base_confidence.clamp(0.0, 1.0) * edit_factor * len_factor;

        // 首字母奖励
        if input_lower.chars().next() == candidate_lower.chars().next() {
            score += 0.05;
        }

        score.clamp(0.0, 1.0)
    }

    /// 尝试匹配
    ///
    /// 返回最佳匹配结果（如果有）
    #[allow(dead_code)]
    pub fn try_match(&self, text: &str) -> Option<FuzzyMatch> {
        if text.is_empty() || self.dictionary.is_empty() {
            return None;
        }

        // 1. 精确匹配
        if let Some(exact) = self.try_exact_match(text) {
            return Some(exact);
        }

        // 2. 编辑距离匹配
        if let Some(fuzzy) = self.try_edit_distance_match(text) {
            return Some(fuzzy);
        }

        // 3. 拼音匹配（中文）
        if let Some(pinyin) = self.try_pinyin_match(text) {
            return Some(pinyin);
        }

        // 4. 音标匹配（英文）
        if let Some(phonetic) = self.try_phonetic_match(text) {
            return Some(phonetic);
        }

        None
    }

    /// 尝试音标匹配（针对多个连续英文 token）
    ///
    /// # Arguments
    /// * `tokens` - 连续的英文 token 列表，如 ["open", "cloud"]
    ///
    /// # Returns
    /// 如果匹配成功，返回词库中的词和置信度
    pub fn try_phonetic_match_tokens(&self, tokens: &[&str]) -> Option<FuzzyMatch> {
        if tokens.is_empty() || self.phonetic_index.is_empty() {
            return None;
        }

        // 过滤非纯英文 token
        let valid_tokens: Vec<&str> = tokens
            .iter()
            .filter(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_alphabetic()))
            .copied()
            .collect();

        if valid_tokens.is_empty() {
            return None;
        }

        // 计算查询键（primary + alternate）
        let primary_key = Self::compute_query_key(&self.dm_encoder, &valid_tokens, false);
        let alternate_key = Self::compute_query_key(&self.dm_encoder, &valid_tokens, true);

        // 收集候选索引（去重）
        let mut candidate_indices: Vec<usize> = Vec::new();
        let mut seen: HashSet<usize> = HashSet::new();

        for key in [primary_key.as_str(), alternate_key.as_str()] {
            if key.is_empty() {
                continue;
            }
            if let Some(indices) = self.phonetic_index.get(key) {
                for &idx in indices {
                    if seen.insert(idx) {
                        candidate_indices.push(idx);
                    }
                }
            }
        }

        if candidate_indices.is_empty() {
            return None;
        }

        // 评分择优
        let input = valid_tokens.join("");
        let input_lower = input.to_lowercase();
        let base_confidence = if candidate_indices.len() == 1 {
            0.9
        } else {
            0.85
        };

        let mut best: Option<(usize, f32)> = None;
        for &idx in &candidate_indices {
            let word = &self.dictionary[idx];
            // 跳过精确同词
            if word.to_lowercase() == input_lower {
                continue;
            }

            let score = Self::calculate_candidate_score(&input, word, base_confidence);
            if score >= MIN_CONFIDENCE && best.as_ref().map_or(true, |(_, s)| score > *s) {
                best = Some((idx, score));
            }
        }

        best.map(|(idx, score)| FuzzyMatch {
            word: self.dictionary[idx].clone(),
            confidence: score,
            match_type: FuzzyMatchType::Phonetic,
        })
    }

    /// 精确匹配
    fn try_exact_match(&self, text: &str) -> Option<FuzzyMatch> {
        let text_lower = text.to_lowercase();
        for word in &self.dictionary {
            if word.to_lowercase() == text_lower {
                return Some(FuzzyMatch {
                    word: word.clone(),
                    confidence: 1.0,
                    match_type: FuzzyMatchType::Exact,
                });
            }
        }
        None
    }

    /// 编辑距离匹配
    fn try_edit_distance_match(&self, text: &str) -> Option<FuzzyMatch> {
        let text_len = text.chars().count();
        // 阈值：max(1, len/4)
        let threshold = std::cmp::max(1, text_len / 4);

        let mut best_match: Option<(String, usize)> = None;

        for word in &self.dictionary {
            let word_len = word.chars().count();
            // 长度差异过大则跳过
            if (word_len as i32 - text_len as i32).abs() > threshold as i32 {
                continue;
            }

            let distance = levenshtein(text, word);
            if distance <= threshold && best_match.as_ref().map_or(true, |(_, d)| distance < *d) {
                best_match = Some((word.clone(), distance));
            }
        }

        best_match.map(|(word, distance)| {
            // 置信度：1.0 - (distance / max_distance)
            let max_distance = std::cmp::max(text_len, word.chars().count());
            let confidence = if max_distance > 0 {
                1.0 - (distance as f32 / max_distance as f32)
            } else {
                1.0
            };
            // 模糊匹配基础置信度 0.8
            FuzzyMatch {
                word,
                confidence: confidence * 0.8,
                match_type: FuzzyMatchType::EditDistance,
            }
        })
    }

    /// 拼音匹配
    fn try_pinyin_match(&self, text: &str) -> Option<FuzzyMatch> {
        let text_pinyin = Self::to_pinyin_str(text);
        if text_pinyin.is_empty() {
            return None;
        }

        for (i, word_pinyin) in self.pinyin_cache.iter().enumerate() {
            if word_pinyin.is_empty() {
                continue;
            }

            // 全拼完全匹配
            if text_pinyin == *word_pinyin {
                return Some(FuzzyMatch {
                    word: self.dictionary[i].clone(),
                    confidence: 0.7,
                    match_type: FuzzyMatchType::Pinyin,
                });
            }

            // 首字母匹配（仅当词库词较长时）
            let word_initials = Self::to_pinyin_initials(&self.dictionary[i]);
            let text_initials = Self::to_pinyin_initials(text);
            if !word_initials.is_empty()
                && word_initials == text_initials
                && self.dictionary[i].chars().count() >= 2
            {
                return Some(FuzzyMatch {
                    word: self.dictionary[i].clone(),
                    confidence: 0.6,
                    match_type: FuzzyMatchType::Pinyin,
                });
            }
        }

        None
    }

    /// 单词音标匹配（用于 try_match）
    ///
    /// 使用预计算索引，支持 primary + alternate 查询和评分择优
    fn try_phonetic_match(&self, text: &str) -> Option<FuzzyMatch> {
        // 仅对纯英文单词生效
        if text.len() < 2 || !text.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }

        // 获取查询的所有音标编码
        let query_codes = Self::get_phonetic_codes(&self.dm_encoder, text);
        if query_codes.is_empty() {
            return None;
        }

        // 收集候选索引（去重）
        let mut candidate_indices: Vec<usize> = Vec::new();
        let mut seen: HashSet<usize> = HashSet::new();
        for code in query_codes {
            if let Some(indices) = self.single_word_phonetic_index.get(&code) {
                for &idx in indices {
                    if seen.insert(idx) {
                        candidate_indices.push(idx);
                    }
                }
            }
        }

        if candidate_indices.is_empty() {
            return None;
        }

        // 评分择优
        let text_lower = text.to_lowercase();
        let base_confidence = if candidate_indices.len() == 1 {
            0.9
        } else {
            0.85
        };

        let mut best: Option<(usize, f32)> = None;
        for &idx in &candidate_indices {
            let word = &self.dictionary[idx];
            // 跳过精确同词
            if word.to_lowercase() == text_lower {
                continue;
            }

            let score = Self::calculate_candidate_score(text, word, base_confidence);
            if score >= MIN_CONFIDENCE && best.as_ref().map_or(true, |(_, s)| score > *s) {
                best = Some((idx, score));
            }
        }

        best.map(|(idx, score)| FuzzyMatch {
            word: self.dictionary[idx].clone(),
            confidence: score,
            match_type: FuzzyMatchType::Phonetic,
        })
    }

    /// 转换为拼音字符串（全拼，无声调）
    fn to_pinyin_str(text: &str) -> String {
        let mut result = String::new();
        for ch in text.chars() {
            if let Some(pinyin) = ch.to_pinyin() {
                result.push_str(pinyin.plain());
            } else if ch.is_ascii_alphanumeric() {
                result.push(ch.to_ascii_lowercase());
            }
        }
        result
    }

    /// 转换为带声调数字的拼音（如 "张三" → "zhang1san1"）
    ///
    /// 如果包含非汉字字符，返回空字符串
    fn to_pinyin_with_tone(text: &str) -> String {
        let mut result = String::new();
        for ch in text.chars() {
            if let Some(pinyin) = ch.to_pinyin() {
                result.push_str(pinyin.with_tone_num());
            } else {
                // 非汉字字符，返回空字符串（不参与拼音匹配）
                return String::new();
            }
        }
        result
    }

    /// 精确拼音替换（带声调）
    ///
    /// 返回 `Some((替换词, 消费字节数))` 如果匹配成功
    ///
    /// 匹配条件：
    /// - 拼音 + 声调 100% 完全匹配
    /// - 原词 ≥2 个汉字
    /// - 无同键冲突（一键多值时跳过）
    ///
    /// 使用最长匹配策略
    pub fn try_exact_pinyin_replace(&self, text: &str) -> Option<(String, usize)> {
        if text.is_empty() || self.dictionary.is_empty() || self.max_word_len < 2 {
            return None;
        }

        // 收集字符结束位置（按 max_word_len 截断）
        let mut char_ends: Vec<usize> = Vec::with_capacity(self.max_word_len);
        for (idx, ch) in text.char_indices() {
            char_ends.push(idx + ch.len_utf8());
            if char_ends.len() >= self.max_word_len {
                break;
            }
        }

        // 至少需要 2 个字符
        if char_ends.len() < 2 {
            return None;
        }

        // 从长到短尝试匹配（最长匹配优先）
        for len in (2..=char_ends.len()).rev() {
            let end_byte = char_ends[len - 1];
            let candidate = &text[..end_byte];
            let key = Self::to_pinyin_with_tone(candidate);
            if key.is_empty() {
                continue;
            }

            match self.pinyin_tone_map.get(&key) {
                Some(Some(idx)) => {
                    // 找到唯一匹配，返回词库中的词
                    return Some((self.dictionary[*idx].clone(), end_byte));
                }
                Some(None) => {
                    // 同键冲突，返回原文（不替换但消费字符）
                    return Some((candidate.to_string(), end_byte));
                }
                None => {
                    // 无匹配，继续尝试更短的前缀
                }
            }
        }

        None
    }

    /// 转换为拼音首字母
    fn to_pinyin_initials(text: &str) -> String {
        let mut result = String::new();
        for ch in text.chars() {
            if let Some(pinyin) = ch.to_pinyin() {
                if let Some(first) = pinyin.plain().chars().next() {
                    result.push(first);
                }
            } else if ch.is_ascii_alphabetic() {
                result.push(ch.to_ascii_lowercase());
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let matcher = FuzzyMatcher::new(vec!["Claude".to_string(), "Tauri".to_string()]);

        let result = matcher.try_match("claude");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "Claude");
        assert_eq!(m.match_type, FuzzyMatchType::Exact);
    }

    #[test]
    fn test_edit_distance_match() {
        let matcher = FuzzyMatcher::new(vec!["readme".to_string()]);

        // "readm" 距离 "readme" 编辑距离为 1
        let result = matcher.try_match("readm");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.match_type, FuzzyMatchType::EditDistance);
    }

    #[test]
    fn test_pinyin_match() {
        let matcher = FuzzyMatcher::new(vec!["配置".to_string()]);

        let result = matcher.try_match("peizhi");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "配置");
        assert_eq!(m.match_type, FuzzyMatchType::Pinyin);
    }

    #[test]
    fn test_no_match() {
        let matcher = FuzzyMatcher::new(vec!["hello".to_string()]);

        let result = matcher.try_match("xyz");
        assert!(result.is_none());
    }

    // === 拼音精确替换测试 ===

    #[test]
    fn test_pinyin_replace_basic() {
        // "掌伞" (zhang3san3) 应该被替换为 "张三" (zhang1san1)... 不对，它们声调不同
        // 使用同音词测试：事例 (shi4li4) vs 示例 (shi4li4)
        let matcher = FuzzyMatcher::new(vec!["示例".to_string()]);

        let result = matcher.try_exact_pinyin_replace("事例");
        assert!(result.is_some());
        let (word, len) = result.unwrap();
        assert_eq!(word, "示例");
        assert_eq!(len, "事例".len());
    }

    #[test]
    fn test_pinyin_replace_tone_strict() {
        // "妈妈" (ma1ma1) vs "骂骂" (ma4ma4) - 声调不同，不应替换
        let matcher = FuzzyMatcher::new(vec!["骂骂".to_string()]);

        let result = matcher.try_exact_pinyin_replace("妈妈");
        assert!(result.is_none());
    }

    #[test]
    fn test_pinyin_replace_min_length() {
        // 单字不应替换
        let matcher = FuzzyMatcher::new(vec!["马".to_string()]);

        let result = matcher.try_exact_pinyin_replace("麻");
        assert!(result.is_none());
    }

    #[test]
    fn test_pinyin_replace_conflict_skip() {
        // 同音词冲突：公式 vs 公事 (gong1shi4) - 应返回原文
        let matcher = FuzzyMatcher::new(vec!["公式".to_string(), "公事".to_string()]);

        let result = matcher.try_exact_pinyin_replace("攻势");
        assert!(result.is_some());
        let (word, _) = result.unwrap();
        // 冲突时返回原文
        assert_eq!(word, "攻势");
    }

    #[test]
    fn test_pinyin_replace_longest_match() {
        // 最长匹配测试（避免多音字以确保测试稳定）
        // "示例" (shi4li4) vs "示例库" (shi4li4ku4)
        let matcher = FuzzyMatcher::new(vec!["示例".to_string(), "示例库".to_string()]);

        // "事例酷" (shi4li4ku4) 应匹配 "示例库"（最长匹配）
        let result = matcher.try_exact_pinyin_replace("事例酷");
        assert!(result.is_some());
        let (word, _) = result.unwrap();
        assert_eq!(word, "示例库");
    }

    #[test]
    fn test_pinyin_replace_non_chinese_skip() {
        // 非中文不参与拼音匹配
        let matcher = FuzzyMatcher::new(vec!["readme".to_string()]);

        let result = matcher.try_exact_pinyin_replace("readme");
        assert!(result.is_none());
    }

    // === 音标匹配测试 ===

    #[test]
    fn test_phonetic_match_single_word() {
        // cloud → Claude (都编码为 KLT)
        let matcher = FuzzyMatcher::new(vec!["Claude".to_string()]);

        let result = matcher.try_match("cloud");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "Claude");
        assert_eq!(m.match_type, FuzzyMatchType::Phonetic);
    }

    #[test]
    fn test_phonetic_match_clawed() {
        // clawed → Claude
        let matcher = FuzzyMatcher::new(vec!["Claude".to_string()]);

        let result = matcher.try_match("clawed");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "Claude");
        assert_eq!(m.match_type, FuzzyMatchType::Phonetic);
    }

    #[test]
    fn test_phonetic_no_match_claw() {
        // claw (KL) ≠ Claude (KLT) - 音标不同
        let matcher = FuzzyMatcher::new(vec!["Claude".to_string()]);

        let result = matcher.try_match("claw");
        // claw 可能通过编辑距离匹配（距离=2，阈值=1），所以检查是否不是音标匹配
        if let Some(m) = result {
            assert_ne!(m.match_type, FuzzyMatchType::Phonetic);
        }
    }

    #[test]
    fn test_phonetic_match_compound_word() {
        // "open cloud" → "OpenClaude"
        let matcher = FuzzyMatcher::new(vec!["OpenClaude".to_string()]);

        let result = matcher.try_phonetic_match_tokens(&["open", "cloud"]);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "OpenClaude");
        assert_eq!(m.match_type, FuzzyMatchType::Phonetic);
    }

    #[test]
    fn test_phonetic_match_compound_clawed() {
        // "open clawed" → "OpenClaude"
        let matcher = FuzzyMatcher::new(vec!["OpenClaude".to_string()]);

        let result = matcher.try_phonetic_match_tokens(&["open", "clawed"]);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "OpenClaude");
    }

    #[test]
    fn test_phonetic_match_compound_clod() {
        // "open clod" → "OpenClaude"
        let matcher = FuzzyMatcher::new(vec!["OpenClaude".to_string()]);

        let result = matcher.try_phonetic_match_tokens(&["open", "clod"]);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "OpenClaude");
    }

    #[test]
    fn test_phonetic_no_match_compound_claw() {
        // "open claw" ≠ "OpenClaude" (claw 编码为 KL，Claude 编码为 KLT)
        let matcher = FuzzyMatcher::new(vec!["OpenClaude".to_string()]);

        let result = matcher.try_phonetic_match_tokens(&["open", "claw"]);
        assert!(result.is_none());
    }

    #[test]
    fn test_split_compound_word_camel_case() {
        let parts = FuzzyMatcher::split_compound_word("OpenClaude");
        assert_eq!(parts, vec!["Open", "Claude"]);
    }

    #[test]
    fn test_split_compound_word_underscore() {
        let parts = FuzzyMatcher::split_compound_word("open_claude");
        assert_eq!(parts, vec!["open", "claude"]);
    }

    #[test]
    fn test_split_compound_word_hyphen() {
        let parts = FuzzyMatcher::split_compound_word("open-claude");
        assert_eq!(parts, vec!["open", "claude"]);
    }

    #[test]
    fn test_split_compound_word_single() {
        let parts = FuzzyMatcher::split_compound_word("Claude");
        assert_eq!(parts, vec!["Claude"]);
    }

    #[test]
    fn test_phonetic_tarry_tauri() {
        // tarry → Tauri (都编码为 TR)
        let matcher = FuzzyMatcher::new(vec!["Tauri".to_string()]);

        let result = matcher.try_match("tarry");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "Tauri");
        assert_eq!(m.match_type, FuzzyMatchType::Phonetic);
    }

    // === 音标匹配增强测试 ===

    #[test]
    fn test_candidate_score_prefers_lower_edit_distance() {
        let score_near = FuzzyMatcher::calculate_candidate_score("cloud", "Claude", 0.9);
        let score_far = FuzzyMatcher::calculate_candidate_score("cloud", "Clouded", 0.9);
        assert!(score_near > score_far);
    }

    #[test]
    fn test_candidate_score_first_letter_bonus() {
        let score_same_initial = FuzzyMatcher::calculate_candidate_score("nite", "Night", 0.9);
        let score_diff_initial = FuzzyMatcher::calculate_candidate_score("nite", "Knight", 0.9);
        assert!(score_same_initial > score_diff_initial);
    }

    #[test]
    fn test_single_word_disambiguation_prefers_closer_candidate_even_if_not_first() {
        let matcher = FuzzyMatcher::new(vec!["Clouded".to_string(), "Claude".to_string()]);
        let result = matcher.try_match("cloud");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.match_type, FuzzyMatchType::Phonetic);
        assert_eq!(m.word, "Claude");
    }

    #[test]
    fn test_compound_disambiguation_prefers_closer_candidate_even_if_not_first() {
        let matcher = FuzzyMatcher::new(vec!["OpenClouded".to_string(), "OpenClaude".to_string()]);
        let result = matcher.try_phonetic_match_tokens(&["open", "cloud"]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().word, "OpenClaude");
    }

    #[test]
    fn test_candidate_score_can_fall_below_threshold() {
        let score = FuzzyMatcher::calculate_candidate_score(
            "ab",
            "supercalifragilisticexpialidocious",
            0.8,
        );
        assert!(score < MIN_CONFIDENCE);
    }
}
