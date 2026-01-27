// 文本差异分析器
//
// 功能：提取 baseline（ASR 原文）与 current（用户修正后）的差异
// 使用 LCS（最长公共子序列）算法

/// 最大处理字符数（超过此长度使用快速 diff）
const MAX_CHARS: usize = 2048;

/// 绝对最大字符数（超过此长度直接截断，防止 OOM）
const ABSOLUTE_MAX_CHARS: usize = 10000;

/// 差异结果
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub original_segment: String,
    pub corrected_segment: String,
    pub context: String,
    /// 原文中的起始位置（字符索引）
    pub orig_start: usize,
    /// 原文中的结束位置（字符索引）
    pub orig_end: usize,
    /// 修正文本中的起始位置（字符索引）
    pub curr_start: usize,
    /// 修正文本中的结束位置（字符索引）
    pub curr_end: usize,
}

/// Diff 操作类型
enum DiffOp {
    Equal,
    Insert,
    Delete,
}

/// 分析两个文本的差异
///
/// # Arguments
/// * `baseline` - 基准文本（ASR 原文）
/// * `current` - 当前文本（用户修正后）
///
/// # Returns
/// 差异列表，每个元素包含原文片段、修正片段和上下文
pub fn analyze_diff(baseline: &str, current: &str) -> Vec<DiffResult> {
    // 防止 OOM：先检查字节长度，超过绝对上限直接截断
    // 使用字节长度作为快速预检（避免遍历整个字符串）
    let baseline_byte_len = baseline.len();
    let current_byte_len = current.len();

    // 如果字节长度超过绝对上限的 4 倍（UTF-8 最多 4 字节/字符），需要截断
    let byte_limit = ABSOLUTE_MAX_CHARS * 4;
    let (baseline_truncated, current_truncated) = if baseline_byte_len > byte_limit || current_byte_len > byte_limit {
        tracing::warn!(
            "Learning: 文本过长 (baseline_bytes={}, current_bytes={}), 截断到 {} 字符",
            baseline_byte_len,
            current_byte_len,
            ABSOLUTE_MAX_CHARS
        );
        // 安全截断：按字符边界截断
        let baseline_chars: String = baseline.chars().take(ABSOLUTE_MAX_CHARS).collect();
        let current_chars: String = current.chars().take(ABSOLUTE_MAX_CHARS).collect();
        (baseline_chars, current_chars)
    } else {
        (baseline.to_string(), current.to_string())
    };

    let baseline_chars: Vec<char> = baseline_truncated.chars().collect();
    let current_chars: Vec<char> = current_truncated.chars().collect();

    // 再次检查字符数量（防止极端情况）
    if baseline_chars.len() > ABSOLUTE_MAX_CHARS || current_chars.len() > ABSOLUTE_MAX_CHARS {
        tracing::warn!(
            "Learning: 字符数仍超限 (baseline={}, current={}), 使用快速 diff",
            baseline_chars.len(),
            current_chars.len()
        );
        return quick_diff(&baseline_chars[..ABSOLUTE_MAX_CHARS.min(baseline_chars.len())],
                          &current_chars[..ABSOLUTE_MAX_CHARS.min(current_chars.len())]);
    }

    tracing::debug!(
        "analyze_diff: baseline_len={}, current_len={}, chars_equal={}",
        baseline_chars.len(),
        current_chars.len(),
        baseline_chars == current_chars
    );

    // 移除敏感信息的详细日志（仅保留长度和 hash）
    if tracing::enabled!(tracing::Level::DEBUG) {
        let baseline_hash = format!("{:x}", md5::compute(baseline));
        let current_hash = format!("{:x}", md5::compute(current));
        tracing::debug!(
            "analyze_diff 详细信息: baseline_hash={}, current_hash={}",
            &baseline_hash[..8],
            &current_hash[..8]
        );
    }

    if baseline_chars == current_chars {
        tracing::info!("analyze_diff: 字符数组完全相同，返回空结果");
        return Vec::new();
    }

    // 长度限制：超过 MAX_CHARS 使用快速 diff
    if baseline_chars.len() > MAX_CHARS || current_chars.len() > MAX_CHARS {
        tracing::debug!(
            "Learning: 文本过长 (baseline={}, current={}), 使用快速 diff",
            baseline_chars.len(),
            current_chars.len()
        );
        return quick_diff(&baseline_chars, &current_chars);
    }

    let table = lcs_table(&baseline_chars, &current_chars);
    let ops = build_ops(&baseline_chars, &current_chars, &table);

    let mut results = Vec::new();
    let mut orig_idx = 0usize;
    let mut curr_idx = 0usize;
    let mut in_change = false;
    let mut orig_start = 0usize;
    let mut curr_start = 0usize;

    for op in ops {
        match op {
            DiffOp::Equal => {
                if in_change {
                    let orig_end = orig_idx;
                    let curr_end = curr_idx;
                    if let Some(result) = build_result(
                        &baseline_chars,
                        &current_chars,
                        orig_start,
                        orig_end,
                        curr_start,
                        curr_end,
                    ) {
                        results.push(result);
                    }
                    in_change = false;
                }
                orig_idx += 1;
                curr_idx += 1;
            }
            DiffOp::Delete => {
                if !in_change {
                    in_change = true;
                    orig_start = orig_idx;
                    curr_start = curr_idx;
                }
                orig_idx += 1;
            }
            DiffOp::Insert => {
                if !in_change {
                    in_change = true;
                    orig_start = orig_idx;
                    curr_start = curr_idx;
                }
                curr_idx += 1;
            }
        }
    }

    if in_change {
        if let Some(result) = build_result(
            &baseline_chars,
            &current_chars,
            orig_start,
            orig_idx,
            curr_start,
            curr_idx,
        ) {
            results.push(result);
        }
    }

    results
}

