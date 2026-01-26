// ASR 文本验证器
//
// 功能：验证焦点文本是否包含 ASR 原文
// 使用 Levenshtein 距离算法计算相似度

/// 最大处理字符数（超过此长度跳过验证）
const MAX_CHARS: usize = 2048;

/// 验证焦点文本是否包含 ASR 原文
///
/// # Arguments
/// * `focused_text` - 当前焦点窗口的文本
/// * `asr_original` - ASR 识别的原始文本
/// * `similarity_threshold` - 相似度阈值（默认 0.8）
///
/// # Returns
/// * `true` - 验证通过（文本匹配）
/// * `false` - 验证失败（如 VS Code 等无法读取文本的应用）
pub fn is_asr_text_present(focused_text: &str, asr_original: &str, similarity_threshold: f64) -> bool {
    let focused = focused_text.trim();
    let asr = asr_original.trim();

    if asr.is_empty() {
        return false;
    }

    // 长度限制：超过 MAX_CHARS 跳过相似度计算，仅检查包含
    let focused_len = focused.chars().count();
    let asr_len = asr.chars().count();

    if focused_len > MAX_CHARS || asr_len > MAX_CHARS {
        tracing::debug!(
            "Learning: 文本过长 (focused={}, asr={}), 仅检查包含关系",
            focused_len,
            asr_len
        );
        return focused.contains(asr);
    }

    // 1. 双向包含检查
    // - focused.contains(asr): 用户在 ASR 文本基础上增加内容
    // - asr.contains(focused): 用户删除了部分内容（删除场景）
    if focused.contains(asr) || asr.contains(focused) {
        return true;
    }

    // 2. 相似度检查
    let similarity = calculate_similarity(focused, asr);
    similarity >= similarity_threshold
}

/// 计算两个字符串的相似度（基于 Levenshtein 距离）
///
/// 返回值范围 [0.0, 1.0]，1.0 表示完全相同
fn calculate_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let distance = levenshtein_distance(a, b);
    let max_len = a.chars().count().max(b.chars().count());
    1.0 - (distance as f64 / max_len as f64)
}

/// Levenshtein 距离算法
///
/// 计算将字符串 a 转换为字符串 b 所需的最小编辑次数
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            let deletion = prev[j + 1] + 1;
            let insertion = curr[j] + 1;
            let substitution = prev[j] + cost;
            curr[j + 1] = deletion.min(insertion).min(substitution);
        }
        prev.clone_from(&curr);
    }

    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(is_asr_text_present("你好世界", "你好世界", 0.8));
    }

    #[test]
    fn test_contains() {
        assert!(is_asr_text_present("前缀你好世界后缀", "你好世界", 0.8));
    }

    #[test]
    fn test_reverse_contains() {
        // 用户删除了部分内容：焦点文本是 ASR 的子集
        assert!(is_asr_text_present("你好", "你好世界", 0.8));
        assert!(is_asr_text_present("世界", "你好世界", 0.8));
    }

    #[test]
    fn test_similar() {
        // "你好世界" vs "你好世介" 相似度约 0.75
        assert!(!is_asr_text_present("你好世介", "你好世界", 0.8));
        assert!(is_asr_text_present("你好世介", "你好世界", 0.7));
    }

    #[test]
    fn test_empty() {
        assert!(!is_asr_text_present("", "你好", 0.8));
        assert!(!is_asr_text_present("你好", "", 0.8));
    }
}
