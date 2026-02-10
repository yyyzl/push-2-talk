// Windows 原生键盘输入模块
// 使用 Win32 SendInput API 替代跨平台 enigo 库
// 提供更低延迟的键盘模拟功能

use anyhow::Result;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_V,
};

/// 按键间延迟（毫秒）
/// 保守设置以确保在各种应用中稳定工作
const KEY_DELAY_MS: u64 = 15;

/// 检查指定虚拟键是否被按下
#[cfg(target_os = "windows")]
fn is_vk_pressed(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// 发送单个按键按下事件
#[cfg(target_os = "windows")]
fn send_key_down(vk: VIRTUAL_KEY) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let result = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };

    if result == 0 {
        anyhow::bail!("SendInput failed for key down: {:?}", vk);
    }

    Ok(())
}

/// 发送单个按键释放事件
#[cfg(target_os = "windows")]
fn send_key_up(vk: VIRTUAL_KEY) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let result = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };

    if result == 0 {
        anyhow::bail!("SendInput failed for key up: {:?}", vk);
    }

    Ok(())
}

/// 模拟 Ctrl+C 组合键（复制）
#[cfg(target_os = "windows")]
pub fn send_ctrl_c() -> Result<()> {
    tracing::debug!("win32_input: 发送 Ctrl+C");

    // 按下 Ctrl
    send_key_down(VK_CONTROL)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 按下并释放 C
    send_key_down(VK_C)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
    send_key_up(VK_C)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 释放 Ctrl
    send_key_up(VK_CONTROL)?;

    Ok(())
}

/// 模拟 Ctrl+V 组合键（粘贴）
#[cfg(target_os = "windows")]
pub fn send_ctrl_v() -> Result<()> {
    tracing::debug!("win32_input: 发送 Ctrl+V");

    // 按下 Ctrl
    send_key_down(VK_CONTROL)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 按下并释放 V
    send_key_down(VK_V)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
    send_key_up(VK_V)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 释放 Ctrl
    send_key_up(VK_CONTROL)?;

    Ok(())
}

/// 释放所有修饰键（防御性措施）
/// 用于确保热键释放后不会有残留的修饰键状态
/// 只释放真正被按下的键，避免发送虚假的 key_up 事件触发系统行为
#[cfg(target_os = "windows")]
pub fn release_all_modifiers() -> Result<()> {
    tracing::debug!("win32_input: 释放所有修饰键（仅释放被按下的键）");

    // 只释放真正被按下的修饰键，避免触发系统行为（如 Win 键释放触发开始菜单）
    if is_vk_pressed(VK_CONTROL) {
        let _ = send_key_up(VK_CONTROL);
    }
    if is_vk_pressed(VK_LCONTROL) {
        let _ = send_key_up(VK_LCONTROL);
    }
    if is_vk_pressed(VK_RCONTROL) {
        let _ = send_key_up(VK_RCONTROL);
    }
    if is_vk_pressed(VK_SHIFT) {
        let _ = send_key_up(VK_SHIFT);
    }
    if is_vk_pressed(VK_LSHIFT) {
        let _ = send_key_up(VK_LSHIFT);
    }
    if is_vk_pressed(VK_RSHIFT) {
        let _ = send_key_up(VK_RSHIFT);
    }
    if is_vk_pressed(VK_MENU) {
        let _ = send_key_up(VK_MENU);
    }
    if is_vk_pressed(VK_LMENU) {
        let _ = send_key_up(VK_LMENU);
    }
    if is_vk_pressed(VK_RMENU) {
        let _ = send_key_up(VK_RMENU);
    }
    if is_vk_pressed(VK_LWIN) {
        let _ = send_key_up(VK_LWIN);
    }
    if is_vk_pressed(VK_RWIN) {
        let _ = send_key_up(VK_RWIN);
    }

    Ok(())
}

// ==================== 焦点管理 API ====================
// 用于在文本插入前确保目标窗口获得焦点

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
};

/// 获取当前前台窗口句柄
///
/// # 返回值
/// * `Some(isize)` - 前台窗口句柄（HWND 转为 isize 以便跨线程传递）
/// * `None` - 没有前台窗口
#[cfg(target_os = "windows")]
pub fn get_foreground_window() -> Option<isize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            None
        } else {
            Some(hwnd.0 as isize)
        }
    }
}

/// 检查窗口句柄是否有效
#[cfg(target_os = "windows")]
pub fn is_window_valid(hwnd: isize) -> bool {
    unsafe { IsWindow(HWND(hwnd as *mut _)).as_bool() }
}