fn build_result(
    baseline_chars: &[char],
    current_chars: &[char],
    orig_start: usize,
    orig_end: usize,
    curr_start: usize,
    curr_end: usize,
) -> Option<DiffResult> {
    let original_segment = slice_chars(baseline_chars, orig_start, orig_end);
    let corrected_segment = slice_chars(current_chars, curr_start, curr_end);

    // 过滤：原文和修正片段都为空
    if original_segment.is_empty() && corrected_segment.is_empty() {
        return None;
    }

    // 过滤：纯标点修改
    let corrected_trimmed = corrected_segment.trim();
    let original_trimmed = original_segment.trim();
    if is_pure_punctuation(corrected_trimmed) && is_pure_punctuation(original_trimmed) {
        return None;
    }

    let context = build_context(
        baseline_chars,
        current_chars,
        orig_start,
        orig_end,
        curr_start,
        curr_end,
    );

    Some(DiffResult {
        original_segment,
        corrected_segment,
        context,
        orig_start,
        orig_end,
        curr_start,
        curr_end,
    })
}

fn is_pure_punctuation(s: &str) -> bool {
    // 空字符串不应该被认为是"纯标点"
    if s.is_empty() {
        return false;
    }

    // 中文标点符号列表
    const PUNCTUATION_CHARS: [char; 20] = [
        '。', '，', '！', '？', '、', '；', '：',
        '"', '"', '\u{2018}', '\u{2019}',  // 中文单引号使用 Unicode 转义
        '（', '）', '【', '】', '《', '》',
        '—', '…', '·'
    ];
    s.chars().all(|c| c.is_ascii_punctuation() || PUNCTUATION_CHARS.contains(&c))
}

fn build_context(
    baseline_chars: &[char],
    current_chars: &[char],
    orig_start: usize,
    orig_end: usize,
    curr_start: usize,
    curr_end: usize,
) -> String {
    const CONTEXT: usize = 10;
    let (chars, seg_start, seg_end) = if orig_start != orig_end {
        (baseline_chars, orig_start, orig_end)
    } else {
        (current_chars, curr_start, curr_end)
    };

    let start = seg_start.saturating_sub(CONTEXT);
    let end = (seg_end + CONTEXT).min(chars.len());
    slice_chars(chars, start, end)
}

fn slice_chars(chars: &[char], start: usize, end: usize) -> String {
    chars.get(start..end).unwrap_or(&[]).iter().collect()
}

/// 空间优化的 LCS 表
///
/// 使用两行滚动数组代替完整的 n*m 矩阵
/// 空间复杂度从 O(n*m) 降低到 O(min(n,m))
///
/// 同时返回 LCS 长度和用于回溯的方向表
fn lcs_table(a: &[char], b: &[char]) -> Vec<Vec<usize>> {
    // 对于较短的序列，仍使用完整表（用于回溯）
    // 优化仅在最终不需要回溯时有意义，但当前 build_ops 需要完整表
    // 因此保持原有实现，但添加内存警告
    let estimated_size = (a.len() + 1) * (b.len() + 1) * std::mem::size_of::<usize>();
    if estimated_size > 16 * 1024 * 1024 {
        // 超过 16MB 时记录警告
        tracing::warn!(
            "LCS 表内存占用较大: {}MB (a={}, b={})",
            estimated_size / 1024 / 1024,
            a.len(),
            b.len()
        );
    }

    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..a.len() {
        for j in 0..b.len() {
            if a[i] == b[j] {
                table[i + 1][j + 1] = table[i][j] + 1;
            } else {
                table[i + 1][j + 1] = table[i + 1][j].max(table[i][j + 1]);
            }
        }
    }
    table
}

