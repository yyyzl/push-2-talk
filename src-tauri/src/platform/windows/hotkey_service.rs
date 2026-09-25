// 全局快捷键监听模块 - 单例模式重构 + 双模式支持
use crate::config::{DualHotkeyConfig, HotkeyConfig, HotkeyKey, TriggerMode};
use anyhow::Result;
#[cfg(not(target_os = "windows"))]
use rdev::{listen, Event, EventType, Key};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

// ================== Windows 物理按键状态检测 ==================
// 用于解决 rdev 可能漏掉 KeyRelease 事件的问题（Ghost Key）

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

/// 检查特定按键的物理状态是否真的被按下
/// 使用 Windows GetAsyncKeyState API 直接查询硬件状态
#[cfg(target_os = "windows")]
fn is_key_physically_down(key: &HotkeyKey) -> bool {
    let vk_code = match key {
        // --- 修饰键 ---
        HotkeyKey::ControlLeft => 0xA2,  // VK_LCONTROL
        HotkeyKey::ControlRight => 0xA3, // VK_RCONTROL
        HotkeyKey::ShiftLeft => 0xA0,    // VK_LSHIFT
        HotkeyKey::ShiftRight => 0xA1,   // VK_RSHIFT
        HotkeyKey::AltLeft => 0xA4,      // VK_LMENU
        HotkeyKey::AltRight => 0xA5,     // VK_RMENU
        HotkeyKey::MetaLeft => 0x5B,     // VK_LWIN
        HotkeyKey::MetaRight => 0x5C,    // VK_RWIN

        // --- 字母键 (A-Z) ---
        // Windows VK Code 对于字母键直接对应大写 ASCII 码
        HotkeyKey::KeyA => 0x41,
        HotkeyKey::KeyB => 0x42,
        HotkeyKey::KeyC => 0x43,
        HotkeyKey::KeyD => 0x44,
        HotkeyKey::KeyE => 0x45,
        HotkeyKey::KeyF => 0x46,
        HotkeyKey::KeyG => 0x47,
        HotkeyKey::KeyH => 0x48,
        HotkeyKey::KeyI => 0x49,
        HotkeyKey::KeyJ => 0x4A,
        HotkeyKey::KeyK => 0x4B,
        HotkeyKey::KeyL => 0x4C,
        HotkeyKey::KeyM => 0x4D,
        HotkeyKey::KeyN => 0x4E,
        HotkeyKey::KeyO => 0x4F,
        HotkeyKey::KeyP => 0x50,
        HotkeyKey::KeyQ => 0x51,
        HotkeyKey::KeyR => 0x52,
        HotkeyKey::KeyS => 0x53,
        HotkeyKey::KeyT => 0x54,
        HotkeyKey::KeyU => 0x55,
        HotkeyKey::KeyV => 0x56,
        HotkeyKey::KeyW => 0x57,
        HotkeyKey::KeyX => 0x58,
        HotkeyKey::KeyY => 0x59,
        HotkeyKey::KeyZ => 0x5A,

        // --- 数字键 (Top Row) ---
        HotkeyKey::Num0 => 0x30,
        HotkeyKey::Num1 => 0x31,
        HotkeyKey::Num2 => 0x32,
        HotkeyKey::Num3 => 0x33,
        HotkeyKey::Num4 => 0x34,
        HotkeyKey::Num5 => 0x35,
        HotkeyKey::Num6 => 0x36,
        HotkeyKey::Num7 => 0x37,
        HotkeyKey::Num8 => 0x38,
        HotkeyKey::Num9 => 0x39,

        // --- 功能键 ---
        HotkeyKey::F1 => 0x70,
        HotkeyKey::F2 => 0x71,
        HotkeyKey::F3 => 0x72,
        HotkeyKey::F4 => 0x73,
        HotkeyKey::F5 => 0x74,
        HotkeyKey::F6 => 0x75,
        HotkeyKey::F7 => 0x76,
        HotkeyKey::F8 => 0x77,
        HotkeyKey::F9 => 0x78,
        HotkeyKey::F10 => 0x79,
        HotkeyKey::F11 => 0x7A,
        HotkeyKey::F12 => 0x7B,

        // --- 常用功能键 ---
        HotkeyKey::Space => 0x20,
        HotkeyKey::Tab => 0x09,
        HotkeyKey::Escape => 0x1B,
        HotkeyKey::Return => 0x0D,
        HotkeyKey::Backspace => 0x08,
        HotkeyKey::Delete => 0x2E,
        HotkeyKey::Insert => 0x2D,

        // --- 方向键 ---
        HotkeyKey::Up => 0x26,
        HotkeyKey::Down => 0x28,
        HotkeyKey::Left => 0x25,
        HotkeyKey::Right => 0x27,

        // --- 导航键 ---
        HotkeyKey::Home => 0x24,
        HotkeyKey::End => 0x23,
        HotkeyKey::PageUp => 0x21,
        HotkeyKey::PageDown => 0x22,

        // --- 大写锁定 ---
        HotkeyKey::CapsLock => 0x14,
    };

    unsafe {
        // GetAsyncKeyState 返回值的最高位（0x8000）表示按键当前是否按下
        (GetAsyncKeyState(vk_code) as u16 & 0x8000) != 0
    }
}

