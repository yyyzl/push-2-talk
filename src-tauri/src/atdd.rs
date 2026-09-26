//! Opt-in acceptance driver. Real recording, ASR, LLM and insertion; no fake responses.
mod lifecycle;

use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Listener, Manager};
use tokio::sync::watch;

const HEADER: &str = "PushToTalk ATDD\n\n";
const SELECTED: &str = "The meeting starts at three tomorrow.";

#[derive(Clone, Copy, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scenario {
    #[default]
    Dictation,
    AssistantQuestion,
    AssistantSelection,
}

#[derive(Clone, Copy, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Application {
    #[default]
    TextEdit,
    Chrome,
    Safari,
}

struct Session {
    scenario: Scenario,
    cancel: watch::Sender<bool>,
    accept_cancel: bool,
    expect_start: bool,
    startup: Option<tauri::async_runtime::JoinHandle<()>>,
    selection_matches: Option<bool>,
}
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

struct RunGuard {
    app: tauri::AppHandle,
    events: Vec<tauri::EventId>,
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        for event in &self.events {
            self.app.unlisten(*event);
        }
        if let Some(mut session) = SESSION.lock().unwrap().take() {
            if let Some(task) = session.startup.take() {
                task.abort();
            }
        }
    }
}

// Called only by the feature-gated hook around the real startup callback.
pub(crate) fn track_start_task(task: tauri::async_runtime::JoinHandle<()>) {
    if let Some(session) = SESSION.lock().unwrap().as_mut() {
        if session.expect_start {
            session.expect_start = false;
            session.startup = Some(task);
        }
    }
}

// Observe equality only: never retain or report text from another application.
pub(crate) fn observe_selection(text: Option<&str>) {
    if let Some(session) = SESSION.lock().unwrap().as_mut() {
        session.selection_matches = match session.scenario {
            Scenario::AssistantSelection => Some(text == Some(SELECTED)),
            Scenario::AssistantQuestion => Some(text.is_none()),
            Scenario::Dictation => None,
        };
    }
}

#[tauri::command]
pub(crate) fn atdd_cancel() -> Result<String, String> {
    let lock = SESSION.lock().unwrap();
    let session = lock.as_ref().ok_or("没有正在执行的验收")?;
    if !session.accept_cancel {
        return Err("录音已经结束，请在助手结果面板处理结果".into());
    }
    session.cancel.send(true).map_err(|e| e.to_string())?;
    Ok("正在取消本轮验收…".into())
}

async fn cancel_owned_recording(app: &tauri::AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    if !state.hotkey_service.atdd_owns(id) {
        return Ok(());
    }
    let result = if state.is_recording_locked.load(Ordering::SeqCst) {
        crate::cancel_locked_recording(app.clone()).await
    } else {
        let result = crate::cancel_transcription(app.clone()).await;
        *state.current_trigger_mode.lock().unwrap() = None;
        if let Some(manager) = state.audio_mute_manager.lock().unwrap().as_ref() {
            manager.end_session();
            let _ = manager.restore_volumes();
        }
        result
    };
    state.hotkey_service.atdd_abort(id);
    *state.recording_start_instant.lock().unwrap() = None;
    result.map(|_| ())
}