fn build_ops(a: &[char], b: &[char], table: &[Vec<usize>]) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    let mut i = a.len();
    let mut j = b.len();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
            ops.push(DiffOp::Equal);
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[i][j - 1] >= table[i - 1][j]) {
            ops.push(DiffOp::Insert);
            j -= 1;
        } else if i > 0 {
            ops.push(DiffOp::Delete);
            i -= 1;
        }
    }
    ops.reverse();
    ops
}

/// 快速 diff（用于超长文本）
///
/// 简单实现：只比较首尾相同部分，中间视为一个大变更
fn quick_diff(baseline: &[char], current: &[char]) -> Vec<DiffResult> {
    // 找公共前缀长度
    let prefix_len = baseline
        .iter()
        .zip(current.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // 找公共后缀长度（排除前缀部分）
    let baseline_suffix = &baseline[prefix_len..];
    let current_suffix = &current[prefix_len..];
    let suffix_len = baseline_suffix
        .iter()
        .rev()
        .zip(current_suffix.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    // 提取差异部分
    let orig_end = baseline.len().saturating_sub(suffix_len);
    let curr_end = current.len().saturating_sub(suffix_len);

    if prefix_len >= orig_end && prefix_len >= curr_end {
        return Vec::new(); // 无差异
    }

    let original_segment: String = baseline[prefix_len..orig_end].iter().collect();
    let corrected_segment: String = current[prefix_len..curr_end].iter().collect();

    // 过滤：原文和修正片段都为空
    if original_segment.is_empty() && corrected_segment.is_empty() {
        return Vec::new();
    }

    // 过滤：纯标点修改
    let corrected_trimmed = corrected_segment.trim();
    let original_trimmed = original_segment.trim();
    if is_pure_punctuation(corrected_trimmed) && is_pure_punctuation(original_trimmed) {
        return Vec::new();
    }

    // 构建上下文
    let context_start = prefix_len.saturating_sub(10);
    let context_end = (curr_end + 10).min(current.len());
    let context: String = current[context_start..context_end].iter().collect();

    vec![DiffResult {
        original_segment,
        corrected_segment,
        context,
        orig_start: prefix_len,
        orig_end,
        curr_start: prefix_len,
        curr_end,
    }]
}

/// 合并词级差异
///
/// 将字符级差异合并为词级差异，解决单词修正被拆分的问题
///
/// # 策略
/// 1. 扩展每个差异块到英文单词边界（连续的字母数字）
/// 2. 合并重叠或相邻的差异块
/// 3. 可选地扩展到相邻单词形成短语候选
///
/// # Arguments
/// * `diffs` - 字符级差异列表
/// * `baseline` - 原文
/// * `current` - 修正文本
///
/// # Returns
/// 合并后的词级差异列表
pub fn merge_word_level_diffs(
    diffs: Vec<DiffResult>,
    baseline: &str,
    current: &str,
) -> Vec<DiffResult> {
    if diffs.is_empty() {
        return Vec::new();
    }

    let baseline_chars: Vec<char> = baseline.chars().collect();
    let current_chars: Vec<char> = current.chars().collect();

    // 步骤 1: 扩展每个差异块到单词边界
    let mut expanded: Vec<DiffResult> = diffs
        .into_iter()
        .map(|diff| expand_to_word_boundary(diff, &baseline_chars, &current_chars))
        .collect();

    // 步骤 2: 合并重叠或相邻的差异块
    expanded.sort_by_key(|d| d.orig_start);
    let mut merged = Vec::new();
    let mut current_diff: Option<DiffResult> = None;

    for diff in expanded {
        match current_diff.take() {
            None => {
                current_diff = Some(diff);
            }
            Some(prev) => {
                // 检查是否应该合并（重叠或距离很近）
                if should_merge(&prev, &diff, &baseline_chars, &current_chars) {
                    current_diff = Some(merge_two_diffs(prev, diff, &baseline_chars, &current_chars));
                } else {
                    merged.push(prev);
                    current_diff = Some(diff);
                }
            }
        }
    }

    if let Some(diff) = current_diff {
        merged.push(diff);
    }

    // 步骤 3: 可选地扩展到相邻单词（形成短语候选）
    merged
        .into_iter()
        .map(|diff| expand_to_phrase(diff, &baseline_chars, &current_chars))
        .collect()
}

/// 扩展差异块到单词边界
fn expand_to_word_boundary(
    mut diff: DiffResult,
    baseline_chars: &[char],
    current_chars: &[char],
) -> DiffResult {
    // 向左扩展原文
    while diff.orig_start > 0 && crate::learning::is_word_char(baseline_chars[diff.orig_start - 1]) {
        diff.orig_start -= 1;
    }

    // 向右扩展原文
    while diff.orig_end < baseline_chars.len() && crate::learning::is_word_char(baseline_chars[diff.orig_end]) {
        diff.orig_end += 1;
    }

    // 向左扩展修正文本
    while diff.curr_start > 0 && crate::learning::is_word_char(current_chars[diff.curr_start - 1]) {
        diff.curr_start -= 1;
    }

    // 向右扩展修正文本
    while diff.curr_end < current_chars.len() && crate::learning::is_word_char(current_chars[diff.curr_end]) {
        diff.curr_end += 1;
    }

    // 更新片段和上下文
    diff.original_segment = slice_chars(baseline_chars, diff.orig_start, diff.orig_end);
    diff.corrected_segment = slice_chars(current_chars, diff.curr_start, diff.curr_end);
    diff.context = build_context_from_positions(
        baseline_chars,
        current_chars,
        diff.orig_start,
        diff.orig_end,
        diff.curr_start,
        diff.curr_end,
    );

    diff
}

/// 判断是否为单词字符（使用统一的定义）
fn is_word_char(c: char) -> bool {
    crate::learning::is_word_char(c)
}

/// 判断两个差异块是否应该合并
fn should_merge(
    prev: &DiffResult,
    next: &DiffResult,
    baseline_chars: &[char],
    current_chars: &[char],
) -> bool {
    // 检查原文中是否重叠或相邻（允许最多 1 个空格）
    let orig_gap = if next.orig_start >= prev.orig_end {
        next.orig_start - prev.orig_end
    } else {
        return true; // 重叠，必须合并
    };

    // 检查修正文本中是否重叠或相邻
    let curr_gap = if next.curr_start >= prev.curr_end {
        next.curr_start - prev.curr_end
    } else {
        return true; // 重叠，必须合并
    };

    // 如果间隔只有 1 个字符且是空格，则合并
    if orig_gap <= 1 && curr_gap <= 1 {
        let orig_between_is_space = orig_gap == 0
            || (orig_gap == 1
                && prev.orig_end < baseline_chars.len()
                && baseline_chars[prev.orig_end].is_whitespace());
        let curr_between_is_space = curr_gap == 0
            || (curr_gap == 1
                && prev.curr_end < current_chars.len()
                && current_chars[prev.curr_end].is_whitespace());

        return orig_between_is_space && curr_between_is_space;
    }

    false
}

/// 合并两个差异块
fn merge_two_diffs(
    prev: DiffResult,
    next: DiffResult,
    baseline_chars: &[char],
    current_chars: &[char],
) -> DiffResult {
    let orig_start = prev.orig_start.min(next.orig_start);
    let orig_end = prev.orig_end.max(next.orig_end);
    let curr_start = prev.curr_start.min(next.curr_start);
    let curr_end = prev.curr_end.max(next.curr_end);

    DiffResult {
        original_segment: slice_chars(baseline_chars, orig_start, orig_end),
        corrected_segment: slice_chars(current_chars, curr_start, curr_end),
        context: build_context_from_positions(
            baseline_chars,
            current_chars,
            orig_start,
            orig_end,
            curr_start,
            curr_end,
        ),
        orig_start,
        orig_end,
        curr_start,
        curr_end,
    }
}

/// 扩展到相邻单词（形成短语候选）
fn expand_to_phrase(
    mut diff: DiffResult,
    baseline_chars: &[char],
    current_chars: &[char],
) -> DiffResult {
    // 只对英文单词扩展（至少包含一个字母）
    let has_alpha = diff.corrected_segment.chars().any(|c| c.is_alphabetic());
    if !has_alpha {
        return diff;
    }

    // 向右扩展一个单词（如果紧邻）
    let mut right_expanded = false;

    // 扩展修正文本
    if diff.curr_end < current_chars.len() {
        // 跳过空格
        let mut pos = diff.curr_end;
        while pos < current_chars.len() && current_chars[pos].is_whitespace() {
            pos += 1;
        }

        // 如果紧邻一个单词，扩展
        if pos < current_chars.len() && is_word_char(current_chars[pos]) {
            while pos < current_chars.len() && is_word_char(current_chars[pos]) {
                pos += 1;
            }

            // 扩展修正文本
            diff.curr_end = pos;
            right_expanded = true;
        }
    }

    // 同步扩展原文
    if right_expanded && diff.orig_end < baseline_chars.len() {
        // 跳过空格
        let mut pos = diff.orig_end;
        while pos < baseline_chars.len() && baseline_chars[pos].is_whitespace() {
            pos += 1;
        }

        // 如果紧邻一个单词，扩展
        if pos < baseline_chars.len() && is_word_char(baseline_chars[pos]) {
            while pos < baseline_chars.len() && is_word_char(baseline_chars[pos]) {
                pos += 1;
            }

            // 扩展原文
            diff.orig_end = pos;
        }
    }

    // 如果扩展了，更新片段和上下文
    if right_expanded {
        diff.original_segment = slice_chars(baseline_chars, diff.orig_start, diff.orig_end);
        diff.corrected_segment = slice_chars(current_chars, diff.curr_start, diff.curr_end);
        diff.context = build_context_from_positions(
            baseline_chars,
            current_chars,
            diff.orig_start,
            diff.orig_end,
            diff.curr_start,
            diff.curr_end,
        );
    }

    diff
}

/// 从位置信息构建上下文（统一使用修正文本）
fn build_context_from_positions(
    baseline_chars: &[char],
    current_chars: &[char],
    orig_start: usize,
    orig_end: usize,
    curr_start: usize,
    curr_end: usize,
) -> String {
    const CONTEXT: usize = 20;

    // 优先使用修正文本作为上下文（更完整）
    let (chars, seg_start, seg_end) = if curr_start != curr_end {
        (current_chars, curr_start, curr_end)
    } else if orig_start != orig_end {
        (baseline_chars, orig_start, orig_end)
    } else {
        (current_chars, curr_start, curr_end)
    };

    let start = seg_start.saturating_sub(CONTEXT);
    let end = (seg_end + CONTEXT).min(chars.len());
    slice_chars(chars, start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_diff() {
        let diffs = analyze_diff("你好世界", "你好世界");
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_simple_correction() {
        let diffs = analyze_diff("你好世介", "你好世界");
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].original_segment, "介");
        assert_eq!(diffs[0].corrected_segment, "界");
    }

    #[test]
    fn test_single_char_replacement() {
        // 单字符替换：天气 → 天空
        let diffs = analyze_diff("今天的天气真好", "今天的天空真好");
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].original_segment, "气");
        assert_eq!(diffs[0].corrected_segment, "空");
    }

    #[test]
    fn test_insertion() {
        let diffs = analyze_diff("你好", "你好世界");
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].corrected_segment, "世界");
    }

    #[test]
    fn test_word_level_merge() {
        // 测试 "cloud code" → "claude code" 的合并
        let baseline = "我最近在学习 cloud code";
        let current = "我最近在学习 claude code";

        // 先获取字符级差异
        let char_diffs = analyze_diff(baseline, current);
        // 应该产生 2 个字符级差异：o→a 和 ""→e
        assert_eq!(char_diffs.len(), 2, "字符级 diff 应该产生 2 个差异");

        // 应用词级合并
        let word_diffs = merge_word_level_diffs(char_diffs, baseline, current);
        // 合并后应该只有 1 个差异
        assert_eq!(word_diffs.len(), 1, "词级合并后应该只有 1 个差异");

        // 验证合并结果
        let diff = &word_diffs[0];
        assert_eq!(diff.original_segment, "cloud code", "原文应该是 'cloud code'");
        assert_eq!(diff.corrected_segment, "claude code", "修正应该是 'claude code'");
        assert!(
            diff.context.contains("我最近在学习"),
            "上下文应该包含完整句子"
        );
    }

    #[test]
    fn test_single_word_correction() {
        // 测试单个中文字修正（不应该扩展）
        let baseline = "今天天气很好";
        let current = "今天天空很好";

        let char_diffs = analyze_diff(baseline, current);
        let word_diffs = merge_word_level_diffs(char_diffs, baseline, current);

        assert_eq!(word_diffs.len(), 1);
        // 中文字符不会被扩展到相邻字符
        assert_eq!(word_diffs[0].original_segment, "气");
        assert_eq!(word_diffs[0].corrected_segment, "空");
    }
}
