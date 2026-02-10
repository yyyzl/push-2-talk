//! TNL (Technical Normalization Layer) - 技术规范化层
//!
//! 在 ASR 输出和 LLM 处理之间插入确定性规则层，处理技术串规范化。
//!
//! ## 处理流程
//! 1. Unicode 归一化 + 空白折叠
//! 2. 分词（汉字/ASCII/空白/符号）
//! 3. 识别技术片段（状态机 + 置信度打分）
//! 4. 口语符号映射（仅在技术片段内）
//! 5. 词库精确/模糊匹配（可选）

mod engine;
mod fuzzy;
mod rules;
mod tech_span;
mod tokenizer;
mod types;

pub use engine::TnlEngine;

/// 判断字符串是否仅包含 ASCII 数字
///
/// 用于邮箱用户名数字段检测、数字间空格合并等场景
#[inline]
pub(crate) fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
