// 普通模式处理管道
//
// 处理流程：ASR结果 → 可选LLM润色 → 自动插入文本
//
// 这是默认的处理模式，保持与原有行为完全兼容
//
// 设计原则：Pipeline 不持有锁，所有依赖通过参数传入

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use super::types::{PipelineResult, TranscriptionContext, TranscriptionMode};
use crate::config::AppConfig;
use crate::learning::coordinator::start_learning_observation;
use crate::llm_post_processor::LlmPostProcessor;
use crate::personalization::{
    default_correction_pairs_path, ConversionResult, CorrectionPairStore, PersonalizationEngine,
};
use crate::text_inserter::TextInserter;
use crate::tnl::{TnlCandidateDecision, TnlDiagnostics, TnlEngine};

const CANDIDATE_ARBITRATION_TIMEOUT_MS: u64 = 800;
const MAX_PERSONALIZATION_DIAGNOSTIC_TEXT_CHARS: usize = 160;
const MAX_PERSONALIZATION_DIAGNOSTIC_CANDIDATES: usize = 20;
const SECS_PER_DAY: u64 = 86_400;

#[derive(Debug, Serialize)]
struct PersonalizationRuntimeDiagnosticPayload {
    schema_version: u8,
    stage: &'static str,
    timestamp_ms: u128,
    source_text: String,
    output_text: String,
    changed: bool,
    elapsed_us: u64,
    candidate_count: usize,
    applied_count: usize,
    pass_summaries: Vec<Value>,
    candidates: Vec<Value>,
    applied: Vec<Value>,
}

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
        enable_post_process: bool,
        dictionary: Vec<String>,
        enable_dictionary_enhancement: bool,
        text_inserter: &mut Option<TextInserter>,
        asr_result: Result<String>,
        asr_time_ms: u64,
        _context: TranscriptionContext, // 普通模式不使用上下文
        target_hwnd: Option<isize>,     // 目标窗口句柄（用于焦点恢复）
    ) -> Result<PipelineResult> {
        // 1. 解包 ASR 结果
        let asr_text = asr_result?;
        tracing::info!(
            "NormalPipeline: 收到 ASR 结果: {} (耗时: {}ms)",
            asr_text,
            asr_time_ms
        );

        // 2. TNL 技术规范化（如果启用）
        let tnl_enabled = AppConfig::load()
            .map(|(c, _)| c.tnl_config.enabled)
            .unwrap_or(true);
        let (text, tnl_changed, tnl_diagnostics) = if tnl_enabled {
            let engine = TnlEngine::new(dictionary.clone());
            let tnl_result = engine.normalize(&asr_text);
            if tnl_result.changed {
                tracing::info!(
                    "NormalPipeline: TNL 规范化: {} → {} (耗时: {}us, 替换: {})",
                    asr_text,
                    tnl_result.text,
                    tnl_result.elapsed_us,
                    tnl_result.applied.len()
                );
            }
            (tnl_result.text, tnl_result.changed, tnl_result.diagnostics)
        } else {
            (asr_text.clone(), false, None)
        };

        // 2.5. 本地个性化二次解码（MVP：仅当 correction_pairs.json 存在时启用）
        let (text, personalization_changed) = if tnl_enabled {
            Self::maybe_apply_personalization(text)
        } else {
            (text, false)
        };

        // 注意：历史记录存储 ASR 原文（asr_text），LLM 处理使用 TNL/个性化后的文本（text）

        // 3. 可选候选仲裁（绑定词库增强开关，不改变全文润色逻辑）
        let pre_arbitration_text = text.clone();
        let (text, tnl_diagnostics, candidate_llm_time_ms) = Self::maybe_arbitrate_candidates(
            post_processor.clone(),
            enable_dictionary_enhancement,
            text,
            tnl_diagnostics,
        )
        .await;
        let candidate_changed = text != pre_arbitration_text;

        // 4. 可选 LLM 后处理
        let (final_text, original_text, llm_time_ms) = Self::maybe_polish(
            app,
            post_processor,
            enable_post_process,
            &dictionary,
            enable_dictionary_enhancement,
            &text,
        )
        .await;
        let combined_llm_time_ms = Self::sum_llm_time(candidate_llm_time_ms, llm_time_ms);

        // 5. 插入前隐藏窗口并主动恢复焦点到目标应用
        // 使用新的焦点恢复机制，确保文本插入到正确的窗口
        super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await;

        // 6. 插入文本
        let inserted = Self::insert_text(text_inserter, &final_text);

        // 7. 触发学习观察（如果启用且插入成功）
        if inserted {
            if let Some(hwnd) = target_hwnd {
                if let Ok((config, _)) = AppConfig::load() {
                    if config.learning_config.enabled {
                        start_learning_observation(
                            app.clone(),
                            final_text.clone(),
                            hwnd,
                            config.learning_config,
                        );
                    }
                }
            }
        }

        // 8. 返回结果
        // 历史记录存储 ASR 原文（约束 C14）
        // 决定是否显示双栏：
        // - 有 LLM 处理 → 使用 LLM 返回的 original_text
        // - 无 LLM 处理但 TNL/候选仲裁改变了文本 → 设置原文以便前端显示双栏
        // - 无 LLM 处理且 TNL 未改变文本 → 不显示双栏（original_text = None）
        let history_original = if original_text.is_some() {
            original_text
        } else if tnl_changed || personalization_changed || candidate_changed {
            Some(asr_text)
        } else {
            None
        };

        let mut result = PipelineResult::success(
            final_text,
            history_original,
            None, // 普通模式无引用文本
            asr_time_ms,
            combined_llm_time_ms,
            TranscriptionMode::Normal,
            inserted,
        );
        result.tnl_diagnostics = tnl_diagnostics;

        Ok(result)
    }

    async fn maybe_arbitrate_candidates(
        processor: Option<LlmPostProcessor>,
        enable_dictionary_enhancement: bool,
        text: String,
        diagnostics: Option<TnlDiagnostics>,
    ) -> (String, Option<TnlDiagnostics>, Option<u64>) {
        let Some(mut diagnostics) = diagnostics else {
            return (text, None, None);
        };

        if !diagnostics.has_pending_llm() {
            return (text, Some(diagnostics), None);
        }

        if !enable_dictionary_enhancement {
            diagnostics.mark_pending_skipped(
                TnlCandidateDecision::SkippedDisabled,
                "dictionary_enhancement_disabled",
                None,
            );
            return (text, Some(diagnostics), None);
        }

        let Some(processor) = processor else {
            diagnostics.mark_pending_skipped(
                TnlCandidateDecision::SkippedNoProcessor,
                "llm_not_configured",
                None,
            );
            return (text, Some(diagnostics), None);
        };

        tracing::info!(
            "NormalPipeline: 开始 TNL 候选仲裁，候选数: {}",
            diagnostics.pending_llm_count()
        );

        let fallback_diagnostics = diagnostics.clone();
        let arbitration = tokio::time::timeout(
            Duration::from_millis(CANDIDATE_ARBITRATION_TIMEOUT_MS),
            processor.arbitrate_tnl_candidates(&text, diagnostics),
        )
        .await;

        match arbitration {
            Ok(Ok(result)) => {
                tracing::info!(
                    "NormalPipeline: TNL 候选仲裁完成 (耗时: {}ms)",
                    result.elapsed_ms
                );
                (
                    result.text,
                    Some(result.diagnostics),
                    Some(result.elapsed_ms),
                )
            }
            Ok(Err(e)) => {
                tracing::warn!("NormalPipeline: TNL 候选仲裁失败，保守跳过: {}", e);
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedError,
                    "arbitration_error",
                    None,
                );
                (text, Some(diagnostics), None)
            }
            Err(_) => {
                tracing::warn!("NormalPipeline: TNL 候选仲裁超时，保守跳过");
                let mut diagnostics = fallback_diagnostics;
                diagnostics.mark_pending_skipped(
                    TnlCandidateDecision::SkippedTimeout,
                    "arbitration_timeout",
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                );
                (
                    text,
                    Some(diagnostics),
                    Some(CANDIDATE_ARBITRATION_TIMEOUT_MS),
                )
            }
        }
    }

    fn maybe_apply_personalization(text: String) -> (String, bool) {
        let Ok(path) = default_correction_pairs_path() else {
            return (text, false);
        };

        if !path.exists() {
            return (text, false);
        }

        let store = match CorrectionPairStore::load_json(&path) {
            Ok(store) => store,
            Err(e) => {
                tracing::warn!("NormalPipeline: 加载个性化纠错对失败，保守跳过: {}", e);
                return (text, false);
            }
        };

        let source_text = text.clone();
        let (text, changed, result, elapsed_us) = Self::run_personalization_with_store(text, store);
        if let Err(e) = Self::write_personalization_diagnostic(&source_text, &result, elapsed_us) {
            tracing::warn!("NormalPipeline: 写入个性化诊断失败，已忽略: {}", e);
        }
        Self::log_personalization_result(&source_text, &result);

        (text, changed)
    }

    #[cfg(test)]
    fn apply_personalization_with_store(
        text: String,
        store: CorrectionPairStore,
    ) -> (String, bool) {
        let source_text = text.clone();
        let (text, changed, result, _) = Self::run_personalization_with_store(text, store);
        Self::log_personalization_result(&source_text, &result);

        (text, changed)
    }

    fn run_personalization_with_store(
        text: String,
        store: CorrectionPairStore,
    ) -> (String, bool, ConversionResult, u64) {
        let engine = PersonalizationEngine::new(store);
        let started_at = Instant::now();
        let result = engine.convert(&text);
        let elapsed_us = started_at.elapsed().as_micros() as u64;

        (result.text.clone(), result.changed, result, elapsed_us)
    }

    fn log_personalization_result(source_text: &str, result: &ConversionResult) {
        if result.changed {
            tracing::info!(
                "NormalPipeline: 个性化二次解码: {} → {} (应用: {}, 候选: {})",
                source_text,
                result.text,
                result.diagnostics.applied.len(),
                result.diagnostics.candidates.len()
            );
        }
    }

    fn sum_llm_time(first: Option<u64>, second: Option<u64>) -> Option<u64> {
        match (first, second) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// 可选的 LLM 后处理
    ///
    /// 如果配置了 LLM 后处理器，则调用它进行润色
    /// 失败时返回原文
    async fn maybe_polish(
        app: &AppHandle,
        processor: Option<LlmPostProcessor>,
        enable_post_process: bool,
        dictionary: &[String],
        enable_dictionary_enhancement: bool,
        text: &str,
    ) -> (String, Option<String>, Option<u64>) {
        if !enable_post_process && !enable_dictionary_enhancement {
            return (text.to_string(), None, None);
        }

        // 仅开启词库增强且词库为空：无需调用 LLM
        if !enable_post_process && enable_dictionary_enhancement && dictionary.is_empty() {
            return (text.to_string(), None, None);
        }

        if let Some(processor) = processor {
            tracing::info!("NormalPipeline: 开始 LLM 后处理...");
            let _ = app.emit("post_processing", "polishing");

            let llm_start = Instant::now();
            match processor
                .polish_transcript(
                    text,
                    dictionary,
                    enable_post_process,
                    enable_dictionary_enhancement,
                )
                .await
            {
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
                    // 通知前端润色失败（脱敏：只发送通用提示，不暴露底层错误细节）
                    let _ = app.emit("polishing_failed", "润色服务暂时不可用");
                    // original_text 保持 None，避免被前端误判为"有润色结果"
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

    fn write_personalization_diagnostic(
        source_text: &str,
        result: &ConversionResult,
        elapsed_us: u64,
    ) -> Result<PathBuf> {
        let diagnostics_dir = Self::runtime_personalization_diagnostics_dir()?;
        Self::write_personalization_diagnostic_to_dir(
            &diagnostics_dir,
            source_text,
            result,
            elapsed_us,
            Self::current_unix_millis(),
        )
    }

    fn runtime_personalization_diagnostics_dir() -> Result<PathBuf> {
        let config_path = AppConfig::config_path()?;
        let config_dir = config_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let now_secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        Ok(config_dir
            .join("diagnostics")
            .join(Self::utc_date_dir_from_unix_secs(now_secs)))
    }

    fn write_personalization_diagnostic_to_dir(
        output_dir: &Path,
        source_text: &str,
        result: &ConversionResult,
        elapsed_us: u64,
        timestamp_ms: u128,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let path = output_dir.join(format!(
            "personalization-{}-{}.json",
            timestamp_ms,
            uuid::Uuid::new_v4()
        ));
        let payload =
            Self::personalization_diagnostic_payload(source_text, result, elapsed_us, timestamp_ms);
        let content = serde_json::to_string_pretty(&payload)?;
        std::fs::write(&path, content)?;
        Ok(path)
    }

    fn personalization_diagnostic_payload(
        source_text: &str,
        result: &ConversionResult,
        elapsed_us: u64,
        timestamp_ms: u128,
    ) -> PersonalizationRuntimeDiagnosticPayload {
        PersonalizationRuntimeDiagnosticPayload {
            schema_version: 1,
            stage: "personalization",
            timestamp_ms,
            source_text: Self::truncate_chars(
                source_text,
                MAX_PERSONALIZATION_DIAGNOSTIC_TEXT_CHARS,
            ),
            output_text: Self::truncate_chars(
                &result.text,
                MAX_PERSONALIZATION_DIAGNOSTIC_TEXT_CHARS,
            ),
            changed: result.changed,
            elapsed_us,
            candidate_count: result.diagnostics.candidates.len(),
            applied_count: result.diagnostics.applied.len(),
            pass_summaries: result
                .diagnostics
                .pass_summaries
                .iter()
                .map(Self::bounded_json)
                .collect(),
            candidates: result
                .diagnostics
                .candidates
                .iter()
                .take(MAX_PERSONALIZATION_DIAGNOSTIC_CANDIDATES)
                .map(Self::bounded_json)
                .collect(),
            applied: result
                .diagnostics
                .applied
                .iter()
                .take(MAX_PERSONALIZATION_DIAGNOSTIC_CANDIDATES)
                .map(Self::bounded_json)
                .collect(),
        }
    }

    fn current_unix_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }

    fn utc_date_dir_from_unix_secs(secs: u64) -> String {
        let days = (secs / SECS_PER_DAY) as i64;
        let (year, month, day) = Self::civil_from_unix_days(days);
        format!("{year:04}-{month:02}-{day:02}")
    }

    fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_index + 2) / 5 + 1;
        let month = month_index + if month_index < 10 { 3 } else { -9 };
        let year = year + i64::from(month <= 2);

        (year as i32, month as u32, day as u32)
    }

    fn bounded_json<T: Serialize>(payload: &T) -> Value {
        let mut value = serde_json::to_value(payload).unwrap_or(Value::Null);
        Self::truncate_json_strings(&mut value, MAX_PERSONALIZATION_DIAGNOSTIC_TEXT_CHARS);
        value
    }

    fn truncate_json_strings(value: &mut Value, max_chars: usize) {
        match value {
            Value::String(text) => {
                *text = Self::truncate_chars(text, max_chars);
            }
            Value::Array(values) => {
                for item in values {
                    Self::truncate_json_strings(item, max_chars);
                }
            }
            Value::Object(values) => {
                for item in values.values_mut() {
                    Self::truncate_json_strings(item, max_chars);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    fn truncate_chars(value: &str, max_chars: usize) -> String {
        if value.chars().count() <= max_chars {
            return value.to_string();
        }

        let truncated = value.chars().take(max_chars).collect::<String>();
        format!("{truncated}...")
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
    use crate::personalization::{CorrectionPair, CorrectionPairStore};

    #[test]
    fn test_pipeline_creation() {
        let _pipeline = NormalPipeline::new();
        // Pipeline 现在是无状态的，只需要能创建即可
    }

    #[test]
    fn test_apply_personalization_with_store_changes_known_pair() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        let store = CorrectionPairStore::new(vec![pair]);

        let (text, changed) = NormalPipeline::apply_personalization_with_store(
            "我打开 克劳德 code".to_string(),
            store,
        );

        assert!(changed);
        assert_eq!(text, "我打开 Claude Code");
    }

    #[test]
    fn test_apply_personalization_with_store_keeps_unrelated_text() {
        let mut pair = CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let store = CorrectionPairStore::new(vec![pair]);

        let (text, changed) = NormalPipeline::apply_personalization_with_store(
            "I use cloud storage".to_string(),
            store,
        );

        assert!(!changed);
        assert_eq!(text, "I use cloud storage");
    }

    #[test]
    fn test_personalization_diagnostic_date_dir_uses_utc_day() {
        assert_eq!(NormalPipeline::utc_date_dir_from_unix_secs(0), "1970-01-01");
        assert_eq!(
            NormalPipeline::utc_date_dir_from_unix_secs(1_704_067_200),
            "2024-01-01"
        );
    }

    #[test]
    fn test_write_personalization_diagnostic_bounds_payload() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let pairs = (0..25)
            .map(|idx| {
                let mut pair = CorrectionPair::new(
                    format!("claude-{idx}"),
                    "cloud code",
                    format!("Claude Code {idx}"),
                );
                pair.source = "manual".to_string();
                pair.confidence = 0.98;
                pair
            })
            .collect();
        let store = CorrectionPairStore::new(pairs);
        let engine = PersonalizationEngine::new(store);
        let result = engine.convert("cloud code");

        let path = NormalPipeline::write_personalization_diagnostic_to_dir(
            temp.path(),
            &"cloud code ".repeat(200),
            &result,
            123,
            456,
        )
        .expect("write diagnostic");
        let payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read diagnostic"))
                .expect("parse diagnostic");

        assert_eq!(payload["schema_version"], 1);
        assert_eq!(payload["stage"], "personalization");
        assert_eq!(payload["elapsed_us"], 123);
        assert_eq!(payload["timestamp_ms"], 456);
        assert_eq!(
            payload["candidates"].as_array().expect("candidates").len(),
            20
        );
        assert!(payload["source_text"]
            .as_str()
            .expect("source text")
            .ends_with("..."));
        assert!(payload["pass_summaries"]
            .as_array()
            .expect("pass summaries")
            .iter()
            .any(|summary| summary["name"] == "exact_text"));
    }
}
