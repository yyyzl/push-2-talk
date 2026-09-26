// 学习流程协调器
//
// 功能：整合观察流程的入口点
// 流程：Pipeline 触发 → 等待观察期 → 验证 → Diff 分析 → LLM 判断 → 发送建议

use crate::platform::{self, InputTarget};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::config::{AppConfig, LearningConfig};
use crate::learning::diff_analyzer::{analyze_diff, merge_word_level_diffs};
use crate::learning::llm_judge::LlmJudge;
use crate::learning::observations::Observations;
use crate::learning::validator::is_asr_text_present;

// 全局活跃观察任务管理器（存储优雅取消标志）
// 使用 Arc<AtomicBool> 替代 AbortHandle，实现"优雅取消"：
// - 旧任务收到取消信号后，立即结束观察期，但继续执行 diff/LLM 流程
// - 避免直接 abort 导致学习丢失
lazy_static::lazy_static! {
    static ref ACTIVE_OBSERVATIONS: Observations<InputTarget> = Observations::new();
}

/// 扩展上下文的最大字符数（防止 CJK 文本导致上下文膨胀）
const MAX_CONTEXT_CHARS: usize = 256;

/// 扩展上下文时前后各取的词数
const CONTEXT_WORDS_BEFORE: usize = 10;
const CONTEXT_WORDS_AFTER: usize = 10;

/// 学习建议事件 payload
#[derive(Debug, Clone, Serialize)]
pub struct LearningSuggestion {
    pub id: String,
    pub word: String,
    pub original: String,
    pub corrected: String,
    pub context: String,
    pub category: String,
    pub reason: String,
}

