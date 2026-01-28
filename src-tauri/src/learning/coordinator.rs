// 学习流程协调器
//
// 功能：整合观察流程的入口点
// 流程：Pipeline 触发 → 等待观察期 → 验证 → Diff 分析 → LLM 判断 → 发送建议

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::config::{AppConfig, LearningConfig};
use crate::learning::diff_analyzer::{analyze_diff, merge_word_level_diffs};
use crate::learning::llm_judge::LlmJudge;
use crate::learning::validator::is_asr_text_present;

// 全局活跃观察任务管理器（存储优雅取消标志）
// 使用 Arc<AtomicBool> 替代 AbortHandle，实现"优雅取消"：
// - 旧任务收到取消信号后，立即结束观察期，但继续执行 diff/LLM 流程
// - 避免直接 abort 导致学习丢失
lazy_static::lazy_static! {
    static ref ACTIVE_OBSERVATIONS: Arc<Mutex<HashMap<isize, Arc<AtomicBool>>>> = Arc::new(Mutex::new(HashMap::new()));
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
    target_hwnd: isize,
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

    // 取消同一窗口的旧观察任务（优雅取消：发送信号让旧任务提前结束观察期）
    // 旧任务会继续执行 diff/LLM 流程，不会丢失学习机会
    {
        let mut active = ACTIVE_OBSERVATIONS.lock().unwrap();
        if let Some(old_cancel_flag) = active.remove(&target_hwnd) {
            tracing::info!(
                "Learning: 优雅取消旧观察任务 [hwnd={}]（旧任务将继续完成学习流程）",
                target_hwnd
            );
            old_cancel_flag.store(true, Ordering::SeqCst);
        }
    }

    // 创建新任务的取消标志
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = cancel_flag.clone();

    // 启动新任务
    let handle = tokio::spawn(async move {
        // RAII 清理守卫：确保任务结束时从 ACTIVE_OBSERVATIONS 中移除
        struct CleanupGuard {
            hwnd: isize,
        }
        impl Drop for CleanupGuard {
            fn drop(&mut self) {
                let mut active = ACTIVE_OBSERVATIONS.lock().unwrap();
                if active.remove(&self.hwnd).is_some() {
                    tracing::debug!("Learning: 任务完成，已从活跃观察中移除 hwnd={}", self.hwnd);
                }
            }
        }
        let _cleanup = CleanupGuard { hwnd: target_hwnd };

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
        ).await {
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
                tracing::warn!(
                    "Learning [{}]: 加载配置失败: {}",
                    &observation_id[..8],
                    e
                );
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

        let judge = LlmJudge::new(
            &resolved.endpoint,
            &resolved.api_key,
            &resolved.model,
        );

        // 预计算词库集合（规范化比对），避免在 diff 循环内反复线性扫描
        let dictionary_word_set: HashSet<String> = app_config
            .dictionary
            .iter()
            .map(|entry| crate::learning::store::extract_word(entry).trim().to_string())
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
                .judge(&diff.original_segment, &diff.corrected_segment, &extended_context)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!(
                        "Learning [{}]: LLM 判断失败: {}",
                        &observation_id[..8],
                        e
                    );
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
            let normalized_word = crate::learning::store::normalize_word(&word);
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
                Ok(_) => tracing::info!(
                    "Learning [{}]: 事件发送成功",
                    &observation_id[..8]
                ),
                Err(e) => tracing::error!(
                    "Learning [{}]: 事件发送失败: {:?}",
                    &observation_id[..8],
                    e
                ),
            }
        }

        tracing::info!(
            "Learning [{}]: 学习流程结束 (总耗时: {}ms)",
            &observation_id[..8],
            start_time.elapsed().as_millis()
        );
    });

    // 保存新任务的取消标志
    {
        let mut active = ACTIVE_OBSERVATIONS.lock().unwrap();
        active.insert(target_hwnd, cancel_flag);
    }

    // 包装为 Tauri JoinHandle
    tauri::async_runtime::JoinHandle::Tokio(handle)
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
    target_hwnd: isize,
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
        check_count += 1;

        // 焦点检查：如果目标窗口已失去焦点，跳过本次读取
        let current_fg = crate::win32_input::get_foreground_window();
        if current_fg != Some(target_hwnd) {
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
        let text = tokio::task::spawn_blocking(move || get_text_via_uia(target_hwnd))
            .await
            .ok()
            .flatten();
        let uia_elapsed = uia_start.elapsed();

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
        // 即使没有读取到文本，也尝试立即读取一次
        if last_text.is_none() {
            tracing::info!(
                "Learning [{}]: 优雅取消时尚未读取到文本，尝试立即读取",
                &observation_id[..8]
            );
            let text = tokio::task::spawn_blocking(move || get_text_via_uia(target_hwnd))
                .await
                .ok()
                .flatten();
            if let Some(content) = text {
                if !content.trim().is_empty() {
                    tracing::info!(
                        "Learning [{}]: 优雅取消时立即读取成功（长度: {}）",
                        &observation_id[..8],
                        content.len()
                    );
                    last_text = Some(content);
                }
            }
        }
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
        if orig_trimmed.is_empty() || (orig_char_count == 1 && orig_trimmed.chars().next().unwrap().is_ascii_alphabetic()) {
            return true;
        }
    }

    false
}

/// 通过 UI Automation 获取目标窗口文本
///
/// 仅使用 UIA 方案，不会抢占焦点
///
/// # 参数
/// * `target_hwnd` - 目标窗口句柄
///
/// # 返回值
/// * `Some(String)` - 成功读取的文本
/// * `None` - 读取失败（窗口无效、UIA 不支持等）
fn get_text_via_uia(target_hwnd: isize) -> Option<String> {
    // 检查窗口是否有效
    if !crate::win32_input::is_window_valid(target_hwnd) {
        tracing::debug!("Learning: 目标窗口已无效");
        return None;
    }

    // 使用 UI Automation 读取文本（无干扰方案）
    match crate::uia_text_reader::get_focused_window_text(target_hwnd) {
        Ok(text) if !text.trim().is_empty() => {
            tracing::debug!(
                "Learning: UIA 成功读取文本（长度: {}）",
                text.len()
            );
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
            diff_start, diff_end, total_len
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
