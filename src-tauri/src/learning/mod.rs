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

pub mod store;
pub mod validator;
pub mod diff_analyzer;
pub mod llm_judge;
pub mod coordinator;
