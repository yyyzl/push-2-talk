// Windows UI Automation 文本读取模块
// 用于无干扰地读取焦点窗口文本，替代剪贴板方案

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::core::{Interface, HRESULT};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationValuePattern, UIA_TextPatternId, UIA_ValuePatternId,
};

/// UIA 调用超时时间（2 秒）
const UIA_TIMEOUT: Duration = Duration::from_secs(2);

/// 最大并发 UIA 工作线程数（防止线程堆积）
const MAX_CONCURRENT_UIA_WORKERS: usize = 2;

/// 当前活跃的 UIA 工作线程计数
static ACTIVE_UIA_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// 黑名单触发阈值（连续失败次数）
const BLACKLIST_FAILURE_THRESHOLD: u32 = 3;

/// 黑名单持续时间（30 秒）
const BLACKLIST_DURATION: Duration = Duration::from_secs(30);

/// 失败计数窗口期（60 秒内的失败才累计）
const FAILURE_WINDOW: Duration = Duration::from_secs(60);

/// COM 初始化守卫（RAII 模式）
///
/// 确保 COM 在使用后正确清理
#[derive(Debug)]
struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    /// 初始化 COM（多线程模式）
    ///
    /// # 错误处理
    /// - 如果 COM 已初始化（RPC_E_CHANGED_MODE），不会报错，但不会调用 CoUninitialize
    /// - 其他错误会返回 Err
    fn new() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED | COINIT_DISABLE_OLE1DDE) };

        // RPC_E_CHANGED_MODE = 0x80010106
        // COM 已在此线程初始化（不同的 apartment 模式）
        if hr == HRESULT(0x80010106u32 as i32) {
            tracing::debug!("uia_text_reader: COM 已初始化（RPC_E_CHANGED_MODE），跳过清理");
            return Ok(Self {
                should_uninit: false,
            });
        }

        hr.ok().context("CoInitializeEx 失败")?;
        Ok(Self {
            should_uninit: true,
        })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe { CoUninitialize() };
        }
    }
}

/// 黑名单状态（全局单例）
#[derive(Default)]
struct BlacklistState {
    by_hwnd: HashMap<isize, FailureEntry>,
}

/// 失败记录条目
#[derive(Debug, Clone)]
struct FailureEntry {
    first_failure_at: Instant,
    failure_count: u32,
    blacklisted_until: Option<Instant>,
}

/// 获取全局黑名单状态
fn blacklist_state() -> &'static Mutex<BlacklistState> {
    static STATE: OnceLock<Mutex<BlacklistState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BlacklistState::default()))
}

/// 检查窗口是否在黑名单中
fn is_blacklisted(hwnd: isize, now: Instant) -> bool {
    let mut state = blacklist_state().lock().unwrap();

    // 清理过期条目
    state.by_hwnd.retain(|_, entry| match entry.blacklisted_until {
        Some(until) => until > now,
        None => now.duration_since(entry.first_failure_at) <= FAILURE_WINDOW,
    });

    match state.by_hwnd.get(&hwnd).and_then(|e| e.blacklisted_until) {
        Some(until) if until > now => true,
        _ => false,
    }
}

/// 记录成功（清除黑名单）
fn record_success(hwnd: isize) {
    let mut state = blacklist_state().lock().unwrap();
    state.by_hwnd.remove(&hwnd);
}

/// 记录失败（可能触发黑名单）
///
/// # 返回值
/// * `true` - 已触发黑名单
/// * `false` - 未触发黑名单
fn record_failure(hwnd: isize, now: Instant) -> bool {
    let mut state = blacklist_state().lock().unwrap();

    let entry = state.by_hwnd.entry(hwnd).or_insert_with(|| FailureEntry {
        first_failure_at: now,
        failure_count: 0,
        blacklisted_until: None,
    });

    // 重置滚动窗口
    if now.duration_since(entry.first_failure_at) > FAILURE_WINDOW {
        entry.first_failure_at = now;
        entry.failure_count = 0;
        entry.blacklisted_until = None;
    }

    entry.failure_count = entry.failure_count.saturating_add(1);

    if entry.failure_count >= BLACKLIST_FAILURE_THRESHOLD {
        let until = now + BLACKLIST_DURATION;
        entry.blacklisted_until = Some(until);
        true
    } else {
        false
    }
}

