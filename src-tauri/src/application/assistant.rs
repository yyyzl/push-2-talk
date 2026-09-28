//! Production assistant workflow: voice/text instructions, conversation and result actions.
use super::{
    runtime::AppState,
    transcription::{
        http_fallback_provider, is_audio_skip_error, transcribe_with_available_clients,
        TranscriptionResult,
    },
};
use crate::shell::windows::{
    emit_error_and_hide_overlay, hide_overlay_silently, hide_overlay_window,
    hide_result_panel_window, show_result_panel_window,
};
use crate::{
    asr::{
        DoubaoASRClient, DoubaoImeRealtimeSession, DoubaoRealtimeSession, QwenASRClient,
        RealtimeSession, SenseVoiceClient,
    },
    assistant_processor::{
        self, AssistantProcessor, AssistantStreamEvent, TurnOutcome, WebSearchPreference,
    },
    audio_recorder::AudioRecorder,
    clipboard_manager, config, pipeline,
    platform::{self, InputTarget},
    search,
    streaming_recorder::StreamingRecorder,
    usage_stats::UsageStats,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub(crate) struct AssistantState {
    pub processor: Arc<Mutex<Option<AssistantProcessor>>>,
    pub conversation: Arc<Mutex<Option<ConversationSession>>>,
    pub processing: Arc<AtomicBool>,
    pub cancel_token: Arc<Mutex<Option<CancellationToken>>>,
}

// ================== 多轮对话数据结构 ==================

/// 对话提示词模式（首轮锁定，追问不变）
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PromptMode {
    /// 问答模式（无选中文本时使用）
    QA,
    /// 文本处理模式（有选中文本时使用）
    TextProcessing,
}

/// 单轮对话记录
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConversationTurn {
    pub user_instruction: String,
    pub selected_text: Option<String>,
    pub assistant_response: String,
    pub asr_time_ms: u64,
    pub llm_time_ms: u64,
    pub search_time_ms: Option<u64>,
    pub tool_calls: Vec<search::AssistantToolCall>,
}

/// 多轮对话会话（替代 PendingAssistantResult）
#[allow(dead_code)]
pub(crate) struct ConversationSession {
    pub id: String,
    pub turns: Vec<ConversationTurn>,
    pub pending_turn: Option<TurnPendingPayload>,
    pub draft_turn_id: Option<String>,
    pub draft_assistant_response: String,
    pub draft_tool_calls: Vec<search::AssistantToolCall>,
    pub draft_status: String,
    pub draft_warning: Option<String>,
    pub draft_web_search_enabled: Option<bool>,
    /// 首轮锁定的提示词模式
    pub system_prompt_mode: PromptMode,
    /// 首轮触发时的目标窗口句柄
    pub target_hwnd: Option<InputTarget>,
    pub created_at: std::time::Instant,
}

// ================== 多轮对话事件 Payload ==================

/// 单轮对话的前端 payload
#[derive(Clone, serde::Serialize)]
pub(crate) struct ConversationTurnPayload {
    user_instruction: String,
    selected_text: Option<String>,
    has_selection: bool,
    assistant_response: String,
    asr_time_ms: u64,
    llm_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_time_ms: Option<u64>,
    #[serde(default)]
    tool_calls: Vec<search::AssistantToolCall>,
}

/// 完整会话状态 payload（用于 pull 模式）
#[derive(Clone, serde::Serialize)]
pub(crate) struct ConversationStatePayload {
    session_id: String,
    turns: Vec<ConversationTurnPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_turn: Option<TurnPendingPayload>,
    #[serde(default)]
    draft_assistant_response: String,
    #[serde(default)]
    draft_tool_calls: Vec<search::AssistantToolCall>,
    #[serde(default)]
    is_processing: bool,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_search_enabled: Option<bool>,
}

/// 追问录音完成后立即发出（前端显示用户消息 + loading）
#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct TurnPendingPayload {
    turn_id: String,
    user_instruction: String,
    selected_text: Option<String>,
    has_selection: bool,
}

/// 一轮完成事件 payload
#[derive(Clone, serde::Serialize)]
pub(crate) struct TurnCompletePayload {
    session_id: String,
    turn: ConversationTurnPayload,
    is_followup: bool,
}

