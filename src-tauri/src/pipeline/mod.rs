// Pipeline 模块 - 结果处理管道
//
// 支持多种处理模式：
// - Normal: 普通模式（ASR → 可选LLM润色 → 自动插入）
// - Assistant: AI 助手模式（双系统提示词，上下文感知）
// - 未来可扩展更多模式...

mod types;
mod normal;
mod assistant;
pub mod focus;

pub use types::*;
pub use normal::NormalPipeline;
pub use assistant::AssistantPipeline;