/// 超时但仍在运行的 UIA 线程计数（用于监控）
static TIMED_OUT_UIA_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// 带超时的函数执行
///
/// 在独立线程中执行函数，如果超时则返回错误
///
/// # 并发控制
/// 限制最大并发工作线程数，防止 UIA 卡死时线程无限堆积
///
/// # 超时处理（修复版）
/// 超时后**不释放**配额，由工作线程完成后自己释放。
/// 这样可以防止新请求进入，避免线程无限堆积。
/// 超时的线程会被计入 TIMED_OUT_UIA_WORKERS 用于监控。
fn run_with_timeout<T, F>(timeout: Duration, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    use std::sync::mpsc;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    // 防止无限创建线程（UIA 卡死时）
    let mut cur = ACTIVE_UIA_WORKERS.load(Ordering::Relaxed);
    loop {
        if cur >= MAX_CONCURRENT_UIA_WORKERS {
            let timed_out_count = TIMED_OUT_UIA_WORKERS.load(Ordering::Relaxed);
            return Err(anyhow!(
                "UIA 工作线程池饱和（活跃: {}, 最大: {}, 超时未完成: {}）",
                cur,
                MAX_CONCURRENT_UIA_WORKERS,
                timed_out_count
            ));
        }
        match ACTIVE_UIA_WORKERS.compare_exchange_weak(
            cur,
            cur + 1,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(v) => cur = v,
        }
    }

    let (tx, rx) = mpsc::channel::<Result<T>>();

    // 使用 Arc<AtomicBool> 标记是否已超时
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_clone = timed_out.clone();

    thread::spawn(move || {
        // RAII 守卫：确保线程退出时减少计数
        struct WorkerGuard {
            timed_out: Arc<AtomicBool>,
        }
        impl Drop for WorkerGuard {
            fn drop(&mut self) {
                // 无论是否超时，工作线程完成时都要减少活跃计数
                ACTIVE_UIA_WORKERS.fetch_sub(1, Ordering::SeqCst);
                // 如果是超时的线程完成了，减少超时计数
                if self.timed_out.load(Ordering::SeqCst) {
                    TIMED_OUT_UIA_WORKERS.fetch_sub(1, Ordering::SeqCst);
                    tracing::debug!(
                        "UIA 超时线程已完成，当前活跃: {}, 超时未完成: {}",
                        ACTIVE_UIA_WORKERS.load(Ordering::Relaxed),
                        TIMED_OUT_UIA_WORKERS.load(Ordering::Relaxed)
                    );
                }
            }
        }
        let _guard = WorkerGuard { timed_out: timed_out_clone };
        let _ = tx.send(f());
    });

    match rx.recv_timeout(timeout) {
        Ok(res) => res,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // 超时：标记超时状态，但**不释放**配额
            // 配额由工作线程完成后自己释放，防止新请求进入导致线程堆积
            timed_out.store(true, Ordering::SeqCst);
            TIMED_OUT_UIA_WORKERS.fetch_add(1, Ordering::SeqCst);
            tracing::warn!(
                "UIA 调用超时（{:?}），配额保留直到线程完成，当前活跃: {}, 超时未完成: {}",
                timeout,
                ACTIVE_UIA_WORKERS.load(Ordering::Relaxed),
                TIMED_OUT_UIA_WORKERS.load(Ordering::Relaxed)
            );
            Err(anyhow!("UIA 调用超时（{:?}）", timeout))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow!("UIA 工作线程断开连接")),
    }
}

/// 使用 UI Automation 读取焦点窗口文本（公共接口）
///
/// # 参数
/// * `hwnd` - 窗口句柄
///
/// # 返回值
/// * `Ok(String)` - 成功读取的文本
/// * `Err(e)` - 读取失败（超时、黑名单、UIA 不支持等）
///
/// # 黑名单机制
/// - 连续失败 3 次后，该窗口会被黑名单 30 秒
/// - 黑名单期间直接返回错误，不尝试 UIA
/// - 成功读取后自动清除黑名单
pub fn get_focused_window_text(hwnd: isize) -> Result<String> {
    // 校验窗口句柄有效性
    if hwnd == 0 || !crate::win32_input::is_window_valid(hwnd) {
        return Err(anyhow!("无效的窗口句柄（hwnd={}）", hwnd));
    }

    let now = Instant::now();
    if is_blacklisted(hwnd, now) {
        return Err(anyhow!("UIA 暂时黑名单（hwnd={}）", hwnd));
    }

    let res = run_with_timeout(UIA_TIMEOUT, move || get_focused_window_text_inner(hwnd));

    match &res {
        Ok(_) => record_success(hwnd),
        Err(e) => {
            let now = Instant::now();
            let blacklisted = record_failure(hwnd, now);
            if blacklisted {
                tracing::warn!(
                    "uia_text_reader: UIA 连续失败，黑名单 hwnd={} 持续 {:?}（最后错误: {}）",
                    hwnd,
                    BLACKLIST_DURATION,
                    e
                );
            } else {
                tracing::debug!("uia_text_reader: UIA 失败 hwnd={}（错误: {}）", hwnd, e);
            }
        }
    }

    res
}

