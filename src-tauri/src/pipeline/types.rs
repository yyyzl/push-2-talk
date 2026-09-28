// Pipeline 核心类型定义
//
// 定义了处理管道所需的所有类型，包括：
// - 转录模式 (TranscriptionMode)
// - 转录上下文 (TranscriptionContext)
// - 处理结果 (PipelineResult)

use serde::Serialize;

use crate::tnl::TnlDiagnostics;

pub use crate::config::TranscriptionMode;

/// Pipeline 处理结果
///
/// 兼容现有的 TranscriptionResult，同时支持扩展字段
#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    /// 最终处理后的文本
    pub text: String,
    /// 原始 ASR 文本（仅在 LLM 处理后与 text 不同时有值）
    pub original_text: Option<String>,
    /// 用户选中的引用文本（仅 AI 助手模式有值）
    pub selected_text: Option<String>,
    /// ASR 耗时（毫秒）
    pub asr_time_ms: u64,
    /// LLM 处理耗时（毫秒）
    pub llm_time_ms: Option<u64>,
    /// 总耗时（毫秒）
    pub total_time_ms: u64,
    /// 处理模式
    pub mode: TranscriptionMode,
    /// 是否已自动插入文本
    pub inserted: bool,
    /// 可选 TNL 候选诊断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tnl_diagnostics: Option<TnlDiagnostics>,
}

impl PipelineResult {
    /// 创建成功结果
    pub fn success(
        text: String,
        original_text: Option<String>,
        selected_text: Option<String>,
        asr_time_ms: u64,
        llm_time_ms: Option<u64>,
        mode: TranscriptionMode,
        inserted: bool,
    ) -> Self {
        Self {
            text,
            original_text,
            selected_text,
            asr_time_ms,
            llm_time_ms,
            total_time_ms: asr_time_ms + llm_time_ms.unwrap_or(0),
            mode,
            inserted,
            tnl_diagnostics: None,
        }
    }
}
