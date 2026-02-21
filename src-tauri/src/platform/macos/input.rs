use anyhow::{anyhow, Result};
use cocoa::appkit::NSApplicationActivationOptions;
use cocoa::base::{id, nil, BOOL, YES};
use cocoa::foundation::NSUInteger;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode, KeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc::{class, msg_send};

use crate::platform::types::WindowId;

const KEY_C: CGKeyCode = 0x08;
const KEY_V: CGKeyCode = 0x09;
const FOCUS_RETRY_DELAY_MS: u64 = 50;

fn create_event_source() -> Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| anyhow!("创建 CGEventSource 失败"))
}

fn post_key_event(
    source: &CGEventSource,
    keycode: CGKeyCode,
    keydown: bool,
    flags: CGEventFlags,
) -> Result<()> {
    let event = CGEvent::new_keyboard_event(source.clone(), keycode, keydown)
        .map_err(|_| anyhow!("创建键盘事件失败: keycode={keycode}, keydown={keydown}"))?;
    event.set_flags(flags);
    event.post(CGEventTapLocation::HID);
    Ok(())
}

fn perform_shortcut(
    keycode: CGKeyCode,
    modifier_keycode: CGKeyCode,
    modifier_flag: CGEventFlags,
) -> Result<()> {
    let source = create_event_source()?;

    post_key_event(
        &source,
        modifier_keycode,
        true,
        CGEventFlags::CGEventFlagNull,
    )?;
    post_key_event(&source, keycode, true, modifier_flag)?;
    post_key_event(&source, keycode, false, modifier_flag)?;
    post_key_event(
        &source,
        modifier_keycode,
        false,
        CGEventFlags::CGEventFlagNull,
    )?;

    Ok(())
}

fn app_for_pid(pid: WindowId) -> Option<id> {
    if pid <= 0 {
        return None;
    }

    unsafe {
        let app: id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if app == nil {
            None
        } else {
            Some(app)
        }
    }
}

fn frontmost_pid() -> Option<WindowId> {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return None;
        }

        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return None;
        }

        let pid: WindowId = msg_send![app, processIdentifier];
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

pub fn send_copy() -> Result<()> {
    release_all_modifiers()?;
    perform_shortcut(KEY_C, KeyCode::COMMAND, CGEventFlags::CGEventFlagCommand)
}

pub fn send_paste() -> Result<()> {
    release_all_modifiers()?;
    perform_shortcut(KEY_V, KeyCode::COMMAND, CGEventFlags::CGEventFlagCommand)
}

pub fn release_all_modifiers() -> Result<()> {
    let source = create_event_source()?;
    let modifiers = [
        KeyCode::COMMAND,
        KeyCode::RIGHT_COMMAND,
        KeyCode::OPTION,
        KeyCode::RIGHT_OPTION,
        KeyCode::CONTROL,
        KeyCode::RIGHT_CONTROL,
        KeyCode::SHIFT,
        KeyCode::RIGHT_SHIFT,
    ];

    for keycode in modifiers {
        let _ = post_key_event(&source, keycode, false, CGEventFlags::CGEventFlagNull);
    }

    Ok(())
}

pub fn get_foreground_window() -> Option<WindowId> {
    frontmost_pid()
}

pub fn is_window_valid(id: WindowId) -> bool {
    app_for_pid(id).is_some()
}

pub fn force_foreground_window(id: WindowId) -> Result<()> {
    let app = app_for_pid(id).ok_or_else(|| anyhow!("目标进程不存在: {id}"))?;
    let options =
        NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps as NSUInteger;
    let activated: BOOL = unsafe { msg_send![app, activateWithOptions: options] };
    if activated == YES {
        Ok(())
    } else {
        Err(anyhow!("激活前台进程失败: {id}"))
    }
}

pub fn restore_focus_with_verify(id: WindowId, max_retries: u32) -> bool {
    if !is_window_valid(id) {
        return false;
    }

    if force_foreground_window(id).is_err() {
        return false;
    }
    if verify_foreground_window(id) {
        return true;
    }

    for _ in 0..max_retries {
        std::thread::sleep(std::time::Duration::from_millis(FOCUS_RETRY_DELAY_MS));
        if verify_foreground_window(id) {
            return true;
        }
        let _ = force_foreground_window(id);
    }

    false
}

pub fn verify_foreground_window(id: WindowId) -> bool {
    get_foreground_window() == Some(id)
}