/// UI Automation 文本读取核心实现
fn get_focused_window_text_inner(hwnd: isize) -> Result<String> {
    let _com = ComGuard::new()?;

    // 创建 UI Automation 实例
    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .context("创建 CUIAutomation 实例失败")?
    };

    let hwnd = HWND(hwnd as *mut _);

    // 尝试从窗口句柄获取根元素
    let root = unsafe { automation.ElementFromHandle(hwnd) }.ok();

    // 尝试获取全局焦点元素
    let mut focused = unsafe { automation.GetFocusedElement() }.ok();

    // 如果同时有根元素和焦点元素，校验焦点元素是否属于同一进程
    // 避免读取到其他窗口的文本
    if let (Some(root_el), Some(focused_el)) = (&root, &focused) {
        let root_pid = unsafe { root_el.CurrentProcessId() }.unwrap_or(0);
        if root_pid != 0 {
            let focused_pid = unsafe { focused_el.CurrentProcessId() }.unwrap_or(0);
            if focused_pid != 0 && focused_pid != root_pid {
                tracing::debug!(
                    "uia_text_reader: 焦点元素进程不匹配（focused_pid={}, root_pid={}），使用根元素",
                    focused_pid,
                    root_pid
                );
                focused = None;
            }
        }
    }

    // 优先使用焦点元素，否则使用根元素
    let element = focused
        .or(root)
        .ok_or_else(|| anyhow!("UIA: 无法获取焦点元素且 ElementFromHandle 失败"))?;

    let text = read_text_from_element(&element)?;
    Ok(normalize_text(text))
}

/// 规范化文本（统一换行符）
fn normalize_text(mut text: String) -> String {
    if text.contains('\r') {
        text = text.replace("\r\n", "\n").replace('\r', "\n");
    }
    text
}

/// 从 UI Automation 元素读取文本
///
/// 尝试顺序：
/// 1. TextPattern（最完整）
/// 2. ValuePattern（适用于输入框）
/// 3. CurrentName（最后手段）
fn read_text_from_element(element: &IUIAutomationElement) -> Result<String> {
    // 1) TextPattern
    if let Ok(text) = read_via_text_pattern(element) {
        return Ok(text);
    }

    // 2) ValuePattern
    if let Ok(text) = read_via_value_pattern(element) {
        return Ok(text);
    }

    // 3) Fallback: CurrentName
    let name = unsafe { element.CurrentName() }.context("UIA CurrentName 失败")?;
    Ok(name.to_string())
}

/// 通过 TextPattern 读取文本
fn read_via_text_pattern(element: &IUIAutomationElement) -> Result<String> {
    let unk = unsafe { element.GetCurrentPattern(UIA_TextPatternId) }
        .context("UIA GetCurrentPattern(UIA_TextPatternId) 失败")?;

    let pattern: IUIAutomationTextPattern = unk
        .cast()
        .context("UIA 转换为 IUIAutomationTextPattern 失败")?;

    let range = unsafe { pattern.DocumentRange() }.context("UIA DocumentRange 失败")?;
    let bstr = unsafe { range.GetText(-1) }.context("UIA GetText(-1) 失败")?;
    Ok(bstr.to_string())
}

/// 通过 ValuePattern 读取文本
fn read_via_value_pattern(element: &IUIAutomationElement) -> Result<String> {
    let unk = unsafe { element.GetCurrentPattern(UIA_ValuePatternId) }
        .context("UIA GetCurrentPattern(UIA_ValuePatternId) 失败")?;

    let pattern: IUIAutomationValuePattern = unk
        .cast()
        .context("UIA 转换为 IUIAutomationValuePattern 失败")?;

    let bstr = unsafe { pattern.CurrentValue() }.context("UIA CurrentValue 失败")?;
    Ok(bstr.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_helper_times_out() {
        let start = Instant::now();
        let res: Result<()> = run_with_timeout(Duration::from_millis(50), || {
            std::thread::sleep(Duration::from_millis(200));
            Ok(())
        });
        assert!(res.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn test_blacklist_trips_after_repeated_failures() {
        // 重置全局状态
        {
            let mut state = blacklist_state().lock().unwrap();
            state.by_hwnd.clear();
        }

        let hwnd = 0x1234isize;
        let t0 = Instant::now();

        assert!(!is_blacklisted(hwnd, t0));
        assert!(!record_failure(hwnd, t0));
        assert!(!is_blacklisted(hwnd, t0));

        let t1 = t0 + Duration::from_secs(1);
        assert!(!record_failure(hwnd, t1));
        assert!(!is_blacklisted(hwnd, t1));

        let t2 = t0 + Duration::from_secs(2);
        assert!(record_failure(hwnd, t2));
        assert!(is_blacklisted(hwnd, t2));
    }

    #[test]
    fn test_normalize_text() {
        assert_eq!(normalize_text("hello\r\nworld".to_string()), "hello\nworld");
        assert_eq!(normalize_text("hello\rworld".to_string()), "hello\nworld");
        assert_eq!(normalize_text("hello\nworld".to_string()), "hello\nworld");
    }
}