/// 启动学习观察流程
///
/// 异步执行，不阻塞主流程
///
/// # Arguments
/// * `app` - Tauri 应用句柄
/// * `asr_text` - ASR 识别的原始文本
/// * `target_hwnd` - 目标窗口句柄
/// * `config` - 学习配置
pub fn start_learning_observation(
    app: AppHandle,
    asr_text: String,
    target_hwnd: InputTarget,
    config: LearningConfig,
) -> JoinHandle<()> {
    // 生成唯一的观察ID
    let observation_id = Uuid::new_v4().to_string();
    let baseline_hash = format!("{:x}", md5::compute(&asr_text));

    tracing::info!(
        "Learning: 启动新观察任务 [id={}, hwnd={}, baseline_hash={}, baseline_len={}]",
        &observation_id[..8],
        target_hwnd,
        &baseline_hash[..8],
        asr_text.len()
    );

    // ========== 早退检查（在 spawn 之前执行，避免竞态） ==========
    // 这些检查如果失败，直接返回空任务，不会写入 ACTIVE_OBSERVATIONS
    if !config.enabled {
        tracing::debug!("Learning [{}]: 功能未启用，跳过", &observation_id[..8]);
        return tauri::async_runtime::JoinHandle::Tokio(tokio::spawn(async {}));
    }

    let baseline = asr_text.trim().to_string();
    if baseline.is_empty() {
        tracing::debug!("Learning [{}]: ASR 文本为空，跳过", &observation_id[..8]);
        return tauri::async_runtime::JoinHandle::Tokio(tokio::spawn(async {}));
    }

    // 注册与替换在同一个锁内完成，且先于 spawn；旧任务仍可完成 diff/LLM。
    let observation = ACTIVE_OBSERVATIONS.begin(target_hwnd);
    let cancel_flag_clone = observation.cancel_flag();

    // 启动新任务
    let handle = tokio::spawn(async move {
        let _observation = observation;

        // 等待观察期（用户修正时间）
        let duration = Duration::from_secs(config.observation_duration_secs.max(1));
        let start_time = Instant::now();
        tracing::info!(
            "Learning [{}]: 开始观察期 {}s",
            &observation_id[..8],
            duration.as_secs()
        );

        // 尝试获取修正后的文本（使用墙钟时间控制，支持优雅取消）
        let corrected = match observe_correction_text(
            &observation_id,
            duration,
            target_hwnd,
            cancel_flag_clone.clone(),
        )
        .await
        {
            Some(text) => text,
            None => {
                tracing::info!(
                    "Learning [{}]: 无法获取修正文本，跳过 (实际耗时: {}ms)",
                    &observation_id[..8],
                    start_time.elapsed().as_millis()
                );
                return;
            }
        };

        let elapsed_ms = start_time.elapsed().as_millis();
        tracing::info!(
            "Learning [{}]: 观察期结束 (实际耗时: {}ms)",
            &observation_id[..8],
            elapsed_ms
        );

        // 验证文本是否匹配（使用较低阈值 0.5 容忍更多修改）
        tracing::debug!(
            "Learning [{}]: 验证文本匹配 - ASR原文: \"{}\"，获取文本: \"{}\"",
            &observation_id[..8],
            truncate_text(&baseline, 30),
            truncate_text(&corrected, 30)
        );
        tracing::debug!(
            "Learning [{}]: 文本长度 - baseline: {}, corrected: {}",
            &observation_id[..8],
            baseline.len(),
            corrected.len()
        );
        if !is_asr_text_present(&corrected, &baseline, 0.5) {
            tracing::info!(
                "Learning [{}]: 文本验证失败（相似度不足），跳过",
                &observation_id[..8]
            );
            return;
        }

        // ========== 预处理：从 corrected 中截取 baseline 附近窗口 ==========
        // 目标：避免输入框里有大量历史内容（或其他非本次插入文本）导致 diff 误判。
        // 即使窗口内仍包含少量前后缀，这些通常表现为“纯插入 diff”（original 为空），后续会被过滤。
        let corrected_for_diff = extract_diff_window(&corrected, &baseline, 120);

        // 分析差异（敏感信息降级到 debug）
        tracing::debug!(
            "Learning [{}]: 准备调用 analyze_diff\n  baseline (len={}): \"{}\"\n  corrected (len={}): \"{}\"\n  baseline_bytes: {:?}\n  corrected_bytes: {:?}",
            &observation_id[..8],
            baseline.len(),
            baseline,
            corrected_for_diff.len(),
            corrected_for_diff,
            baseline.as_bytes(),
            corrected_for_diff.as_bytes()
        );
        let char_diffs = analyze_diff(&baseline, &corrected_for_diff);
        if char_diffs.is_empty() {
            tracing::info!(
                "Learning [{}]: 无有效差异（文本完全相同），跳过",
                &observation_id[..8]
            );
            return;
        }

        tracing::info!(
            "Learning [{}]: 发现 {} 个字符级差异",
            &observation_id[..8],
            char_diffs.len()
        );

        // 应用词级合并，减少 LLM 请求次数
        let diffs = merge_word_level_diffs(char_diffs, &baseline, &corrected_for_diff);
        tracing::info!(
            "Learning [{}]: 合并后剩余 {} 个词级差异",
            &observation_id[..8],
            diffs.len()
        );

        // 加载 LLM 配置
        let app_config = match AppConfig::load() {
            Ok((cfg, _)) => cfg,
            Err(e) => {
                tracing::warn!("Learning [{}]: 加载配置失败: {}", &observation_id[..8], e);
                return;
            }
        };

        // 解析 LLM 配置（使用共享配置或独立配置）
        let resolved = config.resolve_llm(&app_config.llm_config.shared);

        if resolved.api_key.trim().is_empty() {
            tracing::debug!(
                "Learning [{}]: LLM API Key 未配置，跳过",
                &observation_id[..8]
            );
            return;
        }

        let judge = LlmJudge::new(&resolved.endpoint, &resolved.api_key, &resolved.model);

        // 预计算词库集合（规范化比对），避免在 diff 循环内反复线性扫描
        let dictionary_word_set: HashSet<String> = app_config
            .dictionary
            .iter()
            .map(|entry| {
                crate::dictionary_utils::extract_word(entry)
                    .trim()
                    .to_string()
            })
            .filter(|w| !w.is_empty())
            .collect();

        // 逐个判断差异
        for diff in diffs {
            let candidate = diff.corrected_segment.trim();
            if candidate.is_empty() {
                continue;
            }

            // 只学习“改错”：跳过纯插入（original 为空）
            // 这类 diff 往往来自输入框里原有历史内容、外部消息更新，或用户额外新增句子。
            if diff.original_segment.trim().is_empty() {
                tracing::info!(
                    "Learning [{}]: 跳过纯插入差异 - 修正: \"{}\"",
                    &observation_id[..8],
                    diff.corrected_segment
                );
                continue;
            }

            // 过滤单字母修正（避免噪声）
            if is_single_letter_noise(&diff.original_segment, &diff.corrected_segment) {
                tracing::info!(
                    "Learning [{}]: 跳过单字母修正 - 原文: \"{}\", 修正: \"{}\"",
                    &observation_id[..8],
                    diff.original_segment,
                    diff.corrected_segment
                );
                continue;
            }

            tracing::info!(
                "Learning [{}]: 请求 LLM 判断 - 原文: \"{}\" → 修正: \"{}\"",
                &observation_id[..8],
                diff.original_segment,
                diff.corrected_segment
            );

            // 提取扩展上下文（前后各 10 个词，而不是原来的 10 个字符）
            let extended_context = extract_extended_context(
                &corrected_for_diff,
                diff.curr_start,
                diff.curr_end,
                CONTEXT_WORDS_BEFORE,
                CONTEXT_WORDS_AFTER,
            );

            tracing::debug!(
                "Learning [{}]: 扩展上下文（长度: {}）: \"{}\"",
                &observation_id[..8],
                extended_context.len(),
                truncate_text(&extended_context, 100)
            );

            let result = match judge
                .judge(
                    &diff.original_segment,
                    &diff.corrected_segment,
                    &extended_context,
                )
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!("Learning [{}]: LLM 判断失败: {}", &observation_id[..8], e);
                    continue;
                }
            };

            tracing::info!(
                "Learning [{}]: LLM 判断结果 - should_learn: {}, word: \"{}\", category: \"{}\", reason: \"{}\"",
                &observation_id[..8],
                result.should_learn,
                result.word,
                result.category,
                result.reason
            );

            if !result.should_learn {
                tracing::info!(
                    "Learning [{}]: LLM 建议不加入词库: {}",
                    &observation_id[..8],
                    result.reason
                );
                continue;
            }

            let word = if result.word.trim().is_empty() {
                candidate.to_string()
            } else {
                result.word.clone()
            };

            if word.trim().is_empty() {
                continue;
            }

            // 检查词库是否已存在该词（使用预计算的 HashSet 进行 O(1) 查找）
            let normalized_word = crate::dictionary_utils::normalize_word(&word);
            if dictionary_word_set.contains(&normalized_word) {
                tracing::info!(
                    "Learning [{}]: 词汇 \"{}\" 已存在于词库，跳过通知",
                    &observation_id[..8],
                    normalized_word
                );
                continue;
            }

            // 创建建议（使用规范化后的词汇，确保与词库比对一致）
            let suggestion_id = uuid::Uuid::new_v4().to_string();
            let suggestion = LearningSuggestion {
                id: suggestion_id,
                word: normalized_word.clone(),
                original: diff.original_segment.clone(),
                corrected: diff.corrected_segment.clone(),
                context: diff.context.clone(),
                category: result.category,
                reason: result.reason,
            };

            tracing::info!(
                "Learning [{}]: 发送学习建议到前端 - 词汇: \"{}\", 分类: \"{}\", 原因: \"{}\"",
                &observation_id[..8],
                normalized_word,
                suggestion.category,
                suggestion.reason
            );
            match app.emit("vocabulary_learning_suggestion", suggestion.clone()) {
                Ok(_) => tracing::info!("Learning [{}]: 事件发送成功", &observation_id[..8]),
                Err(e) => {
                    tracing::error!("Learning [{}]: 事件发送失败: {:?}", &observation_id[..8], e)
                }
            }
        }

        tracing::info!(
            "Learning [{}]: 学习流程结束 (总耗时: {}ms)",
            &observation_id[..8],
            start_time.elapsed().as_millis()
        );
    });

    // 包装为 Tauri JoinHandle
    tauri::async_runtime::JoinHandle::Tokio(handle)
}

