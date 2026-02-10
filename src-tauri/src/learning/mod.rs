// Learning 模块 - 自动词库学习
//
// 功能：监控用户修正 ASR 识别错误，自动学习专有名词和术语
//
// 架构：
// - store: 词典条目存储管理
// - validator: ASR 文本验证器
// - diff_analyzer: 文本差异分析器
// - llm_judge: LLM 词汇判断器
// - coordinator: 学习流程协调器

pub mod coordinator;
pub mod diff_analyzer;
pub mod llm_judge;
pub mod store;
pub mod validator;

/// 统一的词字符判断函数
///
/// 用于词级 diff 合并和上下文截取，确保两处对词边界的理解一致
///
/// # 规则
/// - ASCII 字母数字（a-z, A-Z, 0-9）
/// - 下划线（_）
/// - 连字符（-）
pub(crate) fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