/// 非 Windows 系统默认返回 true（不做额外检查）
#[cfg(not(target_os = "windows"))]
fn is_key_physically_down(_key: &HotkeyKey) -> bool {
    true
}

/// 检查一组按键是否全部物理按下
fn are_keys_physically_down(keys: &[HotkeyKey]) -> bool {
    keys.iter().all(|k| is_key_physically_down(k))
}

// Windows 下使用轮询（避免低级 hook 导致的按键异常）
#[cfg(target_os = "windows")]
const HOTKEY_POLL_INTERVAL_MS: u64 = 10;

/// 严格匹配：要求目标按键全部按下，且没有额外的修饰键被按下
#[cfg(target_os = "windows")]
fn is_hotkey_pressed_strict(target_keys: &[HotkeyKey]) -> bool {
    if target_keys.is_empty() {
        return false;
    }

    if !are_keys_physically_down(target_keys) {
        return false;
    }

    // 检查是否有“额外修饰键”被按下（用于模拟 rdev 的严格匹配 len==keys.len() 行为）
    const MODIFIERS: [HotkeyKey; 8] = [
        HotkeyKey::ControlLeft,
        HotkeyKey::ControlRight,
        HotkeyKey::ShiftLeft,
        HotkeyKey::ShiftRight,
        HotkeyKey::AltLeft,
        HotkeyKey::AltRight,
        HotkeyKey::MetaLeft,
        HotkeyKey::MetaRight,
    ];

    for modifier in MODIFIERS.iter() {
        if is_key_physically_down(modifier) && !target_keys.contains(modifier) {
            return false;
        }
    }

    true
}

/// 热键状态
#[derive(Debug, Default)]
struct HotkeyState {
    is_recording: bool,
    pressed_keys: HashSet<HotkeyKey>,
    watchdog_running: bool,
    /// 当前触发的模式（如果正在录音）
    current_trigger_mode: Option<TriggerMode>,
    /// 是否通过松手模式快捷键启动（直接进入锁定状态）
    is_release_mode_triggered: bool,
}

/// 回调函数类型（接收触发模式参数和是否为松手模式）
/// 第一个参数：TriggerMode - 听写或AI助手
/// 第二个参数：bool - 是否为松手模式（true=松手模式，false=普通模式）
type Callback = Arc<dyn Fn(TriggerMode, bool) + Send + Sync>;

/// 单例热键服务（支持双模式）
pub struct HotkeyService {
    /// 服务是否激活（控制是否响应热键事件）
    is_active: Arc<AtomicBool>,
    /// 听写模式快捷键配置
    dictation_config: Arc<RwLock<HotkeyConfig>>,
    /// AI助手模式快捷键配置
    assistant_config: Arc<RwLock<HotkeyConfig>>,
    /// 内部状态
    state: Arc<Mutex<HotkeyState>>,
    /// 监听线程是否已启动
    listener_started: Arc<AtomicBool>,
    /// 回调函数（现在接收 TriggerMode 参数）
    on_start: Arc<RwLock<Option<Callback>>>,
    on_stop: Arc<RwLock<Option<Callback>>>,
}