#[cfg(test)]
mod acceptance_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct ScriptedReader {
        focus_checks: AtomicUsize,
        reads: AtomicUsize,
        focused_polls: usize,
        sample: Option<String>,
    }
    impl CorrectionReader for ScriptedReader {
        fn is_focused(&self) -> bool {
            self.focus_checks.fetch_add(1, Ordering::SeqCst) < self.focused_polls
        }
        fn read(&self) -> Option<String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.sample.clone()
        }
    }
    fn reader(focused_polls: usize, sample: Option<&str>) -> Arc<ScriptedReader> {
        Arc::new(ScriptedReader {
            focus_checks: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            focused_polls,
            sample: sample.map(str::to_owned),
        })
    }

    #[tokio::test]
    async fn superseded_before_first_poll_does_not_read_the_new_insertion() {
        let reader = reader(
            usize::MAX,
            Some("new insertion must not become old correction"),
        );
        let result = observe_with_reader(
            "cancel-before",
            Duration::from_secs(5),
            reader.clone(),
            Arc::new(AtomicBool::new(true)),
        )
        .await;
        assert_eq!(result, None);
        assert_eq!(reader.reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn superseded_during_poll_delay_does_not_read_the_new_insertion() {
        let reader = reader(
            usize::MAX,
            Some("new insertion must not become old correction"),
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(observe_with_reader(
            "cancel-wait",
            Duration::from_secs(5),
            reader.clone(),
            cancel.clone(),
        ));
        sleep(Duration::from_millis(50)).await;
        cancel.store(true, Ordering::SeqCst);
        assert_eq!(task.await.unwrap(), None);
        assert_eq!(reader.reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unfocused_target_stops_after_three_polls_without_reading() {
        let reader = reader(0, Some("bystander must never be read"));
        assert_eq!(
            observe_with_reader(
                "lost-focus",
                Duration::from_secs(10),
                reader.clone(),
                Arc::new(AtomicBool::new(false))
            )
            .await,
            None
        );
        assert_eq!(reader.focus_checks.load(Ordering::SeqCst), 3);
        assert_eq!(reader.reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn focus_loss_keeps_only_the_last_focused_sample() {
        let reader = reader(1, Some("known correction before switching apps"));
        let result = observe_with_reader(
            "focus-after",
            Duration::from_secs(10),
            reader.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
        assert_eq!(
            result.as_deref(),
            Some("known correction before switching apps")
        );
        assert_eq!(reader.reads.load(Ordering::SeqCst), 1);
        assert_eq!(reader.focus_checks.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn unsupported_control_produces_no_sample() {
        let reader = reader(usize::MAX, None);
        assert_eq!(
            observe_with_reader(
                "unsupported",
                Duration::from_millis(500),
                reader.clone(),
                Arc::new(AtomicBool::new(false))
            )
            .await,
            None
        );
        assert_eq!(reader.reads.load(Ordering::SeqCst), 1);
    }

    struct ReplacedWhileReading {
        cancel: Arc<AtomicBool>,
        reads: AtomicUsize,
        cancel_on_read: usize,
    }
    impl CorrectionReader for ReplacedWhileReading {
        fn is_focused(&self) -> bool {
            true
        }
        fn read(&self) -> Option<String> {
            let index = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
            if index == self.cancel_on_read {
                self.cancel.store(true, Ordering::SeqCst);
                Some("replacement insertion".into())
            } else {
                Some("previously observed correction".into())
            }
        }
    }

    #[tokio::test]
    async fn replacement_during_native_read_discards_the_in_flight_sample() {
        for cancel_on_read in [1, 2] {
            let cancel = Arc::new(AtomicBool::new(false));
            let reader = Arc::new(ReplacedWhileReading {
                cancel: cancel.clone(),
                reads: AtomicUsize::new(0),
                cancel_on_read,
            });
            let result =
                observe_with_reader("cancel-read", Duration::from_secs(5), reader, cancel).await;
            assert_eq!(
                result.as_deref(),
                if cancel_on_read == 1 {
                    None
                } else {
                    Some("previously observed correction")
                }
            );
        }
    }

    #[test]
    fn real_textedit_correction_survives_validation_and_diff_filters() {
        let baseline = "使用库伯内特斯管理容器服务运行正常。这次测试使用库伯内德斯管理容器，客户运行正常。这个测试使用库博内特斯管理的容器服务运营正常，这次的测试使用库柏内特斯管理。";
        let corrected = format!(
            "PushToTalk ATDD\n\n{}",
            baseline.replacen("库伯内特斯", "Kubernetes", 1)
        );
        assert!(is_asr_text_present(&corrected, baseline, 0.5));
        let window = extract_diff_window(&corrected, baseline, 120);
        let diffs = merge_word_level_diffs(analyze_diff(baseline, &window), baseline, &window);
        assert!(
            diffs.iter().any(|diff| {
                !diff.original_segment.trim().is_empty()
                    && diff.corrected_segment.contains("Kubernetes")
                    && !is_single_letter_noise(&diff.original_segment, &diff.corrected_segment)
            }),
            "a real correction must reach the learning judge"
        );
    }
}

/// 观察修正文本
///
/// 每500ms检测一次文本变化，使用墙钟时间控制观察期时长，返回最后一次成功获取的文本
///
/// # 焦点检查
/// 每次读取前检查目标窗口是否仍在前台，如果用户已切换窗口则跳过读取
///
/// # 优雅取消
/// 当 cancel_flag 被设置为 true 时，立即结束观察期，但返回已获取的文本（如有）
/// 这样旧任务可以继续执行 diff/LLM 流程，不会丢失学习机会
///
/// # 参数
/// * `observation_id` - 观察任务ID（用于日志关联）
/// * `duration` - 观察期时长
/// * `target_hwnd` - 目标窗口句柄
/// * `cancel_flag` - 优雅取消标志
///
/// # 返回值
/// * `Some(String)` - 成功获取修正后的文本
/// * `None` - 获取失败（窗口无效、UIA 不支持等）
async fn observe_correction_text(
    observation_id: &str,
    duration: Duration,
    target_hwnd: InputTarget,
    cancel_flag: Arc<AtomicBool>,
) -> Option<String> {
    observe_with_reader(
        observation_id,
        duration,
        Arc::new(NativeCorrectionReader(target_hwnd)),
        cancel_flag,
    )
    .await
}

trait CorrectionReader: Send + Sync {
    fn is_focused(&self) -> bool;
    fn read(&self) -> Option<String>;
}

struct NativeCorrectionReader(InputTarget);
impl CorrectionReader for NativeCorrectionReader {
    fn is_focused(&self) -> bool {
        platform::desktop().is_focused(self.0)
    }
    fn read(&self) -> Option<String> {
        read_observed_text(self.0)
    }
}

async fn observe_with_reader(
    observation_id: &str,
    duration: Duration,
    reader: Arc<dyn CorrectionReader>,
    cancel_flag: Arc<AtomicBool>,
) -> Option<String> {
    // 降低轮询频率：100ms → 500ms，减少线程风暴
    let check_interval = Duration::from_millis(500);
    let deadline = Instant::now() + duration;

    tracing::info!(
        "Learning [{}]: 开始监控文本变化，每{}ms检测一次，墙钟时间限制{}s",
        &observation_id[..8],
        check_interval.as_millis(),
        duration.as_secs()
    );

    let mut last_text: Option<String> = None;
    let mut focus_lost_count = 0;
    let mut check_count = 0;
    let mut ended_due_to_focus_loss = false;
    let mut ended_due_to_cancel = false;
    const MAX_FOCUS_LOST_COUNT: usize = 3; // 连续 3 次失焦后提前结束

    // 使用墙钟时间控制循环
    while Instant::now() < deadline {
        // 检查优雅取消标志
        if cancel_flag.load(Ordering::SeqCst) {
            tracing::info!(
                "Learning [{}]: 收到优雅取消信号，提前结束观察期（将继续执行学习流程）",
                &observation_id[..8]
            );
            ended_due_to_cancel = true;
            break;
        }

        sleep(check_interval).await;
        // A new insertion can supersede this task while it sleeps. Do not read
        // that insertion as a correction of the old baseline.
        if cancel_flag.load(Ordering::SeqCst) {
            ended_due_to_cancel = true;
            break;
        }
        check_count += 1;

        // 焦点检查：如果目标窗口已失去焦点，跳过本次读取
        if !reader.is_focused() {
            focus_lost_count += 1;
            tracing::debug!(
                "Learning [{}]: 第{}次检测跳过（目标窗口已失焦，连续{}次）",
                &observation_id[..8],
                check_count,
                focus_lost_count
            );

            // 连续多次失焦，提前结束观察期
            if focus_lost_count >= MAX_FOCUS_LOST_COUNT {
                tracing::info!(
                    "Learning [{}]: 连续{}次失焦，提前结束观察期",
                    &observation_id[..8],
                    focus_lost_count
                );
                ended_due_to_focus_loss = true;
                break;
            }
            continue;
        }

        // 焦点在目标窗口，重置计数
        focus_lost_count = 0;

        // 在同步上下文中调用 UIA 读取（带超时保护）
        let uia_start = Instant::now();
        let sample_reader = reader.clone();
        let text = tokio::task::spawn_blocking(move || sample_reader.read())
            .await
            .ok()
            .flatten();
        let uia_elapsed = uia_start.elapsed();
        if cancel_flag.load(Ordering::SeqCst) {
            ended_due_to_cancel = true;
            break;
        }

        // 记录 UIA 读取耗时（用于诊断）
        if uia_elapsed.as_millis() > 200 {
            tracing::debug!(
                "Learning [{}]: UIA 读取耗时较长: {}ms",
                &observation_id[..8],
                uia_elapsed.as_millis()
            );
        }

        if let Some(content) = text {
            if !content.trim().is_empty() {
                tracing::debug!(
                    "Learning [{}]: 第{}次检测成功，文本长度: {}，内容: \"{}\"",
                    &observation_id[..8],
                    check_count,
                    content.len(),
                    truncate_text(&content, 50)
                );
                last_text = Some(content);
            }
        }
    }

    // Debug 级别输出实际文本内容
    if ended_due_to_cancel {
        // 优雅取消：旧任务被新任务取代，但仍应继续学习流程
        tracing::info!(
            "Learning [{}]: 因优雅取消提前结束（检测次数: {}）",
            &observation_id[..8],
            check_count
        );
        // Only an already observed sample belongs to the old baseline. A final
        // fresh read here may contain the replacement recording's inserted text.
    } else if ended_due_to_focus_loss {
        // 数据可靠性较差：窗口失焦意味着后续读取可能不可靠。
        // 但如果在失焦前已成功读取到文本，仍可返回 last_text，避免学习功能过于脆弱。
        tracing::info!(
            "Learning [{}]: 因失焦提前结束（检测次数: {}）",
            &observation_id[..8],
            check_count
        );
        if last_text.is_none() {
            tracing::info!(
                "Learning [{}]: 因失焦提前结束且未曾成功读取文本，放弃本次学习",
                &observation_id[..8]
            );
            return None;
        }
    }

    match &last_text {
        Some(text) => {
            tracing::debug!(
                "Learning [{}]: 观察期结束，最终文本（长度: {}）: \"{}\"",
                &observation_id[..8],
                text.len(),
                truncate_text(text, 100)
            );
            tracing::info!(
                "Learning [{}]: 观察期结束，已获取文本（长度: {}，检测次数: {}）",
                &observation_id[..8],
                text.len(),
                check_count
            );
        }
        None => {
            tracing::info!(
                "Learning [{}]: 观察期结束，未获取到文本（检测次数: {}）",
                &observation_id[..8],
                check_count
            );
        }
    }

    last_text
}

/// 从 corrected 中截取 baseline 附近的一段窗口，用于 diff。
///
/// - 如果能精确找到 baseline 子串：以其为中心截取前后 `context_chars` 个字符
/// - 找不到时：退化为截取 corrected 的末尾窗口（常见输入场景：插入发生在光标附近/末尾）
fn extract_diff_window(corrected: &str, baseline: &str, context_chars: usize) -> String {
    let corrected_trimmed = corrected.trim();
    if corrected_trimmed.is_empty() {
        return String::new();
    }

    let baseline_trimmed = baseline.trim();
    let corrected_chars: Vec<char> = corrected_trimmed.chars().collect();
    let baseline_char_len = baseline_trimmed.chars().count();

    // 优先：精确定位 baseline（注意 find 返回 byte index）
    if !baseline_trimmed.is_empty() {
        if let Some(byte_idx) = corrected_trimmed.find(baseline_trimmed) {
            let start_char = corrected_trimmed[..byte_idx].chars().count();
            let end_char = (start_char + baseline_char_len).min(corrected_chars.len());
            let win_start = start_char.saturating_sub(context_chars);
            let win_end = (end_char + context_chars).min(corrected_chars.len());
            return corrected_chars[win_start..win_end].iter().collect();
        }
    }

    // 退化：截取末尾窗口，尽量避免把整个输入框历史内容纳入 diff
    let win_len = (baseline_char_len + context_chars.saturating_mul(2)).max(160);
    if corrected_chars.len() <= win_len {
        corrected_trimmed.to_string()
    } else {
        corrected_chars[corrected_chars.len() - win_len..]
            .iter()
            .collect()
    }
}

/// 判断是否为单字母噪声修正
///
/// 过滤掉单个 ASCII 字母的修正（如 "o"→"a"、""→"e"），这些通常是字符级 diff 的副产品
/// 注意：只过滤 ASCII 英文字母，不过滤中文字符（中文单字修正可能是人名等有意义的修正）
///
/// # 参数
/// * `original` - 原文片段
/// * `corrected` - 修正片段
///
/// # 返回值
/// * `true` - 是单字母噪声，应该过滤
/// * `false` - 不是噪声，应该保留
fn is_single_letter_noise(original: &str, corrected: &str) -> bool {
    let orig_trimmed = original.trim();
    let corr_trimmed = corrected.trim();

    // 使用 chars().count() 获取字符数量（非字节长度）
    // 这对于多字节字符（如中文）很重要：中文字符 len() 返回 3，chars().count() 返回 1
    let corr_char_count = corr_trimmed.chars().count();
    let orig_char_count = orig_trimmed.chars().count();

    // 如果修正后是单个 ASCII 字母（且原文也是单个 ASCII 字母或为空），则视为噪声
    // 使用 is_ascii_alphabetic() 而非 is_alphabetic()，避免错误过滤中文单字修正
    if corr_char_count == 1 && corr_trimmed.chars().next().unwrap().is_ascii_alphabetic() {
        if orig_trimmed.is_empty()
            || (orig_char_count == 1 && orig_trimmed.chars().next().unwrap().is_ascii_alphabetic())
        {
            return true;
        }
    }

    false
}

/// 通过平台文本观察接口读取目标文本
///
/// Windows 使用 UIA；macOS 使用 AX。读取不会抢占焦点。
///
/// # 参数
/// * `target_hwnd` - 目标窗口句柄
///
/// # 返回值
/// * `Some(String)` - 成功读取的文本
/// * `None` - 读取失败（窗口无效、UIA 不支持等）
fn read_observed_text(target_hwnd: InputTarget) -> Option<String> {
    // 检查窗口是否有效
    if !platform::desktop().is_valid(target_hwnd) {
        tracing::debug!("Learning: 目标窗口已无效");
        return None;
    }

    // 使用平台文本观察接口读取（无干扰方案）
    match platform::desktop().read_text(target_hwnd) {
        Ok(text) if !text.trim().is_empty() => {
            tracing::debug!("Learning: UIA 成功读取文本（长度: {}）", text.len());
            Some(text)
        }
        Ok(_) => {
            tracing::debug!("Learning: UIA 返回空文本");
            None
        }
        Err(e) => {
            tracing::debug!("Learning: UIA 读取失败: {}", e);
            None
        }
    }
}

/// 截断文本用于日志显示
///
/// # 参数
/// * `text` - 原始文本
/// * `max_len` - 最大字符数
///
/// # 返回值
/// 截断后的文本，超出部分用 "..." 替代
fn truncate_text(text: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        text.to_string()
    } else {
        let truncated: String = chars.iter().take(max_len).collect();
        format!("{}...", truncated)
    }
}

/// 从修正文本中提取修改点前后各 N 个词的上下文
///
/// 用于 LLM 判断短语联动（例如："claude" + "code" → "claude code"）
///
/// # 参数
/// * `corrected` - 修正后的完整文本
/// * `diff_start` - diff 在 corrected 中的起始字符位置
/// * `diff_end` - diff 在 corrected 中的结束字符位置
/// * `words_before` - 前面取多少个词（默认 10）
/// * `words_after` - 后面取多少个词（默认 10）
///
/// # 返回值
/// 扩展后的上下文字符串
fn extract_extended_context(
    corrected: &str,
    diff_start: usize,
    diff_end: usize,
    words_before: usize,
    words_after: usize,
) -> String {
    let chars: Vec<char> = corrected.chars().collect();
    let total_len = chars.len();

    // 边界检查 - 异常情况下使用保守的短上下文
    if diff_start >= total_len || diff_end > total_len || diff_start >= diff_end {
        tracing::warn!(
            "Learning: diff 索引异常 (start={}, end={}, len={}), 退化到短上下文",
            diff_start,
            diff_end,
            total_len
        );
        // 退化到更保守的短上下文（前后各 50 字符）
        let safe_start = diff_start.min(total_len).saturating_sub(50);
        let safe_end = diff_end.min(total_len).saturating_add(50).min(total_len);
        return chars[safe_start..safe_end].iter().collect();
    }

    // 向前扫描，找到 words_before 个词的边界
    let mut start_pos = diff_start;
    let mut word_count = 0;
    let mut in_word = false;

    for i in (0..diff_start).rev() {
        let ch = chars[i];
        let is_word_char = crate::learning::is_word_char(ch);

        if is_word_char {
            if !in_word {
                word_count += 1;
                if word_count > words_before {
                    start_pos = i + 1;
                    break;
                }
                in_word = true;
            }
        } else {
            in_word = false;
        }

        if i == 0 {
            start_pos = 0;
        }
    }

    // 向后扫描，找到 words_after 个词的边界
    let mut end_pos = diff_end;
    word_count = 0;
    in_word = false;

    for i in diff_end..total_len {
        let ch = chars[i];
        let is_word_char = crate::learning::is_word_char(ch);

        if is_word_char {
            if !in_word {
                word_count += 1;
                if word_count > words_after {
                    end_pos = i;
                    break;
                }
                in_word = true;
            }
        } else {
            in_word = false;
        }

        if i == total_len - 1 {
            end_pos = total_len;
        }
    }

    // 截取范围
    let result: String = chars[start_pos..end_pos].iter().collect();

    // 硬上限保护（防止 CJK 无空格文本导致上下文膨胀）
    if result.chars().count() > MAX_CONTEXT_CHARS {
        tracing::warn!(
            "Learning: 上下文过长 ({} 字符), 截断到 {}",
            result.chars().count(),
            MAX_CONTEXT_CHARS
        );
        result.chars().take(MAX_CONTEXT_CHARS).collect()
    } else {
        result
    }
}
