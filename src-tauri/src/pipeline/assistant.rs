// AI 助手模式处理管道
//
// 处理流程：
// 1. 如果有选中文本：上下文 + 语音指令 → ASR → AssistantProcessor (文本处理模式) → 自动插入（替换选中）
// 2. 如果无选中文本：语音指令 → ASR → AssistantProcessor (问答模式) → 自动插入
//
// 使用独立的 AssistantProcessor，支持双系统提示词

use anyhow::Result;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use crate::assistant_processor::AssistantProcessor;
use crate::clipboard_manager::{ClipboardGuard, insert_text_with_context};
use super::types::{PipelineResult, TranscriptionContext, TranscriptionMode};

/// AI 助手模式处理管道
///
/// 职责：
/// 1. 接收 ASR 转写的用户指令
/// 2. 根据是否有选中文本选择合适的系统提示词
/// 3. 调用 AssistantProcessor 进行处理
/// 4. 将回答自动插入到当前光标位置（替换选中或插入）
/// 5. 恢复原始剪贴板
pub struct AssistantPipeline;

impl AssistantPipeline {
    /// 创建 AI 助手模式管道
    pub fn new() -> Self {
        Self
    }

    /// 处理 ASR 结果
    ///
    /// # Arguments
    /// * `app` - Tauri 应用句柄（用于发送事件）
    /// * `processor` - AI 助手处理器（调用方负责从锁中获取）
    /// * `clipboard_guard` - 剪贴板守卫（用于恢复）
    /// * `asr_result` - ASR 转录结果（用户的语音指令）
    /// * `asr_time_ms` - ASR 耗时（毫秒）
    /// * `context` - 上下文信息（包含选中文本）
    /// * `target_hwnd` - 目标窗口句柄（用于焦点恢复）
    ///
    /// # Returns
    /// * `Ok(PipelineResult)` - 处理成功
    /// * `Err(e)` - 处理失败
    pub async fn process(
        &self,
        app: &AppHandle,
        processor: Option<AssistantProcessor>,
        clipboard_guard: Option<ClipboardGuard>,
        asr_result: Result<String>,
        asr_time_ms: u64,
        context: TranscriptionContext,
        target_hwnd: Option<isize>,  // 目标窗口句柄（用于焦点恢复）
    ) -> Result<PipelineResult> {
        // 1. 解包 ASR 结果（用户指令）
        let user_instruction = asr_result?;
        tracing::info!(
            "AssistantPipeline: 收到用户指令: {} (ASR耗时: {}ms)",
            user_instruction,
            asr_time_ms
        );

        // 2. 检查 AssistantProcessor 是否可用
        let Some(processor) = processor else {
            anyhow::bail!("AI 助手模式需要配置 LLM，请先在设置中配置 AI 助手 API");
        };

        // 3. 发送处理中事件
        let _ = app.emit("post_processing", ());
        let llm_start = Instant::now();

        // 4. 根据是否有选中文本选择处理方式
        let result = if let Some(ref selected_text) = context.selected_text {
            // 有选中文本：使用文本处理模式
            tracing::info!(
                "AssistantPipeline: 文本处理模式 (选中文本: {} 字符)",
                selected_text.len()
            );
            processor.process_with_context(&user_instruction, selected_text).await?
        } else {
            // 无选中文本：使用问答模式
            tracing::info!("AssistantPipeline: 问答模式");
            processor.process(&user_instruction).await?
        };

        let llm_time_ms = llm_start.elapsed().as_millis() as u64;
        tracing::info!(
            "AssistantPipeline: LLM 回答: {} (LLM耗时: {}ms)",
            result,
            llm_time_ms
        );

        // 5. 插入前隐藏窗口并主动恢复焦点到目标应用
        // 使用新的焦点恢复机制，确保文本插入到正确的窗口
        super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 6. 插入结果（替换选中或插入at 光标）
        let has_selection = context.selected_text.is_some();
        let inserted = Self::insert_result(&result, has_selection, clipboard_guard);

        // 7. 返回结果
        Ok(PipelineResult::success(
            result,
            Some(user_instruction),
            asr_time_ms,
            Some(llm_time_ms),
            TranscriptionMode::Assistant,
            inserted,
        ))
    }

    /// 插入文本到当前光标位置
    fn insert_result(text: &str, has_selection: bool, guard: Option<ClipboardGuard>) -> bool {
        match insert_text_with_context(text, has_selection, guard) {
            Ok(()) => {
                tracing::info!("AssistantPipeline: 结果已插入");
                true
            }
            Err(e) => {
                tracing::error!("AssistantPipeline: 插入失败: {}", e);
                false
            }
        }
    }

}

impl Default for AssistantPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let _pipeline = AssistantPipeline::new();
        // Pipeline 是无状态的，只需要能创建即可
    }
}