/// 验证当前前台窗口是否为指定窗口
#[cfg(target_os = "windows")]
pub fn verify_foreground_window(expected_hwnd: isize) -> bool {
    get_foreground_window() == Some(expected_hwnd)
}

/// 强制设置前台窗口（使用多重策略）
///
/// 绕过 Windows 对 SetForegroundWindow 的限制
///
/// # 策略
/// 1. 直接调用 SetForegroundWindow
/// 2. 使用 AttachThreadInput 技巧
/// 3. 使用 keybd_event(Alt) 技巧
///
/// # 参数
/// * `hwnd` - 目标窗口句柄
///
/// # 返回值
/// * `Ok(())` - 焦点恢复成功
/// * `Err(e)` - 焦点恢复失败
#[cfg(target_os = "windows")]
pub fn force_foreground_window(hwnd: isize) -> Result<()> {
    use std::ptr::null_mut;

    // 检查窗口是否有效
    if !is_window_valid(hwnd) {
        anyhow::bail!("目标窗口已无效");
    }

    let target_hwnd = HWND(hwnd as *mut _);

    unsafe {
        // 策略1：直接尝试 SetForegroundWindow
        if SetForegroundWindow(target_hwnd).as_bool() {
            tracing::debug!("win32_input: SetForegroundWindow 直接成功");
            return Ok(());
        }

        tracing::debug!(
            "win32_input: SetForegroundWindow 直接调用失败，尝试 AttachThreadInput 技巧"
        );

        // 策略2：使用 AttachThreadInput 技巧
        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(target_hwnd, Some(null_mut()));

        if target_thread == 0 {
            tracing::warn!("win32_input: 无法获取目标窗口线程 ID");
        } else {
            // 附加到目标线程
            let attached = AttachThreadInput(current_thread, target_thread, true).as_bool();

            // 尝试设置前台窗口
            let result = SetForegroundWindow(target_hwnd).as_bool();

            // 分离线程（无论成功与否）
            if attached {
                let _ = AttachThreadInput(current_thread, target_thread, false);
            }

            if result {
                tracing::debug!("win32_input: AttachThreadInput 技巧成功");
                return Ok(());
            }
        }

        // 策略3：keybd_event 技巧（最后手段）
        tracing::debug!("win32_input: AttachThreadInput 技巧失败，尝试 keybd_event 技巧");

        // 发送一个 Alt 按键事件，让系统认为当前进程在处理输入
        let _ = send_key_down(VK_MENU);
        thread::sleep(Duration::from_millis(5));
        let _ = send_key_up(VK_MENU);
        thread::sleep(Duration::from_millis(5));

        // 再次尝试
        if SetForegroundWindow(target_hwnd).as_bool() {
            tracing::debug!("win32_input: keybd_event 技巧成功");
            Ok(())
        } else {
            // 最后检查：可能焦点已经在目标窗口了
            if verify_foreground_window(hwnd) {
                tracing::debug!("win32_input: 焦点已在目标窗口");
                Ok(())
            } else {
                anyhow::bail!("所有焦点恢复策略均失败")
            }
        }
    }
}

/// 恢复目标窗口焦点（带验证和重试）
///
/// 尝试恢复焦点并验证是否成功
///
/// # 参数
/// * `hwnd` - 目标窗口句柄
/// * `max_retries` - 最大重试次数
///
/// # 返回值
/// * `true` - 焦点恢复成功
/// * `false` - 焦点恢复失败
#[cfg(target_os = "windows")]
pub fn restore_focus_with_verify(hwnd: isize, max_retries: u32) -> bool {
    for attempt in 0..max_retries {
        // 尝试恢复焦点
        if let Err(e) = force_foreground_window(hwnd) {
            tracing::warn!("win32_input: 焦点恢复尝试 {} 失败: {}", attempt + 1, e);
            thread::sleep(Duration::from_millis(30));
            continue;
        }

        // 等待一小段时间让焦点稳定
        thread::sleep(Duration::from_millis(30));

        // 验证焦点
        if verify_foreground_window(hwnd) {
            tracing::info!("win32_input: 焦点恢复成功 (尝试 {})", attempt + 1);
            return true;
        }

        tracing::debug!("win32_input: 焦点验证失败，重试...");
        thread::sleep(Duration::from_millis(20));
    }

    tracing::warn!("win32_input: 焦点恢复失败，已达最大重试次数");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_send_ctrl_c() {
        // 注意：此测试会实际发送键盘事件
        // 仅在开发环境手动运行
        // let result = send_ctrl_c();
        // assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_send_ctrl_v() {
        // 注意：此测试会实际发送键盘事件
        // 仅在开发环境手动运行
        // let result = send_ctrl_v();
        // assert!(result.is_ok());
    }
}