impl HotkeyService {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            dictation_config: Arc::new(RwLock::new(HotkeyConfig::default())),
            assistant_config: Arc::new(RwLock::new(HotkeyConfig {
                keys: vec![HotkeyKey::AltLeft, HotkeyKey::Space],
                mode: crate::config::HotkeyMode::Press,
                enable_release_lock: false,
                release_mode_keys: None, // AI助手模式不支持松手模式
            })),
            state: Arc::new(Mutex::new(HotkeyState::default())),
            listener_started: Arc::new(AtomicBool::new(false)),
            on_start: Arc::new(RwLock::new(None)),
            on_stop: Arc::new(RwLock::new(None)),
        }
    }

    pub fn is_service_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    pub fn resume(&self) {
        self.reset_state();
        self.is_active.store(true, Ordering::SeqCst);
    }

    /// 将 rdev::Key 转换为 HotkeyKey
    #[cfg(not(target_os = "windows"))]
    fn rdev_to_hotkey_key(key: Key) -> Option<HotkeyKey> {
        match key {
            Key::ControlLeft => Some(HotkeyKey::ControlLeft),
            Key::ControlRight => Some(HotkeyKey::ControlRight),
            Key::ShiftLeft => Some(HotkeyKey::ShiftLeft),
            Key::ShiftRight => Some(HotkeyKey::ShiftRight),
            Key::Alt => Some(HotkeyKey::AltLeft),
            Key::AltGr => Some(HotkeyKey::AltRight),
            Key::MetaLeft => Some(HotkeyKey::MetaLeft),
            Key::MetaRight => Some(HotkeyKey::MetaRight),
            Key::Space => Some(HotkeyKey::Space),
            Key::Tab => Some(HotkeyKey::Tab),
            Key::CapsLock => Some(HotkeyKey::CapsLock),
            Key::Escape => Some(HotkeyKey::Escape),
            Key::F1 => Some(HotkeyKey::F1),
            Key::F2 => Some(HotkeyKey::F2),
            Key::F3 => Some(HotkeyKey::F3),
            Key::F4 => Some(HotkeyKey::F4),
            Key::F5 => Some(HotkeyKey::F5),
            Key::F6 => Some(HotkeyKey::F6),
            Key::F7 => Some(HotkeyKey::F7),
            Key::F8 => Some(HotkeyKey::F8),
            Key::F9 => Some(HotkeyKey::F9),
            Key::F10 => Some(HotkeyKey::F10),
            Key::F11 => Some(HotkeyKey::F11),
            Key::F12 => Some(HotkeyKey::F12),
            Key::KeyA => Some(HotkeyKey::KeyA),
            Key::KeyB => Some(HotkeyKey::KeyB),
            Key::KeyC => Some(HotkeyKey::KeyC),
            Key::KeyD => Some(HotkeyKey::KeyD),
            Key::KeyE => Some(HotkeyKey::KeyE),
            Key::KeyF => Some(HotkeyKey::KeyF),
            Key::KeyG => Some(HotkeyKey::KeyG),
            Key::KeyH => Some(HotkeyKey::KeyH),
            Key::KeyI => Some(HotkeyKey::KeyI),
            Key::KeyJ => Some(HotkeyKey::KeyJ),
            Key::KeyK => Some(HotkeyKey::KeyK),
            Key::KeyL => Some(HotkeyKey::KeyL),
            Key::KeyM => Some(HotkeyKey::KeyM),
            Key::KeyN => Some(HotkeyKey::KeyN),
            Key::KeyO => Some(HotkeyKey::KeyO),
            Key::KeyP => Some(HotkeyKey::KeyP),
            Key::KeyQ => Some(HotkeyKey::KeyQ),
            Key::KeyR => Some(HotkeyKey::KeyR),
            Key::KeyS => Some(HotkeyKey::KeyS),
            Key::KeyT => Some(HotkeyKey::KeyT),
            Key::KeyU => Some(HotkeyKey::KeyU),
            Key::KeyV => Some(HotkeyKey::KeyV),
            Key::KeyW => Some(HotkeyKey::KeyW),
            Key::KeyX => Some(HotkeyKey::KeyX),
            Key::KeyY => Some(HotkeyKey::KeyY),
            Key::KeyZ => Some(HotkeyKey::KeyZ),
            Key::Num0 => Some(HotkeyKey::Num0),
            Key::Num1 => Some(HotkeyKey::Num1),
            Key::Num2 => Some(HotkeyKey::Num2),
            Key::Num3 => Some(HotkeyKey::Num3),
            Key::Num4 => Some(HotkeyKey::Num4),
            Key::Num5 => Some(HotkeyKey::Num5),
            Key::Num6 => Some(HotkeyKey::Num6),
            Key::Num7 => Some(HotkeyKey::Num7),
            Key::Num8 => Some(HotkeyKey::Num8),
            Key::Num9 => Some(HotkeyKey::Num9),
            Key::UpArrow => Some(HotkeyKey::Up),
            Key::DownArrow => Some(HotkeyKey::Down),
            Key::LeftArrow => Some(HotkeyKey::Left),
            Key::RightArrow => Some(HotkeyKey::Right),
            Key::Return => Some(HotkeyKey::Return),
            Key::Backspace => Some(HotkeyKey::Backspace),
            Key::Delete => Some(HotkeyKey::Delete),
            Key::Insert => Some(HotkeyKey::Insert),
            Key::Home => Some(HotkeyKey::Home),
            Key::End => Some(HotkeyKey::End),
            Key::PageUp => Some(HotkeyKey::PageUp),
            Key::PageDown => Some(HotkeyKey::PageDown),
            _ => None,
        }
    }

    /// 初始化监听线程（只调用一次，带自动重启机制）
    pub fn init_listener(&self) -> Result<()> {
        // 防止重复启动
        if self.listener_started.swap(true, Ordering::SeqCst) {
            tracing::debug!("监听线程已启动，跳过重复初始化");
            return Ok(());
        }

        tracing::info!("初始化全局快捷键监听线程（双模式）");

        let is_active = Arc::clone(&self.is_active);
        let dictation_config = Arc::clone(&self.dictation_config);
        let assistant_config = Arc::clone(&self.assistant_config);
        let state = Arc::clone(&self.state);
        let on_start = Arc::clone(&self.on_start);
        let on_stop = Arc::clone(&self.on_stop);

        thread::spawn(move || {
            tracing::info!("快捷键监听线程已启动");

            // ====================================================================
            // Windows：使用 GetAsyncKeyState 轮询按键状态，避免低级 hook 的兼容性问题
            // ====================================================================
            #[cfg(target_os = "windows")]
            {
                tracing::info!(
                    "Windows 热键监听：启用轮询模式 ({}ms)",
                    HOTKEY_POLL_INTERVAL_MS
                );

                let mut prev_dictation_down = false;
                let mut prev_assistant_down = false;
                let mut prev_release_down = false;

                loop {
                    thread::sleep(Duration::from_millis(HOTKEY_POLL_INTERVAL_MS));

                    let dictation_cfg = dictation_config.read().unwrap().clone();
                    let assistant_cfg = assistant_config.read().unwrap().clone();

                    let dictation_down = is_hotkey_pressed_strict(&dictation_cfg.keys);
                    let assistant_down = is_hotkey_pressed_strict(&assistant_cfg.keys);
                    let release_down = dictation_cfg
                        .release_mode_keys
                        .as_deref()
                        .map(is_hotkey_pressed_strict)
                        .unwrap_or(false);

                    // 未激活时：同步边沿状态，避免激活瞬间误触发
                    if !is_active.load(Ordering::Relaxed) {
                        prev_dictation_down = dictation_down;
                        prev_assistant_down = assistant_down;
                        prev_release_down = release_down;
                        continue;
                    }

                    let dictation_rise = dictation_down && !prev_dictation_down;
                    let dictation_fall = !dictation_down && prev_dictation_down;
                    let assistant_rise = assistant_down && !prev_assistant_down;
                    let assistant_fall = !assistant_down && prev_assistant_down;
                    let release_rise = release_down && !prev_release_down;

                    // 更新 pressed_keys（仅用于调试信息）
                    {
                        let mut s = state.lock().unwrap();
                        s.pressed_keys.clear();

                        // 只追踪当前配置相关的按键，避免无意义的全键盘扫描
                        let mut keys_to_check: HashSet<HotkeyKey> = HashSet::new();
                        for key in dictation_cfg.keys.iter() {
                            keys_to_check.insert(key.clone());
                        }
                        for key in assistant_cfg.keys.iter() {
                            keys_to_check.insert(key.clone());
                        }
                        if let Some(ref keys) = dictation_cfg.release_mode_keys {
                            for key in keys.iter() {
                                keys_to_check.insert(key.clone());
                            }
                        }

                        for key in keys_to_check.into_iter() {
                            if is_key_physically_down(&key) {
                                s.pressed_keys.insert(key);
                            }
                        }
                    }

                    let mut start_action: Option<(TriggerMode, bool)> = None;
                    let mut stop_action: Option<(TriggerMode, bool)> = None;

                    {
                        let mut s = state.lock().unwrap();

                        // === 松手模式：再次按下松手模式快捷键则取消录音 ===
                        if s.is_recording && s.is_release_mode_triggered && release_rise {
                            tracing::info!("松手模式下再次按下快捷键，取消录音");
                            s.is_recording = false;
                            s.watchdog_running = false;
                            s.current_trigger_mode = None;
                            s.is_release_mode_triggered = false;
                            stop_action = Some((TriggerMode::Dictation, true));
                        } else if !s.is_recording {
                            // 确定触发模式（优先级：松手模式 > 普通听写 > AI助手）
                            if release_rise {
                                tracing::info!("检测到快捷键按下: 听写模式 (松手模式)");
                                s.is_recording = true;
                                s.current_trigger_mode = Some(TriggerMode::Dictation);
                                s.is_release_mode_triggered = true;
                                s.watchdog_running = false;
                                start_action = Some((TriggerMode::Dictation, true));
                            } else if dictation_rise {
                                let mode_desc = match dictation_cfg.mode {
                                    crate::config::HotkeyMode::Press => "普通模式",
                                    crate::config::HotkeyMode::Toggle => "切换模式",
                                };
                                tracing::info!("检测到快捷键按下: 听写模式 ({})", mode_desc);
                                s.is_recording = true;
                                s.current_trigger_mode = Some(TriggerMode::Dictation);
                                s.is_release_mode_triggered = false;
                                s.watchdog_running = false;
                                start_action = Some((TriggerMode::Dictation, false));
                            } else if assistant_rise {
                                let mode_desc = match assistant_cfg.mode {
                                    crate::config::HotkeyMode::Press => "普通模式",
                                    crate::config::HotkeyMode::Toggle => "切换模式",
                                };
                                tracing::info!("检测到快捷键按下: AI助手模式 ({})", mode_desc);
                                s.is_recording = true;
                                s.current_trigger_mode = Some(TriggerMode::AiAssistant);
                                s.is_release_mode_triggered = false;
                                s.watchdog_running = false;
                                start_action = Some((TriggerMode::AiAssistant, false));
                            }
                        } else if !s.is_release_mode_triggered {
                            // 录音中：根据当前触发模式处理停止逻辑（Press=松手停止；Toggle=再次按下停止）
                            match s.current_trigger_mode {
                                Some(TriggerMode::Dictation) => match dictation_cfg.mode {
                                    crate::config::HotkeyMode::Press => {
                                        if dictation_fall {
                                            tracing::info!("检测到快捷键释放，停止录音");
                                            s.is_recording = false;
                                            s.watchdog_running = false;
                                            s.current_trigger_mode = None;
                                            stop_action = Some((TriggerMode::Dictation, false));
                                        }
                                    }
                                    crate::config::HotkeyMode::Toggle => {
                                        if dictation_rise {
                                            tracing::info!(
                                                "检测到快捷键再次按下，停止录音（切换模式）"
                                            );
                                            s.is_recording = false;
                                            s.watchdog_running = false;
                                            s.current_trigger_mode = None;
                                            stop_action = Some((TriggerMode::Dictation, false));
                                        }
                                    }
                                },
                                Some(TriggerMode::AiAssistant) => match assistant_cfg.mode {
                                    crate::config::HotkeyMode::Press => {
                                        if assistant_fall {
                                            tracing::info!("检测到快捷键释放，停止录音");
                                            s.is_recording = false;
                                            s.watchdog_running = false;
                                            s.current_trigger_mode = None;
                                            stop_action = Some((TriggerMode::AiAssistant, false));
                                        }
                                    }
                                    crate::config::HotkeyMode::Toggle => {
                                        if assistant_rise {
                                            tracing::info!(
                                                "检测到快捷键再次按下，停止录音（切换模式）"
                                            );
                                            s.is_recording = false;
                                            s.watchdog_running = false;
                                            s.current_trigger_mode = None;
                                            stop_action = Some((TriggerMode::AiAssistant, false));
                                        }
                                    }
                                },
                                None => {}
                            }
                        }
                    }

                    if let Some((mode, is_release_mode)) = start_action {
                        if let Some(cb) = on_start.read().unwrap().as_ref() {
                            cb(mode, is_release_mode);
                        }
                    }
                    if let Some((mode, is_release_mode)) = stop_action {
                        if let Some(cb) = on_stop.read().unwrap().as_ref() {
                            cb(mode, is_release_mode);
                        }
                    }

                    prev_dictation_down = dictation_down;
                    prev_assistant_down = assistant_down;
                    prev_release_down = release_down;
                }
            }

            // 外层循环：如果 rdev 监听器崩溃则自动重启
            #[cfg(not(target_os = "windows"))]
            loop {
                let mut first_key_logged = false;

                // 克隆变量供闭包使用
                let is_active_inner = Arc::clone(&is_active);
                let dictation_config_inner = Arc::clone(&dictation_config);
                let assistant_config_inner = Arc::clone(&assistant_config);
                let state_inner = Arc::clone(&state);
                let on_start_inner = Arc::clone(&on_start);
                let on_stop_inner = Arc::clone(&on_stop);

                let callback = move |event: Event| {
                    // 检查服务是否激活
                    if !is_active_inner.load(Ordering::Relaxed) {
                        return;
                    }

                    // 第一次检测到按键时记录
                    if !first_key_logged && matches!(event.event_type, EventType::KeyPress(_)) {
                        first_key_logged = true;
                        tracing::info!("✓ rdev 正常工作 - 已检测到键盘事件");
                    }

                    match event.event_type {
                        EventType::KeyPress(key) => {
                            if let Some(hotkey_key) = Self::rdev_to_hotkey_key(key) {
                                let dictation_cfg = dictation_config_inner.read().unwrap().clone();
                                let assistant_cfg = assistant_config_inner.read().unwrap().clone();
                                let mut s = state_inner.lock().unwrap();

                                s.pressed_keys.insert(hotkey_key);

                                // 调试日志：检测按键数量异常（可能有键卡死）
                                let max_keys =
                                    dictation_cfg.keys.len().max(assistant_cfg.keys.len());
                                if s.pressed_keys.len() > max_keys + 2 {
                                    // 仅在确实异常时输出，使用 debug 级别避免日志刷屏
                                    tracing::debug!(
                                        "当前按下按键数 ({}) 异常偏多，可能有按键状态卡死: {:?}",
                                        s.pressed_keys.len(),
                                        s.pressed_keys
                                    );
                                }

                                // 严格匹配：检查是否匹配三种快捷键配置
                                let (matches_dictation, matches_assistant, matches_release_mode) = {
                                    // 听写模式快捷键
                                    let contains_dictation = dictation_cfg
                                        .keys
                                        .iter()
                                        .all(|k| s.pressed_keys.contains(k));
                                    let count_dictation =
                                        s.pressed_keys.len() == dictation_cfg.keys.len();

                                    // AI助手模式快捷键
                                    let contains_assistant = assistant_cfg
                                        .keys
                                        .iter()
                                        .all(|k| s.pressed_keys.contains(k));
                                    let count_assistant =
                                        s.pressed_keys.len() == assistant_cfg.keys.len();

                                    // 松手模式快捷键（仅听写模式支持）
                                    let matches_release = if let Some(ref release_keys) =
                                        dictation_cfg.release_mode_keys
                                    {
                                        let contains_release =
                                            release_keys.iter().all(|k| s.pressed_keys.contains(k));
                                        let count_release =
                                            s.pressed_keys.len() == release_keys.len();
                                        contains_release && count_release
                                    } else {
                                        false
                                    };

                                    (
                                        contains_dictation && count_dictation,
                                        contains_assistant && count_assistant,
                                        matches_release,
                                    )
                                };

                                // === 松手模式：检查是否需要取消录音（再次按下相同快捷键） ===
                                if s.is_recording
                                    && s.is_release_mode_triggered
                                    && matches_release_mode
                                {
                                    tracing::info!("松手模式下再次按下快捷键，取消录音");
                                    s.is_recording = false;
                                    s.watchdog_running = false;
                                    s.current_trigger_mode = None;
                                    s.is_release_mode_triggered = false;
                                    drop(s);
                                    // 调用 on_stop 回调（传递 true 表示是松手模式取消）
                                    if let Some(cb) = on_stop_inner.read().unwrap().as_ref() {
                                        cb(TriggerMode::Dictation, true); // 松手模式取消
                                    }
                                    return;
                                }

                                if !s.is_recording {
                                    // 确定触发模式（优先级：松手模式 > 普通听写 > AI助手）
                                    let (trigger_mode, is_release_mode) = if matches_release_mode {
                                        (Some(TriggerMode::Dictation), true)
                                    } else if matches_dictation {
                                        (Some(TriggerMode::Dictation), false)
                                    } else if matches_assistant {
                                        (Some(TriggerMode::AiAssistant), false)
                                    } else {
                                        (None, false)
                                    };

                                    if let Some(mode) = trigger_mode {
                                        s.is_recording = true;
                                        s.current_trigger_mode = Some(mode);
                                        s.is_release_mode_triggered = is_release_mode;
                                        let mode_name = mode.display_name();
                                        let mode_desc = if is_release_mode {
                                            "松手模式"
                                        } else {
                                            "普通模式"
                                        };
                                        tracing::info!(
                                            "检测到快捷键按下: {} ({})",
                                            mode_name,
                                            mode_desc
                                        );

                                        // 启动看门狗
                                        if s.watchdog_running {
                                            drop(s);
                                            if let Some(cb) =
                                                on_start_inner.read().unwrap().as_ref()
                                            {
                                                cb(mode, is_release_mode); // 传递松手模式标志
                                            }
                                            return;
                                        }

                                        s.watchdog_running = true;
                                        drop(s);

                                        // 启动看门狗线程
                                        let state_wd = Arc::clone(&state_inner);
                                        let dictation_cfg_wd = Arc::clone(&dictation_config_inner);
                                        let assistant_cfg_wd = Arc::clone(&assistant_config_inner);
                                        let is_active_wd = Arc::clone(&is_active_inner);
                                        let on_stop_wd = Arc::clone(&on_stop_inner);

                                        thread::spawn(move || {
                                            tracing::debug!("看门狗线程已启动");
                                            let mut release_detected_count: u64 = 0;
                                            let required_count = (KEY_RELEASE_STABLE_MS
                                                / WATCHDOG_INTERVAL_MS)
                                                .max(1);

                                            loop {
                                                thread::sleep(Duration::from_millis(
                                                    WATCHDOG_INTERVAL_MS,
                                                ));

                                                // 检查服务是否仍然激活
                                                if !is_active_wd.load(Ordering::Relaxed) {
                                                    let mut s = state_wd.lock().unwrap();
                                                    s.watchdog_running = false;
                                                    s.is_recording = false;
                                                    s.current_trigger_mode = None;
                                                    tracing::debug!("看门狗线程退出（服务已停止）");
                                                    break;
                                                }

                                                let s = state_wd.lock().unwrap();
                                                if !s.watchdog_running || !s.is_recording {
                                                    tracing::debug!("看门狗线程正常退出");
                                                    break;
                                                }

                                                // 根据当前触发模式检查对应的按键
                                                // 双重检查：软件状态 + 硬件物理状态
                                                // 这样即使 rdev 漏掉了 KeyRelease 事件，也能通过硬件状态检测到
                                                let (all_pressed, target_keys) = match s
                                                    .current_trigger_mode
                                                {
                                                    Some(TriggerMode::Dictation) => {
                                                        let cfg = dictation_cfg_wd.read().unwrap();
                                                        let soft_pressed = cfg
                                                            .keys
                                                            .iter()
                                                            .all(|k| s.pressed_keys.contains(k));
                                                        (soft_pressed, cfg.keys.clone())
                                                    }
                                                    Some(TriggerMode::AiAssistant) => {
                                                        let cfg = assistant_cfg_wd.read().unwrap();
                                                        let soft_pressed = cfg
                                                            .keys
                                                            .iter()
                                                            .all(|k| s.pressed_keys.contains(k));
                                                        (soft_pressed, cfg.keys.clone())
                                                    }
                                                    None => (false, vec![]),
                                                };
                                                drop(s);

                                                // 硬件状态检查：使用 GetAsyncKeyState 直接查询物理按键状态
                                                // 只要有一个键物理上松开了，就认为用户已松手
                                                let hardware_pressed = if !target_keys.is_empty() {
                                                    are_keys_physically_down(&target_keys)
                                                } else {
                                                    false
                                                };

                                                // 最终判断：软件状态和硬件状态都要按下才算真正按着
                                                let truly_pressed = all_pressed && hardware_pressed;

                                                if !truly_pressed {
                                                    release_detected_count += 1;
                                                    if release_detected_count >= required_count {
                                                        let mut s = state_wd.lock().unwrap();
                                                        if s.is_recording {
                                                            // 检查是否为松手模式
                                                            if s.is_release_mode_triggered {
                                                                // 松手模式下，检测到按键释放后清理软件状态，但录音继续
                                                                s.pressed_keys.clear();
                                                                tracing::info!("看门狗检测到松手模式快捷键释放（硬件状态同步），录音继续");
                                                                drop(s);
                                                                break; // 退出看门狗，但不停止录音
                                                            }

                                                            let mode = s
                                                                .current_trigger_mode
                                                                .unwrap_or(TriggerMode::Dictation);
                                                            s.is_recording = false;
                                                            s.watchdog_running = false;
                                                            s.current_trigger_mode = None;
                                                            s.is_release_mode_triggered = false;
                                                            // 清理可能卡住的按键状态
                                                            s.pressed_keys.clear();
                                                            drop(s);

                                                            // 区分是软件检测还是硬件检测
                                                            if !all_pressed {
                                                                tracing::warn!("看门狗检测到按键释放（软件状态），强制停止录音");
                                                            } else {
                                                                tracing::warn!("看门狗检测到按键释放（硬件状态同步），强制停止录音");
                                                            }
                                                            if let Some(cb) =
                                                                on_stop_wd.read().unwrap().as_ref()
                                                            {
                                                                cb(mode, false);
                                                                // 传递 false（非松手模式）
                                                            }
                                                        }
                                                        break;
                                                    }
                                                } else {
                                                    release_detected_count = 0;
                                                }
                                            }

                                            let mut s = state_wd.lock().unwrap();
                                            s.watchdog_running = false;
                                        });

                                        if let Some(cb) = on_start_inner.read().unwrap().as_ref() {
                                            cb(mode, is_release_mode); // 传递松手模式标志
                                        }
                                    }
                                }
                            }
                        }
                        EventType::KeyRelease(key) => {
                            if let Some(hotkey_key) = Self::rdev_to_hotkey_key(key) {
                                let dictation_cfg = dictation_config_inner.read().unwrap().clone();
                                let assistant_cfg = assistant_config_inner.read().unwrap().clone();
                                let mut s = state_inner.lock().unwrap();

                                s.pressed_keys.remove(&hotkey_key);

                                // 增强的防呆逻辑：如果释放的是修饰键且未录音，检查是否所有修饰键都已释放
                                if hotkey_key.is_modifier() && !s.is_recording {
                                    let has_any_modifier =
                                        s.pressed_keys.iter().any(|k| k.is_modifier());
                                    if !has_any_modifier && !s.pressed_keys.is_empty() {
                                        // 所有修饰键已释放，但还有其他键残留，可能是状态卡死
                                        tracing::warn!(
                                            "所有修饰键已释放但仍有残留按键: {:?}，强制清理",
                                            s.pressed_keys
                                        );
                                        s.pressed_keys.clear();
                                    } else if !has_any_modifier {
                                        s.pressed_keys.clear();
                                        tracing::debug!("所有修饰键已释放，强制清理按键状态");
                                    }
                                }

                                if !s.is_recording {
                                    return;
                                }

                                // 根据当前触发模式检查对应的按键是否全部按下
                                let all_pressed = match s.current_trigger_mode {
                                    Some(TriggerMode::Dictation) => dictation_cfg
                                        .keys
                                        .iter()
                                        .all(|k| s.pressed_keys.contains(k)),
                                    Some(TriggerMode::AiAssistant) => assistant_cfg
                                        .keys
                                        .iter()
                                        .all(|k| s.pressed_keys.contains(k)),
                                    None => false,
                                };

                                if !all_pressed {
                                    // === 松手模式：检查是否为松手模式快捷键触发 ===
                                    if s.is_release_mode_triggered {
                                        tracing::info!("松手模式快捷键释放，录音继续（锁定状态）");
                                        return; // 不停止录音，等待用户点击悬浮窗按钮
                                    }

                                    let mode =
                                        s.current_trigger_mode.unwrap_or(TriggerMode::Dictation);
                                    s.is_recording = false;
                                    s.watchdog_running = false;
                                    s.current_trigger_mode = None;
                                    s.is_release_mode_triggered = false; // 重置标志
                                    drop(s);

                                    tracing::info!("检测到快捷键释放，停止录音");
                                    if let Some(cb) = on_stop_inner.read().unwrap().as_ref() {
                                        cb(mode, false); // 释放时不是松手模式
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                };

                // 执行监听
                tracing::info!("开始执行 rdev listen...");
                if let Err(error) = listen(callback) {
                    tracing::error!(
                        "rdev 监听器发生错误退出: {:?}。将在 2 秒后重启监听。",
                        error
                    );
                } else {
                    tracing::warn!(
                        "rdev 监听器意外正常返回（通常不应发生）。将在 2 秒后重启监听。"
                    );
                }

                // 重启前重置状态，防止按键卡死
                {
                    let mut s = state.lock().unwrap();
                    s.pressed_keys.clear();
                    s.is_recording = false;
                    s.watchdog_running = false;
                    s.current_trigger_mode = None;
                }

                // 等待一会再重启，避免死循环占用 CPU
                thread::sleep(Duration::from_secs(2));
                tracing::info!("正在重启 rdev 监听器...");
            }
        });

        Ok(())
    }

    /// 激活双模式快捷键服务（新接口）
    ///
    /// # Arguments
    /// * `config` - 双快捷键配置（听写模式 + AI助手模式）
    /// * `on_start` - 开始录音回调（接收 TriggerMode 参数）
    /// * `on_stop` - 停止录音回调（接收 TriggerMode 和 is_release_mode 参数）
    pub fn activate_dual<F1, F2>(
        &self,
        config: DualHotkeyConfig,
        on_start: F1,
        on_stop: F2,
    ) -> Result<()>
    where
        F1: Fn(TriggerMode, bool) + Send + Sync + 'static,
        F2: Fn(TriggerMode, bool) + Send + Sync + 'static,
    {
        // 验证配置
        config.validate()?;

        tracing::info!(
            "激活双模式快捷键服务 (听写: {}, AI助手: {})",
            config.dictation.format_display(),
            config.assistant.format_display()
        );

        // 更新配置
        *self.dictation_config.write().unwrap() = config.dictation;
        *self.assistant_config.write().unwrap() = config.assistant;

        // 更新回调
        *self.on_start.write().unwrap() = Some(Arc::new(on_start));
        *self.on_stop.write().unwrap() = Some(Arc::new(on_stop));

        // 重置状态
        {
            let mut s = self.state.lock().unwrap();
            s.is_recording = false;
            s.pressed_keys.clear();
            s.watchdog_running = false;
            s.current_trigger_mode = None;
        }

        // 确保监听线程已启动
        self.init_listener()?;

        // 激活服务
        self.is_active.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// 停用服务（不终止线程）
    pub fn deactivate(&self) {
        tracing::info!("停用快捷键服务");
        self.is_active.store(false, Ordering::SeqCst);

        // 重置状态
        let mut s = self.state.lock().unwrap();
        s.is_recording = false;
        s.pressed_keys.clear();
        s.watchdog_running = false;
        s.current_trigger_mode = None;
    }

    /// 强制重置热键状态（用于手动修复状态卡死问题）
    pub fn reset_state(&self) {
        let mut s = self.state.lock().unwrap();
        tracing::info!(
            "强制重置热键状态。清理前按键: {:?}, is_recording: {}",
            s.pressed_keys,
            s.is_recording
        );
        s.pressed_keys.clear();
        s.is_recording = false;
        s.watchdog_running = false;
        s.current_trigger_mode = None;
    }

    /// 获取当前状态信息（用于调试）
    pub fn get_debug_info(&self) -> String {
        let s = self.state.lock().unwrap();
        let dictation_cfg = self.dictation_config.read().unwrap();
        let assistant_cfg = self.assistant_config.read().unwrap();
        format!(
            "is_active: {}, is_recording: {}, pressed_keys: {:?}, trigger_mode: {:?}, dictation_hotkey: {}, assistant_hotkey: {}",
            self.is_active.load(Ordering::Relaxed),
            s.is_recording,
            s.pressed_keys,
            s.current_trigger_mode,
            dictation_cfg.format_display(),
            assistant_cfg.format_display()
        )
    }
}