/// LLM 调用失败事件 payload
#[derive(Clone, serde::Serialize)]
pub(crate) struct TurnErrorPayload {
    session_id: String,
    error_message: String,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct TurnDeltaPayload {
    session_id: String,
    turn_id: String,
    content_delta: String,
    draft_assistant_response: String,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct TurnWarningPayload {
    session_id: String,
    turn_id: String,
    message: String,
}

fn emit_assistant_stream_event(
    app: &AppHandle,
    session_id: &str,
    turn_id: &str,
    event: AssistantStreamEvent,
    draft_assistant_response: Option<String>,
) {
    match event {
        AssistantStreamEvent::Delta { content_delta } => {
            let _ = app.emit(
                "assistant_turn_delta",
                TurnDeltaPayload {
                    session_id: session_id.to_string(),
                    turn_id: turn_id.to_string(),
                    draft_assistant_response: draft_assistant_response
                        .unwrap_or_else(|| content_delta.clone()),
                    content_delta,
                },
            );
        }
        AssistantStreamEvent::ToolCallStarted {
            id,
            name,
            query,
            round,
        } => {
            let _ = app.emit(
                "assistant_tool_call_started",
                serde_json::json!({
                    "session_id": session_id,
                    "turn_id": turn_id,
                    "id": id,
                    "name": name,
                    "query": query,
                    "round": round,
                }),
            );
        }
        AssistantStreamEvent::ToolCallFinished { call } => {
            let _ = app.emit(
                "assistant_tool_call_finished",
                serde_json::json!({
                    "session_id": session_id,
                    "turn_id": turn_id,
                    "call": call,
                }),
            );
        }
        AssistantStreamEvent::Warning { message } => {
            let _ = app.emit(
                "assistant_turn_warning",
                TurnWarningPayload {
                    session_id: session_id.to_string(),
                    turn_id: turn_id.to_string(),
                    message,
                },
            );
        }
    }
}

fn load_search_runtime_config() -> config::SearchConfig {
    crate::application::configuration::load_persisted_config()
        .map(|cfg| cfg.search_config)
        .unwrap_or_else(|e| {
            tracing::warn!("加载联网搜索配置失败，使用默认值: {}", e);
            config::SearchConfig::default()
        })
}

fn resolve_pending_web_search_enabled(
    processor: &AssistantProcessor,
    prompt_mode: &PromptMode,
    preference: WebSearchPreference,
    search_config: &config::SearchConfig,
) -> bool {
    processor.is_web_search_requested(prompt_mode, preference, Some(search_config))
        && search::SearchRegistry::runtime_unavailable_reason(search_config).is_none()
}

fn register_assistant_cancel_token(state: &AssistantState) -> CancellationToken {
    let token = CancellationToken::new();
    *state.cancel_token.lock().unwrap() = Some(token.clone());
    token
}

fn clear_assistant_cancel_token_after_turn(state: &AssistantState, token: &CancellationToken) {
    let mut guard = state.cancel_token.lock().unwrap();
    if token.is_cancelled() {
        if guard
            .as_ref()
            .map(|current| current.is_cancelled())
            .unwrap_or(false)
        {
            *guard = None;
        }
    } else {
        *guard = None;
    }
}

fn finish_assistant_turn_processing(
    state: &AssistantState,
    turn_id: &str,
    cancel_token: &CancellationToken,
) {
    let active_turn_id = state
        .conversation
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|session| session.draft_turn_id.clone());
    if active_turn_id
        .as_deref()
        .is_some_and(|active| active != turn_id)
    {
        return;
    }

    state.processing.store(false, Ordering::SeqCst);
    clear_assistant_cancel_token_after_turn(state, cancel_token);
}

fn log_assistant_turn_cancelled(context: &str, err: &anyhow::Error) {
    tracing::info!("{context}已取消: {}", err);
}

fn new_assistant_turn_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn set_conversation_pending(
    state: &AssistantState,
    session_id: &str,
    turn_id: &str,
    pending: TurnPendingPayload,
    web_search_enabled: bool,
) -> bool {
    let mut lock = state.conversation.lock().unwrap();
    let Some(session) = lock.as_mut() else {
        return false;
    };
    if session.id != session_id {
        return false;
    }

    session.pending_turn = Some(pending);
    session.draft_turn_id = Some(turn_id.to_string());
    session.draft_assistant_response.clear();
    session.draft_tool_calls.clear();
    session.draft_status = "processing".to_string();
    session.draft_warning = None;
    session.draft_web_search_enabled = Some(web_search_enabled);
    true
}

fn update_conversation_draft_from_event(
    state: &AssistantState,
    session_id: &str,
    turn_id: &str,
    event: &AssistantStreamEvent,
) -> Option<String> {
    let mut lock = state.conversation.lock().unwrap();
    let Some(session) = lock.as_mut() else {
        return None;
    };
    if session.id != session_id || session.draft_turn_id.as_deref() != Some(turn_id) {
        return None;
    }

    match event {
        AssistantStreamEvent::Delta { content_delta } => {
            session.draft_assistant_response.push_str(content_delta);
        }
        AssistantStreamEvent::ToolCallStarted {
            id,
            name,
            query,
            round,
        } => {
            let started = search::AssistantToolCall {
                id: id.clone(),
                name: name.clone(),
                query: query.clone(),
                status: "searching".to_string(),
                results: Vec::new(),
                error: None,
                elapsed_ms: 0,
                round: *round,
            };
            session.draft_tool_calls.retain(|call| call.id != *id);
            session.draft_tool_calls.push(started);
        }
        AssistantStreamEvent::ToolCallFinished { call } => {
            session.draft_tool_calls.retain(|item| item.id != call.id);
            session.draft_tool_calls.push(call.clone());
        }
        AssistantStreamEvent::Warning { message } => {
            session.draft_warning = Some(message.clone());
        }
    }
    Some(session.draft_assistant_response.clone())
}

fn emit_and_record_assistant_stream_event(
    app: &AppHandle,
    session_id: &str,
    turn_id: &str,
    event: AssistantStreamEvent,
) {
    let draft_assistant_response = update_conversation_draft_from_event(
        &app.state::<AppState>().assistant,
        session_id,
        turn_id,
        &event,
    );
    emit_assistant_stream_event(app, session_id, turn_id, event, draft_assistant_response);
}

fn push_completed_turn_if_active(
    state: &AssistantState,
    session_id: &str,
    turn_id: &str,
    turn: ConversationTurn,
) -> bool {
    let mut lock = state.conversation.lock().unwrap();
    let Some(session) = lock.as_mut() else {
        return false;
    };
    if session.id != session_id || session.draft_turn_id.as_deref() != Some(turn_id) {
        return false;
    }
    if session.draft_status == "cancelled" {
        return false;
    }

    session.turns.push(turn);
    session.pending_turn = None;
    session.draft_turn_id = None;
    session.draft_assistant_response.clear();
    session.draft_tool_calls.clear();
    session.draft_status = "idle".to_string();
    session.draft_warning = None;
    session.draft_web_search_enabled = None;
    true
}

fn mark_conversation_error(
    state: &AssistantState,
    session_id: &str,
    turn_id: &str,
    message: String,
) {
    let mut lock = state.conversation.lock().unwrap();
    let Some(session) = lock.as_mut() else {
        return;
    };
    if session.id != session_id || session.draft_turn_id.as_deref() != Some(turn_id) {
        return;
    }
    session.pending_turn = None;
    session.draft_turn_id = None;
    session.draft_assistant_response.clear();
    session.draft_tool_calls.clear();
    session.draft_status = "error".to_string();
    session.draft_warning = Some(message);
    session.draft_web_search_enabled = None;
}

fn mark_active_conversation_cancelled(
    state: &AssistantState,
) -> (
    String,
    Option<String>,
    String,
    Vec<search::AssistantToolCall>,
) {
    let mut lock = state.conversation.lock().unwrap();
    let Some(session) = lock.as_mut() else {
        return (String::new(), None, String::new(), Vec::new());
    };
    session.draft_status = "cancelled".to_string();
    session.draft_warning = Some("已停止生成".to_string());
    (
        session.id.clone(),
        session.draft_turn_id.clone(),
        session.draft_assistant_response.clone(),
        session.draft_tool_calls.clone(),
    )
}

fn to_turn_payload(turn: &ConversationTurn) -> ConversationTurnPayload {
    ConversationTurnPayload {
        user_instruction: turn.user_instruction.clone(),
        selected_text: turn.selected_text.clone(),
        has_selection: turn.selected_text.is_some(),
        assistant_response: turn.assistant_response.clone(),
        asr_time_ms: turn.asr_time_ms,
        llm_time_ms: turn.llm_time_ms,
        search_time_ms: turn.search_time_ms,
        tool_calls: turn.tool_calls.clone(),
    }
}

fn turn_from_outcome(
    user_instruction: String,
    selected_text: Option<String>,
    asr_time_ms: u64,
    outcome: TurnOutcome,
) -> ConversationTurn {
    ConversationTurn {
        user_instruction,
        selected_text,
        assistant_response: outcome.assistant_response,
        asr_time_ms,
        llm_time_ms: outcome.llm_time_ms,
        search_time_ms: outcome.search_time_ms,
        tool_calls: outcome.tool_calls,
    }
}

fn add_candidate_arbitration_time(
    mut outcome: TurnOutcome,
    candidate_llm_time_ms: Option<u64>,
) -> TurnOutcome {
    if let Some(extra) = candidate_llm_time_ms {
        outcome.llm_time_ms = outcome.llm_time_ms.saturating_add(extra);
    }
    outcome
}

#[cfg(test)]
fn apply_assistant_personalization_with_store(
    text: String,
    store: crate::personalization::CorrectionPairStore,
) -> (String, bool, Option<crate::tnl::TnlDiagnostics>) {
    apply_assistant_personalization_with_store_and_config(
        text,
        store,
        crate::personalization::PersonalizationEngineConfig::default(),
    )
}

#[cfg(test)]
fn apply_assistant_personalization_with_store_and_config(
    text: String,
    store: crate::personalization::CorrectionPairStore,
    config: crate::personalization::PersonalizationEngineConfig,
) -> (String, bool, Option<crate::tnl::TnlDiagnostics>) {
    apply_assistant_personalization_with_store_and_config_and_spans(text, store, config, &[])
}

#[cfg(test)]
fn apply_assistant_personalization_with_store_and_config_and_spans(
    text: String,
    store: crate::personalization::CorrectionPairStore,
    config: crate::personalization::PersonalizationEngineConfig,
    technical_spans: &[crate::tnl::Span],
) -> (String, bool, Option<crate::tnl::TnlDiagnostics>) {
    let source_text = text.clone();
    let result = crate::personalization::apply_personalization_with_store_and_config_and_spans(
        text,
        store,
        config,
        technical_spans,
    );
    pipeline::text::log_personalization_result(&source_text, &result.conversion);

    let diagnostics =
        crate::personalization::personalization_candidates_to_tnl_diagnostics(&result.conversion);
    (result.text, result.changed, diagnostics)
}

async fn maybe_arbitrate_assistant_candidates(
    processor: &AssistantProcessor,
    text: String,
    diagnostics: Option<crate::tnl::TnlDiagnostics>,
) -> (String, Option<crate::tnl::TnlDiagnostics>, Option<u64>) {
    pipeline::text::arbitrate(text, diagnostics, |text, diagnostics| async move {
        processor.arbitrate_tnl_candidates(&text, diagnostics).await
    })
    .await
}

#[cfg(test)]
mod assistant_personalization_tests {
    use super::*;

