// 普通模式处理管道
//
// 处理流程：ASR结果 → 可选LLM润色 → 自动插入文本
//
// 这是默认的处理模式，保持与原有行为完全兼容
//
// 设计原则：Pipeline 不持有锁，所有依赖通过参数传入

use anyhow::Result;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use crate::llm_post_processor::LlmPostProcessor;
use crate::text_inserter::TextInserter;
use super::types::{PipelineResult, TranscriptionContext, TranscriptionMode};

/// 普通模式处理管道
///
/// 职责：
/// 1. 可选的 LLM 后处理（润色、翻译等）
/// 2. 自动插入文本到当前活动窗口
///
/// 设计：无状态，所有依赖通过 process() 参数传入
pub struct NormalPipeline;

impl NormalPipeline {
    /// 创建普通模式管道
    pub fn new() -> Self {
        Self
    }

    /// 处理 ASR 结果
    ///
    /// # Arguments
    /// * `app` - Tauri 应用句柄（用于发送事件）
    /// * `post_processor` - LLM 后处理器（调用方负责从锁中获取）
    /// * `text_inserter` - 文本插入器（调用方负责从锁中获取）
    /// * `asr_result` - ASR 转录结果
    /// * `asr_time_ms` - ASR 耗时（毫秒）
    /// * `_context` - 上下文（普通模式不使用）
    /// * `target_hwnd` - 目标窗口句柄（用于焦点恢复）
    ///
    /// # Returns
    /// * `Ok(PipelineResult)` - 处理成功
    /// * `Err(e)` - 处理失败
    pub async fn process(
        &self,
        app: &AppHandle,
        post_processor: Option<LlmPostProcessor>,
        text_inserter: &mut Option<TextInserter>,
        asr_result: Result<String>,
        asr_time_ms: u64,
        _context: TranscriptionContext,  // 普通模式不使用上下文
        target_hwnd: Option<isize>,      // 目标窗口句柄（用于焦点恢复）
    ) -> Result<PipelineResult> {
        // 1. 解包 ASR 结果
        let text = asr_result?;
        tracing::info!("NormalPipeline: 收到 ASR 结果: {} (耗时: {}ms)", text, asr_time_ms);

        // 2. 可选 LLM 后处理
        let (final_text, original_text, llm_time_ms) = Self::maybe_polish(app, post_processor, &text).await;

        // 3. 插入前隐藏窗口并主动恢复焦点到目标应用
        // 使用新的焦点恢复机制，确保文本插入到正确的窗口
        super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 4. 插入文本
        let inserted = Self::insert_text(text_inserter, &final_text);

        // 5. 返回结果
        Ok(PipelineResult::success(
            final_text,
            original_text,
            asr_time_ms,
            llm_time_ms,
            TranscriptionMode::Normal,
            inserted,
        ))
    }

    /// 可选的 LLM 后处理
    ///
    /// 如果配置了 LLM 后处理器，则调用它进行润色
    /// 失败时返回原文
    async fn maybe_polish(
        app: &AppHandle,
        processor: Option<LlmPostProcessor>,
        text: &str,
    ) -> (String, Option<String>, Option<u64>) {
        if let Some(processor) = processor {
            tracing::info!("NormalPipeline: 开始 LLM 后处理...");
            let _ = app.emit("post_processing", ());

            let llm_start = Instant::now();
            match processor.polish_transcript(text).await {
                Ok(polished) => {
                    let llm_elapsed = llm_start.elapsed().as_millis() as u64;
                    tracing::info!(
                        "NormalPipeline: LLM 后处理完成: {} (耗时: {}ms)",
                        polished,
                        llm_elapsed
                    );
                    (polished, Some(text.to_string()), Some(llm_elapsed))
                }
                Err(e) => {
                    tracing::warn!("NormalPipeline: LLM 后处理失败，使用原文: {}", e);
                    (text.to_string(), None, None)
                }
            }
        } else {
            (text.to_string(), None, None)
        }
    }

    /// 插入文本到当前活动窗口
    ///
    /// 返回是否成功插入
    fn insert_text(text_inserter: &mut Option<TextInserter>, text: &str) -> bool {
        if let Some(ref mut inserter) = text_inserter {
            match inserter.insert_text(text) {
                Ok(()) => {
                    tracing::info!("NormalPipeline: 文本插入成功");
                    true
                }
                Err(e) => {
                    tracing::error!("NormalPipeline: 插入文本失败: {}", e);
                    false
                }
            }
        } else {
            tracing::warn!("NormalPipeline: TextInserter 未初始化");
            false
        }
    }

}

impl Default for NormalPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let _pipeline = NormalPipeline::new();
        // Pipeline 现在是无状态的，只需要能创建即可
    }
}