#[tauri::command]
pub(crate) async fn run(
    app: tauri::AppHandle,
    scenario: Option<Scenario>,
    application: Option<Application>,
    inspect_only: Option<bool>,
) -> Result<String, String> {
    let scenario = scenario.unwrap_or_default();
    let application = application.unwrap_or_default();
    let inspect_only = inspect_only.unwrap_or(false);
    let (cancel, mut cancelled) = watch::channel(false);
    {
        let mut session = SESSION.lock().unwrap();
        if session.is_some() {
            return Err("已有验收在执行".into());
        }
        *session = Some(Session {
            scenario,
            cancel,
            accept_cancel: true,
            expect_start: false,
            startup: None,
            selection_matches: None,
        });
    }
    let mut guard = RunGuard {
        app: app.clone(),
        events: vec![],
    };
    let state = app.state::<crate::AppState>();
    let service = state.hotkey_service.clone();
    if inspect_only && service.is_service_active() {
        return Err("目标检查不使用麦克风；请先暂停服务，避免快捷键启动录音".into());
    }
    if !inspect_only
        && (!service.is_service_active() || !crate::platform::desktop().status().ready())
    {
        return Err("需要正常启动服务并授予全部权限".into());
    }
    if state.conversation_session.lock().unwrap().is_some()
        || state.is_assistant_processing.load(Ordering::SeqCst)
    {
        return Err("请先结束现有助手会话，再开始独立验收".into());
    }
    if inspect_only
        && (state.current_trigger_mode.lock().unwrap().is_some()
            || state.is_processing_stop.load(Ordering::SeqCst)
            || state
                .streaming_recorder
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|r| r.is_recording())
            || state
                .audio_recorder
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|r| r.is_recording()))
    {
        return Err("请等待现有录音与处理结束，再检查目标".into());
    }
    let llm_ready = state.assistant_processor.lock().unwrap().is_some();
    if !lifecycle::wait_delay(Duration::from_secs(5), &mut cancelled).await {
        return Ok("已取消准备，没有启动录音".into());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let extension = match application {
        Application::TextEdit => "txt",
        Application::Chrome => "html",
        Application::Safari => "safari.html",
    };
    let fixture = std::env::temp_dir().join(format!("PushToTalk-ATDD-{timestamp}.{extension}"));
    let contents = if scenario == Scenario::AssistantSelection {
        format!("{HEADER}{SELECTED}\n")
    } else {
        HEADER.to_string()
    };
    let (start, length) = if scenario == Scenario::AssistantSelection {
        (
            HEADER.encode_utf16().count() as u64,
            SELECTED.encode_utf16().count() as u64,
        )
    } else {
        (contents.encode_utf16().count() as u64, 0)
    };
    let document = if application != Application::TextEdit {
        // Only the fixed non-sensitive fixture above is interpolated here.
        format!(
            r#"<!doctype html><meta charset="utf-8"><title>PushToTalk ATDD {timestamp}</title>
<style>body{{font:18px sans-serif;margin:40px}}textarea{{display:block;width:80vw;height:50vh;font:18px sans-serif}}</style>
<h1>PushToTalk ATDD</h1><label for="input">PushToTalk ATDD 输入框</label>
<textarea id="input" autofocus>{contents}</textarea>"#
        )
    } else {
        contents.clone()
    };
    std::fs::write(&fixture, document).map_err(|e| e.to_string())?;
    let setup_path = fixture.clone();
    let prepared_target = tokio::task::spawn_blocking(move || {
        crate::platform::atdd_prepare_fixture(&setup_path, &contents, start, length)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if *cancelled.borrow() {
        return Ok("已取消准备，没有启动录音".into());
    }
    if inspect_only {
        let description = crate::platform::atdd_target_description(Some(prepared_target));
        return Ok(format!(
            "目标检查完成；未启动录音。{description}。文档：{}",
            fixture.display()
        ));
    }

    let outcome = Arc::new(AtomicU8::new(0));
    for (event, result) in [
        ("assistant_turn_complete", 1),
        ("error", 2),
        ("transcription_complete", 3),
        ("transcription_cancelled", 4),
    ] {
        let observed = outcome.clone();
        guard.events.push(app.listen(event, move |_| {
            observed.store(result, Ordering::SeqCst);
        }));
    }
    SESSION.lock().unwrap().as_mut().unwrap().expect_start = true;
    let mode = if scenario == Scenario::Dictation {
        crate::config::TriggerMode::Dictation
    } else {
        crate::config::TriggerMode::AiAssistant
    };
    let id = service.atdd_recording(mode).map_err(|e| e.to_string())?;
    let startup = SESSION.lock().unwrap().as_mut().unwrap().startup.take();
    let Some(startup) = startup else {
        cancel_owned_recording(&app, id).await?;
        return Err("未取得本轮录音初始化任务，已取消".into());
    };
    match lifecycle::await_startup(startup, &mut cancelled, Duration::from_secs(30)).await {
        Ok(true) => {}
        result => {
            cancel_owned_recording(&app, id).await?;
            return result.map(|_| "已取消录音初始化，没有启动后续处理".into());
        }
    }
    let recording = state
        .streaming_recorder
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|r| r.is_recording())
        || state
            .audio_recorder
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|r| r.is_recording());
    if !recording {
        cancel_owned_recording(&app, id).await?;
        return Err("录音未成功初始化，请查看应用错误提示".into());
    }
    let target = *state.target_window.lock().unwrap();
    let before = crate::platform::atdd_target_description(target);
    if !lifecycle::wait_delay(Duration::from_secs(18), &mut cancelled).await {
        cancel_owned_recording(&app, id).await?;
        return Ok(format!("已取消本轮录音；文档：{}", fixture.display()));
    }
    SESSION.lock().unwrap().as_mut().unwrap().accept_cancel = false;
    service.atdd_finish(id);
    // Wait for actual pipeline events. A timeout is reported, never counted as success.
    for _ in 0..900 {
        if outcome.load(Ordering::SeqCst) != 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let result = match outcome.load(Ordering::SeqCst) {
        1 => "真实助手结果已返回，请在结果面板验收并插入",
        2 => "处理失败，请查看应用错误提示",
        3 => "听写流水线已返回，请核对原文档与历史",
        4 => "录音已被取消",
        _ => "等待结果超时，不能判定通过；请检查当前处理状态",
    };
    let selection = match SESSION.lock().unwrap().as_ref().unwrap().selection_matches {
        Some(true) => "符合场景",
        Some(false) => "不符合场景",
        None => "未观察",
    };
    let after = crate::platform::atdd_target_description(target);
    Ok(format!("{result}；实际选区捕获={selection}；LLM配置={}。开始目标：{before}。结束目标：{after}。文档：{}",
        if llm_ready { "已配置" } else { "未配置" }, fixture.display()))
}
