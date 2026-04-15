// Omni ASR Prompt 构建器
//
// 为 Omni 多模态模型构建结构化的 system prompt，
// 包含转录规则、内置词库、用户自定义词库等信息。

use crate::dictionary_utils::entries_to_words;
use std::collections::HashSet;

/// 角色定义
const BASE_ROLE: &str =
"始终以非思考模式响应，跳过思考，直接输出答案。\n
你是一个专业的语音转录助手。你的唯一任务是将用户的语音精确转录为文字。\n
";

/// 默认转录规则
const DEFAULT_RULES: &str = "\n\n## 转录规则\n\
1. 忠实转录是第一原则和前提\n\
2. 保持语句通顺、逻辑合理\n\
3. 添加正确的标点符号\n\
4. 英文单词和专业术语保持原文拼写和大小写\n\
5. 数字使用阿拉伯数字\n\
6. 只输出转录文本，不加任何解释或前缀";

/// 构建 Omni ASR 的 system prompt
///
/// 结构：
/// 1. 角色定义 + 基础规则
/// 2. 用户自定义规则（如有）
/// 3. 内置词库（按领域分类，如有）
/// 4. 用户自定义词库（如有）
/// 5. 输出指令
pub fn build(
    user_dictionary: &[String],
    builtin_raw: &str,
    selected_builtin_domains: &[String],
    include_builtin: bool,
    custom_rules: &str,
) -> String {
    let mut prompt = String::with_capacity(4096);

    // 角色定义
    prompt.push_str(BASE_ROLE);

    // 基础转录规则
    prompt.push_str(DEFAULT_RULES);

    // 用户自定义规则（追加）
    if !custom_rules.trim().is_empty() {
        prompt.push_str("\n\n## 额外规则\n");
        prompt.push_str(custom_rules);
    }

    // 内置词库（按领域解析并格式化）
    if include_builtin && !builtin_raw.trim().is_empty() {
        let domains = format_builtin_domains(builtin_raw, selected_builtin_domains);
        if !domains.is_empty() {
            prompt.push_str("\n\n## 专业词汇表（按领域分类）\n");
            prompt.push_str(&domains);
        }
    }

    // 用户自定义词库（提纯后列出）
    let purified = entries_to_words(user_dictionary);
    if !purified.is_empty() {
        prompt.push_str("\n\n## 用户自定义词汇\n");
        prompt.push_str("以下词汇必须严格使用指定写法：\n");
        for word in &purified {
            prompt.push_str(&format!("- {}\n", word));
        }
    }

    // 输出指令
    prompt.push_str("\n\n## 输出\n直接输出转录文本，不要加任何其他内容。");

    prompt
}

/// 将 【领域】:[词1,词2,...] 格式解析为 Markdown
///
/// 输入示例：
///   【AI】:[OpenAI,GPT-4,Claude]
///   【编程】:[Rust,TypeScript]
///
/// 输出示例：
///   ### AI
///   OpenAI, GPT-4, Claude
///
///   ### 编程
///   Rust, TypeScript
fn format_builtin_domains(raw: &str, selected_builtin_domains: &[String]) -> String {
    let mut result = String::new();
    let selected_domain_set: HashSet<&str> = selected_builtin_domains
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|domain| !domain.is_empty())
        .collect();

    if selected_domain_set.is_empty() {
        return result;
    }

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 解析 【领域】:[词1,词2,...] 格式
        if let Some((domain, words_part)) = parse_domain_line(line) {
            if !selected_domain_set.contains(domain.trim()) {
                continue;
            }
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format!("### {}\n", domain));

            // 将词汇用逗号+空格分隔
            let words: Vec<&str> = words_part
                .split(',')
                .map(|w| w.trim())
                .filter(|w| !w.is_empty())
                .collect();
            result.push_str(&words.join(", "));
            result.push('\n');
        }
    }

    result
}

/// 解析单行领域格式：【领域】:[词1,词2,...]
///
/// 返回 (领域名, 词汇部分原始字符串)
fn parse_domain_line(line: &str) -> Option<(&str, &str)> {
    // 查找【和】
    let domain_start = line.find('\u{3010}')?; // 【
    let domain_end = line.find('\u{3011}')?; // 】
    if domain_end <= domain_start {
        return None;
    }

    let domain = &line[domain_start + '\u{3010}'.len_utf8()..domain_end];
    if domain.is_empty() {
        return None;
    }

    // 查找冒号后的方括号内容
    let after_domain = &line[domain_end + '\u{3011}'.len_utf8()..];
    let colon_pos = after_domain.find(':')?;
    let after_colon = &after_domain[colon_pos + 1..];

    // 去掉方括号
    let words = after_colon.trim();
    let words = words.strip_prefix('[').unwrap_or(words);
    let words = words.strip_suffix(']').unwrap_or(words);

    Some((domain, words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_minimal_prompt() {
        let prompt = build(&[], "", &[], false, "");
        assert!(prompt.contains("专业的语音转录助手"));
        assert!(prompt.contains("转录规则"));
        assert!(prompt.contains("直接输出转录文本"));
        // 无词库时不应包含词汇表
        assert!(!prompt.contains("专业词汇表"));
        assert!(!prompt.contains("用户自定义词汇"));
    }

    #[test]
    fn test_build_starts_with_no_thinking_instruction() {
        let prompt = build(&[], "", &[], false, "");
        assert!(prompt.starts_with("始终以非思考模式响应"));
    }

    #[test]
    fn test_build_with_custom_rules() {
        let prompt = build(&[], "", &[], false, "金额用阿拉伯数字+单位");
        assert!(prompt.contains("额外规则"));
        assert!(prompt.contains("金额用阿拉伯数字+单位"));
    }

    #[test]
    fn test_build_with_user_dictionary() {
        let dict = vec!["Claude".to_string(), "PushToTalk|auto".to_string()];
        let prompt = build(&dict, "", &[], false, "");
        assert!(prompt.contains("用户自定义词汇"));
        assert!(prompt.contains("- Claude"));
        assert!(prompt.contains("- PushToTalk"));
        // auto 后缀应被去除
        assert!(!prompt.contains("|auto"));
    }

    #[test]
    fn test_format_builtin_domains_should_only_include_selected_domains() {
        let raw = "【AI】:[OpenAI,GPT-4,Claude]\n【编程】:[Rust,TypeScript]";
        let result = format_builtin_domains(raw, &["AI".to_string()]);
        assert!(result.contains("### AI"));
        assert!(result.contains("OpenAI, GPT-4, Claude"));
        assert!(!result.contains("### 编程"));
        assert!(!result.contains("Rust, TypeScript"));
    }

    #[test]
    fn test_build_with_builtin_domains_should_respect_switch_and_selection() {
        let raw = "【AI】:[OpenAI,GPT-4]\n【编程】:[Rust,TypeScript]";

        let enabled_prompt = build(&[], raw, &["AI".to_string()], true, "");
        assert!(enabled_prompt.contains("### AI"));
        assert!(!enabled_prompt.contains("### 编程"));

        let disabled_prompt = build(&[], raw, &["AI".to_string()], false, "");
        assert!(!disabled_prompt.contains("### AI"));
    }

    #[test]
    fn test_parse_domain_line() {
        let (domain, words) = parse_domain_line("【AI】:[OpenAI,GPT-4]").unwrap();
        assert_eq!(domain, "AI");
        assert_eq!(words, "OpenAI,GPT-4");
    }

    #[test]
    fn test_parse_domain_line_invalid() {
        assert!(parse_domain_line("no domain here").is_none());
        assert!(parse_domain_line("").is_none());
    }
}