    #[test]
    fn assistant_personalization_with_store_changes_known_pair() {
        let mut pair =
            crate::personalization::CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        let store = crate::personalization::CorrectionPairStore::new(vec![pair]);

        let (text, changed, diagnostics) =
            apply_assistant_personalization_with_store("我打开 cloud code".to_string(), store);

        assert!(changed);
        assert_eq!(text, "我打开 Claude Code");
        assert!(diagnostics.is_none());
    }

    #[test]
    fn assistant_personalization_respects_runtime_pass_config() {
        let mut pair =
            crate::personalization::CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "manual".to_string();
        pair.confidence = 0.98;
        pair.alias_keys.push("kelaode|code".to_string());
        let store = crate::personalization::CorrectionPairStore::new(vec![pair]);
        let config = crate::personalization::PersonalizationEngineConfig {
            enable_syllable_match_pass: false,
            ..crate::personalization::PersonalizationEngineConfig::default()
        };

        let (text, changed, diagnostics) = apply_assistant_personalization_with_store_and_config(
            "我打开 克劳德 code".to_string(),
            store,
            config,
        );

        assert!(!changed);
        assert_eq!(text, "我打开 克劳德 code");
        assert!(diagnostics.is_none());
    }

    #[test]
    fn assistant_personalization_exports_medium_confidence_candidate_for_arbitration() {
        let mut pair =
            crate::personalization::CorrectionPair::new("claude-code", "cloud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.accepted_count = 1;
        pair.confidence = 0.80;
        let store = crate::personalization::CorrectionPairStore::new(vec![pair]);

        let (text, changed, diagnostics) =
            apply_assistant_personalization_with_store("我打开 cloud code".to_string(), store);

        assert!(!changed);
        assert_eq!(text, "我打开 cloud code");
        let diagnostics = diagnostics.expect("medium confidence candidate should be exported");
        assert_eq!(diagnostics.pending_llm_count(), 1);
        assert_eq!(
            diagnostics.candidates[0].source,
            crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair
        );
        assert_eq!(
            diagnostics.candidates[0].decision,
            crate::tnl::TnlCandidateDecision::PendingLlm
        );
    }

    #[test]
    fn assistant_personalization_uses_named_entity_spans_for_alias_score() {
        let mut pair =
            crate::personalization::CorrectionPair::new("claude-code", "claud code", "Claude Code");
        pair.source = "learned".to_string();
        pair.accepted_count = 1;
        pair.confidence = 0.93;
        pair.alias_keys.push("kelaode|code".to_string());
        let store = crate::personalization::CorrectionPairStore::new(vec![pair]);
        let text = "我打开 克劳德 code";
        let span_start = text.find("克劳德").expect("named entity term");
        let spans = vec![crate::tnl::Span {
            text: "克劳德".to_string(),
            start: span_start,
            end: span_start + "克劳德".len(),
            span_type: crate::tnl::SpanType::NamedEntity,
        }];

        let (text, changed, diagnostics) =
            apply_assistant_personalization_with_store_and_config_and_spans(
                text.to_string(),
                store,
                crate::personalization::PersonalizationEngineConfig::default(),
                &spans,
            );

        assert!(changed);
        assert_eq!(text, "我打开 Claude Code");
        assert!(diagnostics.is_none());
    }

    #[test]
    fn assistant_merge_keeps_tnl_and_personalization_candidates() {
        let tnl = crate::tnl::TnlDiagnostics {
            candidates: vec![crate::tnl::TnlCandidate {
                id: "tnl-0".to_string(),
                original: "Cruiser".to_string(),
                target: "Cursor".to_string(),
                start: 0,
                end: 7,
                score: 0.72,
                risk: crate::tnl::TnlCandidateRisk::Medium,
                source: crate::tnl::TnlCandidateSource::DictionaryPhonetic,
                evidence: vec!["tnl".to_string()],
                decision: crate::tnl::TnlCandidateDecision::PendingLlm,
            }],
            arbitration: None,
        };
        let personalization = crate::tnl::TnlDiagnostics {
            candidates: vec![crate::tnl::TnlCandidate {
                id: "personalization-8-18-0".to_string(),
                original: "cloud code".to_string(),
                target: "Claude Code".to_string(),
                start: 8,
                end: 18,
                score: 0.80,
                risk: crate::tnl::TnlCandidateRisk::Medium,
                source: crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair,
                evidence: vec!["pair_id:claude-code".to_string()],
                decision: crate::tnl::TnlCandidateDecision::PendingLlm,
            }],
            arbitration: None,
        };

        let merged = pipeline::text::merge_tnl_diagnostics(Some(tnl), Some(personalization))
            .expect("merged diagnostics");

        assert_eq!(merged.candidates.len(), 2);
        assert_eq!(merged.pending_llm_count(), 2);
        assert_eq!(
            merged.candidates[1].source,
            crate::tnl::TnlCandidateSource::PersonalizationCorrectionPair
        );
    }

    #[test]
    fn assistant_turn_time_includes_candidate_arbitration_time() {
        let outcome = TurnOutcome {
            assistant_response: "ok".to_string(),
            tool_calls: Vec::new(),
            llm_time_ms: 120,
            search_time_ms: None,
        };

        let outcome = add_candidate_arbitration_time(outcome, Some(35));

        assert_eq!(outcome.llm_time_ms, 155);
    }
}

/// 将会话历史格式化并发送 transcription_complete 事件（用于 History 记录）
pub(crate) fn emit_conversation_history(
    app: &AppHandle,
    session: &ConversationSession,
    inserted: bool,
) {
    if session.turns.is_empty() {
        tracing::debug!("AI 助手会话没有已完成轮次，跳过历史记录事件");
        return;
    }

    let total_asr: u64 = session.turns.iter().map(|t| t.asr_time_ms).sum();
    let total_llm: u64 = session.turns.iter().map(|t| t.llm_time_ms).sum();
    let total_search: u64 = session.turns.iter().filter_map(|t| t.search_time_ms).sum();
    let tool_calls_summary: Vec<search::ToolCallSummary> = session
        .turns
        .iter()
        .flat_map(|turn| turn.tool_calls.iter().map(|call| call.summary()))
        .collect();
    let mut citation_remap = std::collections::HashMap::new();
    let mut citations = Vec::new();
    let mut next_citation_index = 1_u32;

    for (turn_idx, turn) in session.turns.iter().enumerate() {
        for call in &turn.tool_calls {
            for item in &call.results {
                let mut renumbered = item.clone();
                renumbered.index = next_citation_index;
                citation_remap.insert(format!("{turn_idx}:{}", item.id), next_citation_index);
                citations.push(renumbered);
                next_citation_index += 1;
            }
        }
    }

    let formatted = assistant_processor::format_conversation_for_copy_with_citation_remap(
        &session.turns,
        &citation_remap,
    );
    let web_searched = !tool_calls_summary.is_empty();
    let search_failed = web_searched
        && tool_calls_summary
            .iter()
            .all(|summary| summary.status != "success" || summary.results_count == 0);

    let result = TranscriptionResult {
        text: formatted,
        original_text: session.turns.first().map(|t| t.user_instruction.clone()),
        selected_text: session.turns.first().and_then(|t| t.selected_text.clone()),
        asr_time_ms: total_asr,
        llm_time_ms: Some(total_llm),
        total_time_ms: total_asr + total_llm + total_search,
        mode: Some("assistant".to_string()),
        inserted: Some(inserted),
        tnl_diagnostics: None,
        citations: web_searched.then_some(citations),
        tool_calls_summary: web_searched.then_some(tool_calls_summary),
        web_searched,
        search_failed,
    };
    let _ = app.emit("transcription_complete", result);
}

pub(crate) async fn handle_assistant_mode(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    doubao_ime_session: Arc<tokio::sync::Mutex<Option<DoubaoImeRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    assistant_processor: Arc<Mutex<Option<AssistantProcessor>>>,
    selected_text: Option<String>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    use_realtime: bool,
    target_hwnd: Option<InputTarget>, // 目标窗口句柄（用于焦点恢复）
    usage_stats: Arc<Mutex<UsageStats>>,
    recording_start_instant: Arc<Mutex<Option<std::time::Instant>>>,
) {
    let _ = app.emit("transcribing", ());
    let asr_start = std::time::Instant::now();

    // 1. 停止录音并获取音频数据
    let (asr_result, audio_data) = if use_realtime {
        // 实时模式：先停止流式录音
        let audio_data = {
            let mut recorder_guard = streaming_recorder.lock().unwrap();
            if let Some(ref mut rec) = *recorder_guard {
                match rec.stop_streaming() {
                    Ok(data) => Some(data),
                    Err(e) => {
                        tracing::error!("停止流式录音失败: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        };

        // 等待音频发送任务完成
        {
            let handle = audio_sender_handle.lock().unwrap().take();
            if let Some(h) = handle {
                tracing::info!("等待音频发送任务完成...");
                super::recording_resources::join_audio_sender(h).await;
            }
        }

        // 获取实时转录结果
        let provider = realtime_provider.lock().unwrap().clone();
        let result = match provider {
            Some(config::AsrProvider::Doubao) => {
                let mut session_guard = doubao_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.finish_audio().await;
                    let res = session.wait_for_result().await;
                    drop(session_guard);
                    *doubao_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的豆包会话"))
                }
            }
            Some(config::AsrProvider::DoubaoIme) => {
                let mut session_guard = doubao_ime_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.finish_audio().await;
                    let res = session.wait_for_result().await;
                    drop(session_guard);
                    *doubao_ime_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的豆包输入法会话"))
                }
            }
            _ => {
                let mut session_guard = active_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.commit_audio().await;
                    let res = session.wait_for_result().await;
                    let _ = session.close().await;
                    drop(session_guard);
                    *active_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的千问会话"))
                }
            }
        };

        (result, audio_data)
    } else {
        // HTTP 模式：停止录音并获取数据
        let audio_data = {
            let mut recorder_guard = recorder.lock().unwrap();
            if let Some(ref mut rec) = *recorder_guard {
                match rec.stop_recording_to_memory() {
                    Ok(data) => Some(data),
                    Err(e) => {
                        if is_audio_skip_error(&e) {
                            tracing::info!("音频已跳过: {}", e);
                            hide_overlay_silently(&app);
                        } else {
                            emit_error_and_hide_overlay(&app, format!("停止录音失败: {}", e));
                        }
                        None
                    }
                }
            } else {
                None
            }
        };

        let result = if let Some(ref data) = audio_data {
            // 使用 HTTP ASR
            let enable_fb = *enable_fallback_state.lock().unwrap();
            let qwen = { qwen_client_state.lock().unwrap().clone() };
            let doubao = { doubao_client_state.lock().unwrap().clone() };
            let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
            let active_prov = realtime_provider.lock().unwrap().clone();
            let fallback_prov = app
                .state::<AppState>()
                .fallback_provider
                .lock()
                .unwrap()
                .clone();

            transcribe_with_available_clients(
                qwen,
                doubao,
                sensevoice,
                data,
                enable_fb,
                active_prov,
                fallback_prov,
                "(AI助手HTTP) ",
            )
            .await
        } else {
            Err(anyhow::anyhow!("未获取到音频数据"))
        };

        (result, audio_data)
    };

    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    // 2. 如果实时模式失败且有音频数据，尝试 HTTP 备用
    let final_result = if asr_result.is_err() && audio_data.is_some() {
        tracing::warn!("实时 ASR 失败，尝试 HTTP 备用");
        let data = audio_data.unwrap();
        let enable_fb = *enable_fallback_state.lock().unwrap();
        let qwen = { qwen_client_state.lock().unwrap().clone() };
        let doubao = { doubao_client_state.lock().unwrap().clone() };
        let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
        let active_prov = realtime_provider.lock().unwrap().clone();
        let fallback_prov = app
            .state::<AppState>()
            .fallback_provider
            .lock()
            .unwrap()
            .clone();

        match http_fallback_provider(active_prov, fallback_prov.clone(), enable_fb) {
            Ok(effective_active_prov) => {
                transcribe_with_available_clients(
                    qwen,
                    doubao,
                    sensevoice,
                    &data,
                    enable_fb,
                    effective_active_prov,
                    fallback_prov,
                    "(AI助手备用) ",
                )
                .await
            }
            Err(error) => Err(error),
        }
    } else {
        asr_result
    };

    // 3. 解包 ASR 结果
    let asr_text = match final_result {
        Ok(text) => {
            tracing::info!("AI 助手 ASR 结果: {} ({}ms)", text, asr_time_ms);
            text
        }
        Err(e) => {
            hide_overlay_window(&app).await;
            let _ = recording_start_instant.lock().unwrap().take();
            tracing::error!("AI 助手 ASR 失败: {}", e);
            let _ = app.emit("error", format!("AI 助手处理失败: {}", e));
            return;
        }
    };

    // 空文本检查
    if asr_text.trim().is_empty() {
        hide_overlay_window(&app).await;
        let _ = recording_start_instant.lock().unwrap().take();
        tracing::info!("AI 助手: ASR 返回空文本，跳过处理");
        let _ = app.emit("error", "未识别到语音，请检查麦克风输入后重试");
        return;
    }

    // 4. TNL 技术规范化
    let dictionary = {
        let state = app.state::<AppState>();
        let dict = state.dictionary.lock().unwrap().clone();
        dict
    };
    let settings = app
        .state::<AppState>()
        .recording
        .settings
        .lock()
        .unwrap()
        .clone();
    let Some(settings) = settings else {
        emit_error_and_hide_overlay(&app, "录音配置快照缺失".into());
        return;
    };
    let prepared = pipeline::text::prepare(&asr_text, &dictionary, &settings.tnl_config);
    let user_instruction = prepared.text;
    let tnl_diagnostics = prepared.diagnostics;

    // 5. 获取 processor
    let processor = { assistant_processor.lock().unwrap().clone() };
    let Some(processor) = processor else {
        hide_overlay_window(&app).await;
        let _ = recording_start_instant.lock().unwrap().take();
        let _ = app.emit(
            "error",
            "AI 助手模式需要配置 LLM，请先在设置中配置 AI 助手 API".to_string(),
        );
        return;
    };
    let (user_instruction, tnl_diagnostics, candidate_llm_time_ms) =
        maybe_arbitrate_assistant_candidates(&processor, user_instruction, tnl_diagnostics).await;
    pipeline::text::record_personalization_arbitration_feedback(&tnl_diagnostics);

    // 6. 检查会话状态：分支新对话 / 追问
    let state = app.state::<AppState>();
    let session_info = {
        let lock = state.assistant.conversation.lock().unwrap();
        lock.as_ref()
            .map(|s| (s.id.clone(), s.turns.clone(), s.system_prompt_mode.clone()))
    };

    if let Some((session_id, history, prompt_mode)) = session_info {
        // =================== 追问路径 ===================

        // 原子 CAS 防并行追问：热键双触发（rdev ghost key）会导致两个管道并行进入此处，
        // 使用 compare_exchange 确保只有第一个管道能继续，第二个直接返回。
        if state
            .assistant
            .processing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::warn!("AI 助手: 已有追问在处理中，忽略并行触发（疑似热键双触发）");
            return;
        }

        let turn_id = new_assistant_turn_id();
        let web_search_preference = WebSearchPreference::UseConfig;
        let search_config = load_search_runtime_config();
        let web_search_enabled = resolve_pending_web_search_enabled(
            &processor,
            &prompt_mode,
            web_search_preference,
            &search_config,
        );

        // 发送 turn_pending 事件（前端立即显示用户消息 + loading）
        let pending_payload = TurnPendingPayload {
            turn_id: turn_id.clone(),
            user_instruction: user_instruction.clone(),
            selected_text: selected_text.clone(),
            has_selection: selected_text.is_some(),
        };
        if !set_conversation_pending(
            &state.assistant,
            &session_id,
            &turn_id,
            pending_payload.clone(),
            web_search_enabled,
        ) {
            tracing::warn!("AI 助手: 追问 pending 写入失败，会话已变化");
            state.assistant.processing.store(false, Ordering::SeqCst);
            return;
        }
        let _ = app.emit("assistant_turn_pending", pending_payload);

        // 隐藏 overlay
        hide_overlay_window(&app).await;

        // 更新统计
        if let Some(start_time) = recording_start_instant.lock().unwrap().take() {
            let recording_ms = start_time.elapsed().as_millis() as u64;
            let recognized_chars = user_instruction
                .chars()
                .filter(|c| !c.is_whitespace())
                .count() as u64;
            let mut stats = usage_stats.lock().unwrap();
            if let Err(e) = stats.update_and_save(recording_ms, recognized_chars) {
                tracing::error!("更新统计数据失败: {}", e);
            }
        }

        // 调用 LLM（追问模式）
        let _ = app.emit("post_processing", "assistant");
        let cancel_token = register_assistant_cancel_token(&state.assistant);
        let stream_app = app.clone();
        let stream_session_id = session_id.clone();
        let stream_turn_id = turn_id.clone();

        let result = processor
            .process_turn(
                &history,
                &user_instruction,
                selected_text.as_deref(),
                &prompt_mode,
                Some(search_config),
                web_search_preference,
                cancel_token.clone(),
                move |event| {
                    emit_and_record_assistant_stream_event(
                        &stream_app,
                        &stream_session_id,
                        &stream_turn_id,
                        event,
                    )
                },
            )
            .await;

        match result {
            Ok(outcome) => {
                let outcome = add_candidate_arbitration_time(outcome, candidate_llm_time_ms);
                let turn = turn_from_outcome(
                    user_instruction.clone(),
                    selected_text.clone(),
                    asr_time_ms,
                    outcome,
                );

                if !push_completed_turn_if_active(
                    &state.assistant,
                    &session_id,
                    &turn_id,
                    turn.clone(),
                ) {
                    tracing::warn!(
                        "AI 助手: 追问完成但会话已关闭、已取消或已被新请求替换，丢弃结果"
                    );
                    finish_assistant_turn_processing(&state.assistant, &turn_id, &cancel_token);
                    return;
                }

                // 发送 turn_complete 事件
                let payload = TurnCompletePayload {
                    session_id,
                    turn: to_turn_payload(&turn),
                    is_followup: true,
                };
                let _ = app.emit("assistant_turn_complete", payload);
                tracing::info!(
                    "AI 助手追问完成 (ASR: {}ms, LLM: {}ms)",
                    asr_time_ms,
                    turn.llm_time_ms
                );
            }
            Err(e) => {
                if cancel_token.is_cancelled() {
                    log_assistant_turn_cancelled("AI 助手追问", &e);
                } else {
                    // 发送 turn_error 事件（不写入 turns，用户可重试）
                    let error_payload = TurnErrorPayload {
                        session_id: session_id.clone(),
                        error_message: format!("{}", e),
                    };
                    mark_conversation_error(
                        &state.assistant,
                        &session_id,
                        &turn_id,
                        format!("{}", e),
                    );
                    let _ = app.emit("assistant_turn_error", error_payload);
                    tracing::error!("AI 助手追问失败: {}", e);
                }
            }
        }

        finish_assistant_turn_processing(&state.assistant, &turn_id, &cancel_token);
    } else {
        // =================== 新会话路径 ===================

        // 确定 PromptMode（首轮锁定）
        let prompt_mode = if selected_text.is_some() {
            PromptMode::TextProcessing
        } else {
            PromptMode::QA
        };

        // 隐藏 overlay
        hide_overlay_window(&app).await;

        // 更新统计
        if let Some(start_time) = recording_start_instant.lock().unwrap().take() {
            let recording_ms = start_time.elapsed().as_millis() as u64;
            let recognized_chars = user_instruction
                .chars()
                .filter(|c| !c.is_whitespace())
                .count() as u64;
            let mut stats = usage_stats.lock().unwrap();
            if let Err(e) = stats.update_and_save(recording_ms, recognized_chars) {
                tracing::error!("更新统计数据失败: {}", e);
            }
        }

        let session_id = uuid::Uuid::new_v4().to_string();

        // 创建空 session 并显示面板，让后续 streaming delta 有承载对象
        {
            let mut lock = state.assistant.conversation.lock().unwrap();
            // 安全清理：如果有旧会话未关闭，补发历史事件
            if let Some(old_session) = lock.take() {
                tracing::warn!(
                    "AI 助手: 新会话覆盖了旧会话 (id={}), 补发完成事件",
                    old_session.id
                );
                emit_conversation_history(&app, &old_session, false);
            }
            let session = ConversationSession {
                id: session_id.clone(),
                turns: Vec::new(),
                pending_turn: None,
                draft_turn_id: None,
                draft_assistant_response: String::new(),
                draft_tool_calls: Vec::new(),
                draft_status: "idle".to_string(),
                draft_warning: None,
                draft_web_search_enabled: None,
                system_prompt_mode: prompt_mode.clone(),
                target_hwnd,
                created_at: std::time::Instant::now(),
            };
            *lock = Some(session);
        }

        show_result_panel_window(&app).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let turn_id = new_assistant_turn_id();
        let web_search_preference = WebSearchPreference::UseConfig;
        let search_config = load_search_runtime_config();
        let web_search_enabled = resolve_pending_web_search_enabled(
            &processor,
            &prompt_mode,
            web_search_preference,
            &search_config,
        );
        let pending_payload = TurnPendingPayload {
            turn_id: turn_id.clone(),
            user_instruction: user_instruction.clone(),
            selected_text: selected_text.clone(),
            has_selection: selected_text.is_some(),
        };

        if state
            .assistant
            .processing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::warn!("AI 助手: 已有请求在处理中，忽略新会话触发");
            return;
        }

        if !set_conversation_pending(
            &state.assistant,
            &session_id,
            &turn_id,
            pending_payload.clone(),
            web_search_enabled,
        ) {
            tracing::warn!("AI 助手: 新会话 pending 写入失败，会话已变化");
            state.assistant.processing.store(false, Ordering::SeqCst);
            return;
        }
        let _ = app.emit("assistant_turn_pending", pending_payload);

        // 调用 LLM（首轮统一走 agentic process_turn）
        let _ = app.emit("post_processing", "assistant");
        let cancel_token = register_assistant_cancel_token(&state.assistant);
        let stream_app = app.clone();
        let stream_session_id = session_id.clone();
        let stream_turn_id = turn_id.clone();

        let result = processor
            .process_turn(
                &[],
                &user_instruction,
                selected_text.as_deref(),
                &prompt_mode,
                Some(search_config),
                web_search_preference,
                cancel_token.clone(),
                move |event| {
                    emit_and_record_assistant_stream_event(
                        &stream_app,
                        &stream_session_id,
                        &stream_turn_id,
                        event,
                    )
                },
            )
            .await;

        match result {
            Ok(outcome) => {
                let outcome = add_candidate_arbitration_time(outcome, candidate_llm_time_ms);
                let turn = turn_from_outcome(
                    user_instruction.clone(),
                    selected_text.clone(),
                    asr_time_ms,
                    outcome,
                );

                if !push_completed_turn_if_active(
                    &state.assistant,
                    &session_id,
                    &turn_id,
                    turn.clone(),
                ) {
                    tracing::warn!(
                        "AI 助手: 首轮完成但会话已关闭、已取消或已被新请求替换，丢弃结果"
                    );
                    finish_assistant_turn_processing(&state.assistant, &turn_id, &cancel_token);
                    return;
                }

                // 发送 turn_complete 事件
                let payload = TurnCompletePayload {
                    session_id: session_id.clone(),
                    turn: to_turn_payload(&turn),
                    is_followup: false,
                };
                let _ = app.emit("assistant_turn_complete", payload);

                tracing::info!(
                    "AI 助手新会话创建完成 (ASR: {}ms, LLM: {}ms)",
                    asr_time_ms,
                    turn.llm_time_ms
                );
            }
            Err(e) => {
                let _ = recording_start_instant.lock().unwrap().take();
                if cancel_token.is_cancelled() {
                    log_assistant_turn_cancelled("AI 助手处理", &e);
                } else {
                    tracing::error!("AI 助手处理失败: {}", e);
                    let error_payload = TurnErrorPayload {
                        session_id: session_id.clone(),
                        error_message: format!("{}", e),
                    };
                    mark_conversation_error(
                        &state.assistant,
                        &session_id,
                        &turn_id,
                        format!("{}", e),
                    );
                    let _ = app.emit("assistant_turn_error", error_payload);
                }
            }
        }
        finish_assistant_turn_processing(&state.assistant, &turn_id, &cancel_token);
    }
}

#[tauri::command]
pub(crate) async fn paste_latest_reply(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let session =
        { state.assistant.conversation.lock().unwrap().take() }.ok_or("无待处理的结果")?;

    let last_turn = session.turns.last().ok_or("会话中无回复")?;
    let result_text = last_turn.assistant_response.clone();
    let has_selection = session
        .turns
        .first()
        .map(|t| t.selected_text.is_some())
        .unwrap_or(false);

    // 检查目标窗口是否仍有效
    if let Some(hwnd) = session.target_hwnd {
        if platform::desktop().is_valid(hwnd) {
            // 先隐藏面板窗口，等窗口管理器处理完毕
            hide_result_panel_window(&app).await;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            // 恢复焦点到目标窗口
            if let Err(error) = platform::prepare_target(platform::desktop(), Some(hwnd)) {
                clipboard_manager::copy_to_clipboard(&result_text).map_err(|e| e.to_string())?;
                emit_conversation_history(&app, &session, false);
                return Ok(format!("{}；结果已复制到剪贴板", error));
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            // 粘贴文本
            if let Err(error) = clipboard_manager::insert_text_with_context(
                &result_text,
                has_selection,
                None,
                Some(hwnd),
            ) {
                clipboard_manager::copy_to_clipboard(&result_text).map_err(|e| e.to_string())?;
                emit_conversation_history(&app, &session, false);
                return Ok(format!("{}；结果已复制到剪贴板", error));
            }

            // 发送完成事件（粘贴 = 已插入）
            emit_conversation_history(&app, &session, true);

            return Ok("已粘贴".into());
        }
    }

    // 降级：目标窗口无效，复制到剪贴板
    clipboard_manager::copy_to_clipboard(&result_text)
        .map_err(|e| format!("复制到剪贴板失败: {}", e))?;
    hide_result_panel_window(&app).await;

    emit_conversation_history(&app, &session, false);

    Ok("原窗口已关闭，已复制到剪贴板".into())
}

/// 复制最新一轮 AI 回复到剪贴板（不结束会话）
#[tauri::command]
pub(crate) async fn copy_latest_reply(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let lock = state.assistant.conversation.lock().unwrap();
    if let Some(ref session) = *lock {
        if let Some(last_turn) = session.turns.last() {
            clipboard_manager::copy_to_clipboard(&last_turn.assistant_response)
                .map_err(|e| format!("复制到剪贴板失败: {}", e))?;
        }
    }
    Ok(())
}

/// 复制整个对话到剪贴板（Markdown 格式，不结束会话）
#[tauri::command]
pub(crate) async fn copy_full_conversation(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let lock = state.assistant.conversation.lock().unwrap();
    if let Some(ref session) = *lock {
        let formatted = assistant_processor::format_conversation_for_copy(&session.turns);
        clipboard_manager::copy_to_clipboard(&formatted)
            .map_err(|e| format!("复制到剪贴板失败: {}", e))?;
    }
    Ok(())
}

/// 获取当前会话完整状态（供前端 pull 模式使用）
///
/// 窗口从 hidden→visible 后，前端可能错过 push 事件。
/// 此命令让前端主动拉取最新会话状态。
#[tauri::command]
pub(crate) async fn get_conversation_state(
    state: tauri::State<'_, AppState>,
) -> Result<Option<ConversationStatePayload>, String> {
    let lock = state.assistant.conversation.lock().unwrap();
    let is_processing = state.assistant.processing.load(Ordering::SeqCst);
    Ok(lock.as_ref().map(|session| {
        let status = if is_processing && session.pending_turn.is_some() {
            "processing".to_string()
        } else {
            session.draft_status.clone()
        };
        ConversationStatePayload {
            session_id: session.id.clone(),
            turns: session.turns.iter().map(to_turn_payload).collect(),
            pending_turn: session.pending_turn.clone(),
            draft_assistant_response: session.draft_assistant_response.clone(),
            draft_tool_calls: session.draft_tool_calls.clone(),
            is_processing,
            status,
            warning_message: session.draft_warning.clone(),
            web_search_enabled: session.draft_web_search_enabled,
        }
    }))
}

/// 关闭结果面板并结束当前对话会话
///
/// 补发 `transcription_complete` 事件（inserted=false），
/// 确保历史记录能记录到这次 AI 助手交互。
#[tauri::command]
pub(crate) async fn dismiss_conversation(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if state.assistant.processing.load(Ordering::SeqCst) {
        if let Some(token) = state.assistant.cancel_token.lock().unwrap().take() {
            token.cancel();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while state.assistant.processing.load(Ordering::SeqCst)
            && std::time::Instant::now() < deadline
        {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        state.assistant.processing.store(false, Ordering::SeqCst);
    }
    if let Some(session) = state.assistant.conversation.lock().unwrap().take() {
        emit_conversation_history(&app, &session, false);
    }
    hide_result_panel_window(&app).await;
    Ok(())
}

/// 文本追问：接收用户键入的文本，跳过录音/ASR/TNL，直接调用 LLM 追问
///
/// 仅在面板已打开（有活跃会话）时可用。与语音追问共享 `is_assistant_processing`
/// 并发保护，同一时刻只能有一个在执行。
#[tauri::command]
pub(crate) async fn send_text_question(
    text: String,
    web_search_enabled: Option<bool>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("输入内容不能为空".into());
    }

    // 1. 读取会话状态（面板打开 = 有会话）
    let session_info = {
        let lock = state.assistant.conversation.lock().unwrap();
        lock.as_ref()
            .map(|s| (s.id.clone(), s.turns.clone(), s.system_prompt_mode.clone()))
    };
    let Some((session_id, history, prompt_mode)) = session_info else {
        return Err("当前没有活跃的对话会话".into());
    };

    // 2. 获取 processor
    let processor = { state.assistant.processor.lock().unwrap().clone() };
    let Some(processor) = processor else {
        return Err("AI 助手未配置".into());
    };

    // 3. 并发保护：CAS(false→true)
    if state
        .assistant
        .processing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("正在处理中，请稍候".into());
    }

    // 4. 注册取消令牌并发 pending 事件（前端立即显示用户消息 + loading）
    let turn_id = new_assistant_turn_id();
    let cancel_token = register_assistant_cancel_token(&state.assistant);
    let web_search_preference = match web_search_enabled {
        Some(true) => WebSearchPreference::Enabled,
        Some(false) => WebSearchPreference::Disabled,
        None => WebSearchPreference::UseConfig,
    };
    let search_config = load_search_runtime_config();
    let web_search_allowed = resolve_pending_web_search_enabled(
        &processor,
        &prompt_mode,
        web_search_preference,
        &search_config,
    );
    let pending_payload = TurnPendingPayload {
        turn_id: turn_id.clone(),
        user_instruction: text.clone(),
        selected_text: None,
        has_selection: false,
    };
    if !set_conversation_pending(
        &state.assistant,
        &session_id,
        &turn_id,
        pending_payload.clone(),
        web_search_allowed,
    ) {
        finish_assistant_turn_processing(&state.assistant, &turn_id, &cancel_token);
        return Err("当前对话会话已变化，请重试".into());
    }
    let _ = app.emit("assistant_turn_pending", pending_payload);

    tokio::spawn(run_text_question_task(
        app,
        session_id,
        turn_id,
        history,
        prompt_mode,
        text,
        processor,
        cancel_token,
        web_search_preference,
    ));

    Ok(())
}

pub(crate) async fn run_text_question_task(
    app: AppHandle,
    session_id: String,
    turn_id: String,
    history: Vec<ConversationTurn>,
    prompt_mode: PromptMode,
    text: String,
    processor: AssistantProcessor,
    cancel_token: CancellationToken,
    web_search_preference: WebSearchPreference,
) {
    let search_config = load_search_runtime_config();
    let stream_app = app.clone();
    let stream_session_id = session_id.clone();
    let stream_turn_id = turn_id.clone();

    let result = processor
        .process_turn(
            &history,
            &text,
            None,
            &prompt_mode,
            Some(search_config),
            web_search_preference,
            cancel_token.clone(),
            move |event| {
                emit_and_record_assistant_stream_event(
                    &stream_app,
                    &stream_session_id,
                    &stream_turn_id,
                    event,
                )
            },
        )
        .await;

    let state = app.state::<AppState>();
    match result {
        Ok(outcome) => {
            let turn = turn_from_outcome(text, None, 0, outcome);
            if push_completed_turn_if_active(&state.assistant, &session_id, &turn_id, turn.clone())
            {
                let payload = TurnCompletePayload {
                    session_id: session_id.clone(),
                    turn: to_turn_payload(&turn),
                    is_followup: true,
                };
                let _ = app.emit("assistant_turn_complete", payload);
                tracing::info!("AI 助手文本追问完成 (LLM: {}ms)", turn.llm_time_ms);
            } else {
                tracing::warn!(
                    "AI 助手: 文本追问完成但会话已关闭、已取消或已被新请求替换，丢弃结果"
                );
            }
        }
        Err(e) => {
            if cancel_token.is_cancelled() {
                log_assistant_turn_cancelled("AI 助手文本追问", &e);
            } else {
                let message = format!("{}", e);
                mark_conversation_error(&state.assistant, &session_id, &turn_id, message.clone());
                let error_payload = TurnErrorPayload {
                    session_id: session_id.clone(),
                    error_message: message,
                };
                let _ = app.emit("assistant_turn_error", error_payload);
                tracing::error!("AI 助手文本追问失败: {}", e);
            }
        }
    }

    finish_assistant_turn_processing(&state.assistant, &turn_id, &cancel_token);
}

#[tauri::command]
pub(crate) async fn cancel_assistant_generation(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if let Some(token) = state.assistant.cancel_token.lock().unwrap().take() {
        token.cancel();
    }
    state.assistant.processing.store(false, Ordering::SeqCst);

    let (session_id, turn_id, partial_content, tool_calls) =
        mark_active_conversation_cancelled(&state.assistant);

    let _ = app.emit(
        "assistant_turn_cancelled",
        serde_json::json!({
            "session_id": session_id,
            "turn_id": turn_id,
            "partial_content": partial_content,
            "tool_calls": tool_calls,
            "message": "已停止生成",
        }),
    );
    Ok(())
}

#[cfg(test)]
mod conversation_tests {
    use super::*;
    fn state() -> AssistantState {
        let state = AssistantState::default();
        *state.conversation.lock().unwrap() = Some(ConversationSession {
            id: "session".into(),
            turns: vec![],
            pending_turn: None,
            draft_turn_id: None,
            draft_assistant_response: String::new(),
            draft_tool_calls: vec![],
            draft_status: "idle".into(),
            draft_warning: None,
            draft_web_search_enabled: None,
            system_prompt_mode: PromptMode::QA,
            target_hwnd: None,
            created_at: std::time::Instant::now(),
        });
        state
    }
    fn begin(state: &AssistantState, turn: &str) {
        assert!(set_conversation_pending(
            state,
            "session",
            turn,
            TurnPendingPayload {
                turn_id: turn.into(),
                user_instruction: "question".into(),
                selected_text: None,
                has_selection: false,
            },
            false
        ));
        state.processing.store(true, Ordering::SeqCst);
    }
    #[test]
    fn late_stream_chunks_and_completion_do_not_change_a_new_turn() {
        let state = state();
        begin(&state, "old");
        let old_cancel = register_assistant_cancel_token(&state);
        begin(&state, "new");
        let new_cancel = register_assistant_cancel_token(&state);
        assert!(update_conversation_draft_from_event(
            &state,
            "session",
            "old",
            &AssistantStreamEvent::Delta {
                content_delta: "late".into()
            }
        )
        .is_none());
        finish_assistant_turn_processing(&state, "old", &old_cancel);
        assert!(state.processing.load(Ordering::SeqCst));
        assert!(!new_cancel.is_cancelled());
        assert!(state.cancel_token.lock().unwrap().is_some());
        assert!(state
            .conversation
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .draft_assistant_response
            .is_empty());
    }
    #[test]
    fn a_cancelled_turn_rejects_a_late_success() {
        let state = state();
        begin(&state, "turn");
        mark_active_conversation_cancelled(&state);
        assert!(!push_completed_turn_if_active(
            &state,
            "session",
            "turn",
            ConversationTurn {
                user_instruction: "question".into(),
                selected_text: None,
                assistant_response: "late".into(),
                asr_time_ms: 1,
                llm_time_ms: 2,
                search_time_ms: None,
                tool_calls: vec![],
            }
        ));
        let conversation = state.conversation.lock().unwrap();
        assert_eq!(conversation.as_ref().unwrap().draft_status, "cancelled");
        assert!(conversation.as_ref().unwrap().turns.is_empty());
    }
}
