// src-tauri/src/assistant_processor.rs
//
// AI 助手处理器
//
// 支持双系统提示词：问答模式和文本处理模式

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::config::{AssistantConfig, LlmFeatureConfig, SearchConfig, SharedLlmConfig};
use crate::llm_post_processor::LlmPostProcessor;
use crate::openai_client::{
    ChatOptions, Message, OpenAiClient, OpenAiClientConfig, StreamChunk, ToolCall, ToolDefinition,
    ToolFunctionDefinition,
};
use crate::search::{AssistantToolCall, SearchRegistry};
use crate::tnl::{TnlCandidateArbitrationResult, TnlDiagnostics};
use crate::{ConversationTurn, PromptMode};

/// AI 助手处理器
///
/// 根据是否有上下文（选中文本）使用不同的系统提示词
#[derive(Clone)]
pub struct AssistantProcessor {
    qa_client: OpenAiClient,
    text_processing_client: OpenAiClient,
    /// 问答模式系统提示词（无选中文本时使用）
    qa_system_prompt: String,
    /// 文本处理模式系统提示词（有选中文本时使用）
    text_processing_system_prompt: String,
    qa_options: ChatOptions,
    text_processing_options: ChatOptions,
    enable_web_search: bool,
    web_search_max_loops: u32,
    web_search_in_text_mode: bool,
}

#[derive(Debug, Clone)]
pub enum AssistantStreamEvent {
    Delta {
        content_delta: String,
    },
    ToolCallStarted {
        id: String,
        name: String,
        query: String,
        round: u32,
    },
    ToolCallFinished {
        call: AssistantToolCall,
    },
    Warning {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct TurnOutcome {
    pub assistant_response: String,
    pub tool_calls: Vec<AssistantToolCall>,
    pub llm_time_ms: u64,
    pub search_time_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchPreference {
    UseConfig,
    Enabled,
    Disabled,
}

impl AssistantProcessor {
    /// AI 助手模式请求超时（秒）
    ///
    /// 助手模式可能涉及复杂推理，需要比默认 30 秒更长的等待时间
    const ASSISTANT_TIMEOUT_SECS: u64 = 300;

    /// 创建新的 AI 助手处理器实例
    pub fn new(config: AssistantConfig, shared: &SharedLlmConfig) -> Self {
        let qa_resolved = config.resolve_qa_llm(shared);
        let qa_client = OpenAiClient::new(
            OpenAiClientConfig::new(
                &qa_resolved.endpoint,
                &qa_resolved.api_key,
                &qa_resolved.model,
            )
            .with_timeout_secs(Self::ASSISTANT_TIMEOUT_SECS),
        );
        let text_resolved = config.resolve_text_processing_llm(shared);
        let text_processing_client = OpenAiClient::new(
            OpenAiClientConfig::new(
                &text_resolved.endpoint,
                &text_resolved.api_key,
                &text_resolved.model,
            )
            .with_timeout_secs(Self::ASSISTANT_TIMEOUT_SECS),
        );
        let qa_options = feature_chat_options(&config.qa_feature_config());
        let text_processing_options =
            feature_chat_options(&config.text_processing_feature_config());

        Self {
            qa_client,
            text_processing_client,
            qa_system_prompt: config.qa_system_prompt,
            text_processing_system_prompt: config.text_processing_system_prompt,
            qa_options,
            text_processing_options,
            enable_web_search: config.enable_web_search,
            web_search_max_loops: config.web_search_max_loops,
            web_search_in_text_mode: config.web_search_in_text_mode,
        }
    }

    fn client_for_prompt_mode(&self, prompt_mode: &PromptMode) -> &OpenAiClient {
        match prompt_mode {
            PromptMode::QA => &self.qa_client,
            PromptMode::TextProcessing => &self.text_processing_client,
        }
    }

    fn options_for_prompt_mode(&self, prompt_mode: &PromptMode) -> ChatOptions {
        match prompt_mode {
            PromptMode::QA => self.qa_options.clone(),
            PromptMode::TextProcessing => self.text_processing_options.clone(),
        }
    }

    /// 处理用户指令（无上下文 - 问答模式）
    ///
    /// # Arguments
    /// * `user_input` - 用户的语音转写文本（问题/指令）
    ///
    /// # Returns
    /// * LLM 的回答
    pub async fn process(&self, user_input: &str) -> Result<String> {
        if user_input.trim().is_empty() {
            return Ok(String::new());
        }

        tracing::info!("AssistantProcessor: 问答模式处理指令: {}", user_input);

        self.qa_client
            .chat_simple(&self.qa_system_prompt, user_input, self.qa_options.clone())
            .await
    }

    /// 带上下文的指令处理（文本处理模式）
    ///
    /// # Arguments
    /// * `user_instruction` - 用户的语音指令
    /// * `selected_text` - 选中的文本
    ///
    /// # Returns
    /// * LLM 处理后的结果
    pub async fn process_with_context(
        &self,
        user_instruction: &str,
        selected_text: &str,
    ) -> Result<String> {
        if user_instruction.trim().is_empty() {
            return Ok(String::new());
        }

        tracing::info!(
            "AssistantProcessor: 文本处理模式 (指令: {}, 上下文长度: {} 字符)",
            user_instruction,
            selected_text.len()
        );

        // 构建包含上下文的用户消息
        let user_message = format!(
            "【选中的文本】\n{}\n\n【用户指令】\n{}",
            selected_text, user_instruction
        );

        self.text_processing_client
            .chat_simple(
                &self.text_processing_system_prompt,
                &user_message,
                self.text_processing_options.clone(),
            )
            .await
    }

    /// 对 AI 助手语音指令中的 TNL/个性化中置信候选执行轻量 LLM 仲裁。
    pub async fn arbitrate_tnl_candidates(
        &self,
        text: &str,
        diagnostics: TnlDiagnostics,
    ) -> Result<TnlCandidateArbitrationResult> {
        LlmPostProcessor::arbitrate_tnl_candidates_with_client(
            &self.text_processing_client,
            text,
            diagnostics,
        )
        .await
    }

    /// 多轮对话追问处理
    ///
    /// 基于历史对话上下文处理新的用户指令。
    /// system_prompt 由首轮锁定的 PromptMode 决定，追问不改变。
    ///
    /// # Arguments
    /// * `history` - 历史对话轮次
    /// * `new_instruction` - 新的用户语音指令（已经过 TNL 规范化）
    /// * `new_selected_text` - 追问时新选中的文本（可选）
    /// * `prompt_mode` - 首轮锁定的提示词模式
    ///
    /// # Returns
    /// * LLM 的回答
    #[allow(dead_code)]
    #[deprecated(note = "统一入口已迁移到 process_turn；保留给旧调用链回退使用")]
    pub async fn process_followup(
        &self,
        history: &[ConversationTurn],
        new_instruction: &str,
        new_selected_text: Option<&str>,
        prompt_mode: &PromptMode,
    ) -> Result<String> {
        let system_prompt = match prompt_mode {
            PromptMode::QA => &self.qa_system_prompt,
            PromptMode::TextProcessing => &self.text_processing_system_prompt,
        };

        let messages =
            build_followup_messages(system_prompt, history, new_instruction, new_selected_text);

        tracing::info!(
            "AssistantProcessor: 追问模式 (历史轮次: {}, 消息数: {}, 模式: {:?})",
            history.len(),
            messages.len(),
            prompt_mode,
        );

        self.client_for_prompt_mode(prompt_mode)
            .chat(&messages, self.options_for_prompt_mode(prompt_mode))
            .await
    }

    pub async fn process_turn<F>(
        &self,
        history: &[ConversationTurn],
        new_instruction: &str,
        new_selected_text: Option<&str>,
        prompt_mode: &PromptMode,
        search_config: Option<SearchConfig>,
        web_search_preference: WebSearchPreference,
        cancel_token: CancellationToken,
        mut emit: F,
    ) -> Result<TurnOutcome>
    where
        F: FnMut(AssistantStreamEvent) + Send,
    {
        if new_instruction.trim().is_empty() {
            return Ok(TurnOutcome {
                assistant_response: String::new(),
                tool_calls: Vec::new(),
                llm_time_ms: 0,
                search_time_ms: None,
            });
        }

        // 注意：本入口仅服务 AI 助手模式。结果显示在结果面板，不调用 text_inserter，
        // 因此不触发 learning 模块。如需扩展到听写模式，请考虑 learning observation 触发时机。
        let (tools_enabled, unavailable_reason) =
            self.resolve_tools_availability(prompt_mode, web_search_preference, &search_config);
        if let Some(message) = unavailable_reason {
            emit(AssistantStreamEvent::Warning { message });
        }
        let system_prompt = match prompt_mode {
            PromptMode::QA => &self.qa_system_prompt,
            PromptMode::TextProcessing => &self.text_processing_system_prompt,
        };
        let system_prompt = if tools_enabled {
            format!("{system_prompt}\n\n{}", web_search_system_prompt())
        } else {
            system_prompt.to_string()
        };
        let mut messages =
            build_turn_messages(&system_prompt, history, new_instruction, new_selected_text);
        let tools = tools_enabled.then(|| vec![search_web_tool_definition()]);
        let default_max_results = search_config
            .as_ref()
            .map(|cfg| cfg.max_results)
            .unwrap_or(5);
        let search_timeout_secs = search_config
            .as_ref()
            .map(|cfg| cfg.timeout_secs)
            .unwrap_or(6);
        let registry = search_config.map(SearchRegistry::from_config);
        let max_loops = self.web_search_max_loops.clamp(1, 3);
        let mut loop_round = 0_u32;
        let mut final_content = String::new();
        let mut all_tool_calls = Vec::new();
        let mut total_search_ms = 0_u64;
        let llm_started = Instant::now();

        if tools_enabled && should_presearch_for_realtime_question(new_instruction) {
            let tool_call = presearch_tool_call(new_instruction);
            messages.push(Message::assistant_with_tool_calls(
                String::new(),
                vec![tool_call.clone()],
            ));
            let execution = execute_tool_call(
                &tool_call,
                registry.as_ref(),
                1,
                default_max_results,
                search_timeout_secs,
                cancel_token.clone(),
                &mut emit,
            )
            .await?;

            total_search_ms += execution.elapsed_ms;
            let tool_content = tool_result_content(&execution);
            messages.push(Message::tool(
                tool_call.id.clone(),
                tool_call.function.name.clone(),
                tool_content,
            ));
            all_tool_calls.push(execution);
            loop_round = 1;
        }

        loop {
            loop_round += 1;
            let mut streamed_content = String::new();
            let response = self
                .client_for_prompt_mode(prompt_mode)
                .chat_stream(
                    &messages,
                    self.options_for_prompt_mode(prompt_mode),
                    tools.clone(),
                    cancel_token.clone(),
                    |chunk: StreamChunk| {
                        if let Some(delta) = chunk.delta_content.clone() {
                            streamed_content.push_str(&delta);
                            emit(AssistantStreamEvent::Delta {
                                content_delta: delta,
                            });
                        }
                    },
                )
                .await?;

            if !response.content.is_empty() && streamed_content != response.content {
                let missing = response
                    .content
                    .strip_prefix(&streamed_content)
                    .unwrap_or("");
                if !missing.is_empty() {
                    emit(AssistantStreamEvent::Delta {
                        content_delta: missing.to_string(),
                    });
                }
            }

            final_content.push_str(&response.content);

            if response.tool_calls.is_empty()
                || response.finish_reason.as_deref() != Some("tool_calls")
            {
                return Ok(TurnOutcome {
                    assistant_response: final_content.trim().to_string(),
                    tool_calls: all_tool_calls,
                    llm_time_ms: llm_started.elapsed().as_millis() as u64,
                    search_time_ms: (total_search_ms > 0).then_some(total_search_ms),
                });
            }

            if loop_round >= max_loops {
                emit(AssistantStreamEvent::Warning {
                    message: "联网搜索工具调用已达到最大轮数，正在基于已有结果生成最终回答"
                        .to_string(),
                });
                messages.push(Message::user(search_loop_limit_final_answer_prompt(
                    max_loops,
                )));

                let mut streamed_content = String::new();
                let response = self
                    .client_for_prompt_mode(prompt_mode)
                    .chat_stream(
                        &messages,
                        self.options_for_prompt_mode(prompt_mode),
                        None,
                        cancel_token.clone(),
                        |chunk: StreamChunk| {
                            if let Some(delta) = chunk.delta_content.clone() {
                                streamed_content.push_str(&delta);
                                emit(AssistantStreamEvent::Delta {
                                    content_delta: delta,
                                });
                            }
                        },
                    )
                    .await?;

                if !response.content.is_empty() && streamed_content != response.content {
                    let missing = response
                        .content
                        .strip_prefix(&streamed_content)
                        .unwrap_or("");
                    if !missing.is_empty() {
                        emit(AssistantStreamEvent::Delta {
                            content_delta: missing.to_string(),
                        });
                    }
                }
                final_content.push_str(&response.content);

                return Ok(TurnOutcome {
                    assistant_response: final_content.trim().to_string(),
                    tool_calls: all_tool_calls,
                    llm_time_ms: llm_started.elapsed().as_millis() as u64,
                    search_time_ms: (total_search_ms > 0).then_some(total_search_ms),
                });
            }

            messages.push(Message::assistant_with_tool_calls(
                response.content,
                response.tool_calls.clone(),
            ));

            for tool_call in response.tool_calls {
                let execution = execute_tool_call(
                    &tool_call,
                    registry.as_ref(),
                    loop_round,
                    default_max_results,
                    search_timeout_secs,
                    cancel_token.clone(),
                    &mut emit,
                )
                .await?;

                total_search_ms += execution.elapsed_ms;
                let tool_content = tool_result_content(&execution);
                messages.push(Message::tool(
                    tool_call.id.clone(),
                    tool_call.function.name.clone(),
                    tool_content,
                ));
                all_tool_calls.push(execution);
            }
        }
    }

    fn resolve_tools_availability(
        &self,
        prompt_mode: &PromptMode,
        web_search_preference: WebSearchPreference,
        search_config: &Option<SearchConfig>,
    ) -> (bool, Option<String>) {
        if !self.is_web_search_requested(prompt_mode, web_search_preference, search_config.as_ref())
        {
            return (false, None);
        }
        let Some(config) = search_config.as_ref() else {
            return (
                false,
                Some("联网搜索配置读取失败，已改为普通回答".to_string()),
            );
        };
        if let Some(reason) = SearchRegistry::runtime_unavailable_reason(config) {
            return (false, Some(reason));
        }
        (true, None)
    }

    pub fn is_web_search_requested(
        &self,
        prompt_mode: &PromptMode,
        web_search_preference: WebSearchPreference,
        search_config: Option<&SearchConfig>,
    ) -> bool {
        let requested = match web_search_preference {
            WebSearchPreference::UseConfig => {
                self.enable_web_search
                    || search_config
                        .map(SearchRegistry::has_usable_provider)
                        .unwrap_or(false)
            }
            WebSearchPreference::Enabled => true,
            WebSearchPreference::Disabled => false,
        };

        if !requested {
            return false;
        }

        if matches!(prompt_mode, PromptMode::TextProcessing)
            && !self.web_search_in_text_mode
            && web_search_preference != WebSearchPreference::Enabled
        {
            return false;
        }

        true
    }
}

fn feature_chat_options(config: &LlmFeatureConfig) -> ChatOptions {
    let mut options = ChatOptions::for_smart_command();
    options.reasoning = config.reasoning.clone();
    options.custom_body = config.custom_body.clone();
    options
}

// ================== 多轮对话纯函数 ==================

/// 多轮对话最大轮次（LLM 发送时的滑动窗口大小）
const MAX_CONVERSATION_TURNS: usize = 20;

/// 将用户指令和选中文本组合为 user message 内容
///
/// - 有选中文本: `"【选中的文本】\n{selected}\n\n【用户指令】\n{instruction}"`
/// - 无选中文本: 直接返回 instruction
pub(crate) fn format_user_content(instruction: &str, selected_text: Option<&str>) -> String {
    match selected_text {
        Some(text) if !text.is_empty() => {
            format!("【选中的文本】\n{}\n\n【用户指令】\n{}", text, instruction)
        }
        _ => instruction.to_string(),
    }
}

/// 构建多轮对话的 LLM messages 数组
///
/// 结构: `[system, user₁, assistant₁, user₂, assistant₂, ..., userₙ]`
///
/// 当 history 超过 `MAX_CONVERSATION_TURNS` 时，只发送最近 N 轮（滑动窗口）。
#[allow(dead_code)]
pub(crate) fn build_followup_messages(
    system_prompt: &str,
    history: &[ConversationTurn],
    new_instruction: &str,
    new_selected_text: Option<&str>,
) -> Vec<Message> {
    build_turn_messages(system_prompt, history, new_instruction, new_selected_text)
}

pub(crate) fn build_turn_messages(
    system_prompt: &str,
    history: &[ConversationTurn],
    new_instruction: &str,
    new_selected_text: Option<&str>,
) -> Vec<Message> {
    let mut messages = Vec::new();

    // 1. system prompt
    messages.push(Message::system(system_prompt));

    // 2. 历史轮次（滑动窗口截断）
    let window_start = if history.len() > MAX_CONVERSATION_TURNS {
        history.len() - MAX_CONVERSATION_TURNS
    } else {
        0
    };

    for turn in &history[window_start..] {
        messages.push(Message::user(format_user_content(
            &turn.user_instruction,
            turn.selected_text.as_deref(),
        )));
        append_historical_assistant_messages(&mut messages, turn);
    }

    // 3. 本次追问
    messages.push(Message::user(format_user_content(
        new_instruction,
        new_selected_text,
    )));

    messages
}

fn append_historical_assistant_messages(messages: &mut Vec<Message>, turn: &ConversationTurn) {
    if turn.tool_calls.is_empty() {
        messages.push(Message::assistant(&turn.assistant_response));
        return;
    }

    let tool_calls: Vec<ToolCall> = turn
        .tool_calls
        .iter()
        .map(|call| ToolCall {
            id: call.id.clone(),
            call_type: "function".to_string(),
            function: crate::openai_client::ToolFunctionCall {
                name: call.name.clone(),
                arguments: serde_json::json!({
                    "query": call.query,
                })
                .to_string(),
            },
        })
        .collect();

    messages.push(Message::assistant_with_tool_calls("", tool_calls));
    for call in &turn.tool_calls {
        messages.push(Message::tool(
            call.id.clone(),
            call.name.clone(),
            compact_historical_tool_content(call),
        ));
    }
    messages.push(Message::assistant(&turn.assistant_response));
}

fn compact_historical_tool_content(call: &AssistantToolCall) -> String {
    if call.status == "success" {
        let items: Vec<Value> = call
            .results
            .iter()
            .map(|item| {
                serde_json::json!({
                    "index": item.index,
                    "id": item.id,
                    "title": item.title,
                    "url": item.url,
                })
            })
            .collect();
        serde_json::json!({
            "type": "search_result",
            "query": call.query,
            "items": items,
        })
        .to_string()
    } else {
        serde_json::json!({
            "type": "tool_error",
            "query": call.query,
            "error": call.error.clone().unwrap_or_else(|| "搜索失败".to_string()),
        })
        .to_string()
    }
}

fn web_search_system_prompt() -> &'static str {
    r#"联网搜索可用时，凡是用户问题需要今天、最新、实时、天气、新闻、价格、文档更新等外部信息，必须先调用 search_web 工具，不要直接回答“无法提供实时信息”。
如果当前消息历史中已经有 search_web 工具结果，请优先基于结果回答，不要重复搜索同一问题。
搜索结果每条都有 index 和 id。引用搜索结果时，请在对应句子后紧邻标注 [citation](index:id)，例如 [citation](1:abc123def456)。
不要编造引用；只有来自工具结果的内容才标注引用。"#
}

fn search_loop_limit_final_answer_prompt(max_loops: u32) -> String {
    format!(
        "联网搜索已经达到本轮上限（{max_loops} 轮）。不要再调用 search_web 工具；请只基于已有搜索结果和对话上下文给出最终回答。如果已有结果不足，请明确说明还缺哪些信息。"
    )
}

fn should_presearch_for_realtime_question(instruction: &str) -> bool {
    let trimmed = instruction.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    const DIRECT_REALTIME_KEYWORDS: &[&str] = &[
        "实时",
        "最新",
        "天气",
        "下雨",
        "降雨",
        "气温",
        "温度",
        "预报",
        "空气质量",
        "新闻",
        "价格",
        "股价",
        "汇率",
        "更新",
        "latest",
        "weather",
        "rain",
        "rainfall",
        "temperature",
        "forecast",
        "air quality",
        "aqi",
        "news",
        "price",
        "exchange rate",
    ];
    if DIRECT_REALTIME_KEYWORDS
        .iter()
        .any(|keyword| lower.contains(keyword))
    {
        return true;
    }

    const TIME_KEYWORDS: &[&str] = &[
        "今天", "今日", "现在", "当前", "目前", "最近", "刚刚", "昨天", "明天", "本周", "today",
        "current", "recent",
    ];
    const QUERY_INTENT_KEYWORDS: &[&str] = &[
        "怎么样",
        "多少",
        "什么",
        "哪里",
        "哪",
        "查询",
        "搜索",
        "查一下",
        "查",
        "有吗",
        "吗",
        "发布",
        "？",
        "?",
        "how",
        "what",
        "when",
        "where",
    ];

    TIME_KEYWORDS.iter().any(|keyword| lower.contains(keyword))
        && QUERY_INTENT_KEYWORDS
            .iter()
            .any(|keyword| lower.contains(keyword))
}

fn presearch_tool_call(query: &str) -> ToolCall {
    ToolCall {
        id: format!("presearch_{}", uuid::Uuid::new_v4().simple()),
        call_type: "function".to_string(),
        function: crate::openai_client::ToolFunctionCall {
            name: "search_web".to_string(),
            arguments: serde_json::json!({
                "query": query.trim(),
            })
            .to_string(),
        },
    }
}

fn search_web_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: ToolFunctionDefinition {
            name: "search_web".to_string(),
            description: "联网搜索实时信息，返回带 index/id/title/url/snippet 的引用来源。"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "要搜索的关键词或问题，应由模型根据用户意图生成。"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "返回结果数量，默认使用应用配置。",
                        "minimum": 1,
                        "maximum": 10
                    }
                },
                "required": ["query"]
            }),
        },
    }
}

async fn execute_tool_call<F>(
    tool_call: &ToolCall,
    registry: Option<&SearchRegistry>,
    round: u32,
    default_max_results: u32,
    timeout_secs: u32,
    cancel_token: CancellationToken,
    emit: &mut F,
) -> Result<AssistantToolCall>
where
    F: FnMut(AssistantStreamEvent) + Send,
{
    let started = Instant::now();
    let name = tool_call.function.name.clone();
    let (query, requested_max_results) = parse_search_arguments(&tool_call.function.arguments);
    let query = query.unwrap_or_else(|| "".to_string());

    emit(AssistantStreamEvent::ToolCallStarted {
        id: tool_call.id.clone(),
        name: name.clone(),
        query: query.clone(),
        round,
    });

    let mut record = AssistantToolCall {
        id: tool_call.id.clone(),
        name: name.clone(),
        query: query.clone(),
        status: "error".to_string(),
        results: Vec::new(),
        error: None,
        elapsed_ms: 0,
        round,
    };

    if name != "search_web" {
        record.error = Some(format!("未知工具: {name}"));
        record.elapsed_ms = started.elapsed().as_millis() as u64;
        emit(AssistantStreamEvent::ToolCallFinished {
            call: record.clone(),
        });
        return Ok(record);
    }

    let Some(registry) = registry else {
        record.error = Some("联网搜索未配置可用引擎".to_string());
        record.elapsed_ms = started.elapsed().as_millis() as u64;
        emit(AssistantStreamEvent::ToolCallFinished {
            call: record.clone(),
        });
        return Ok(record);
    };

    if registry.is_empty() {
        record.error = Some("联网搜索未配置可用引擎".to_string());
        record.elapsed_ms = started.elapsed().as_millis() as u64;
        emit(AssistantStreamEvent::ToolCallFinished {
            call: record.clone(),
        });
        return Ok(record);
    }

    if query.trim().is_empty() {
        record.error = Some("搜索 query 不能为空".to_string());
        record.elapsed_ms = started.elapsed().as_millis() as u64;
        emit(AssistantStreamEvent::ToolCallFinished {
            call: record.clone(),
        });
        return Ok(record);
    }

    let max_results = requested_max_results
        .unwrap_or(default_max_results)
        .clamp(1, 10);
    let search = tokio::select! {
        _ = cancel_token.cancelled() => return Err(anyhow::anyhow!("AI 助手生成已取消")),
        result = registry.search(&query, max_results, timeout_secs) => result,
    };

    match search {
        Ok(result) => {
            record.status = "success".to_string();
            record.results = result.items;
            record.elapsed_ms = result.elapsed_ms.max(started.elapsed().as_millis() as u64);
        }
        Err(err) => {
            record.error = Some(err.to_string());
            record.elapsed_ms = started.elapsed().as_millis() as u64;
        }
    }

    emit(AssistantStreamEvent::ToolCallFinished {
        call: record.clone(),
    });
    Ok(record)
}

fn parse_search_arguments(arguments: &str) -> (Option<String>, Option<u32>) {
    let Ok(value) = serde_json::from_str::<Value>(arguments) else {
        return (None, None);
    };
    let query = value["query"].as_str().map(ToString::to_string);
    let max_results = value["max_results"]
        .as_u64()
        .map(|v| (v as u32).clamp(1, 10));
    (query, max_results)
}

fn tool_result_content(call: &AssistantToolCall) -> String {
    if call.status == "success" {
        serde_json::json!({
            "type": "search_result",
            "query": call.query,
            "items": call.results,
        })
        .to_string()
    } else {
        serde_json::json!({
            "type": "tool_error",
            "query": call.query,
            "error": call.error.clone().unwrap_or_else(|| "搜索失败".to_string()),
        })
        .to_string()
    }
}

/// 将对话历史格式化为 Markdown 用于复制
///
/// 格式:
/// ```text
/// **问**: 用户指令
/// > 选中文本: ...（如有）
///
/// **答**: AI 回复
///
/// ---
///
/// **问**: 追问指令
///
/// **答**: AI 回复
/// ```
pub(crate) fn format_conversation_for_copy(turns: &[ConversationTurn]) -> String {
    format_conversation_for_copy_with_citation_remap(turns, &HashMap::new())
}

pub(crate) fn format_conversation_for_copy_with_citation_remap(
    turns: &[ConversationTurn],
    citation_remap: &HashMap<String, u32>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    for (turn_idx, turn) in turns.iter().enumerate() {
        let mut section = format!("**问**: {}\n", turn.user_instruction);

        if let Some(ref text) = turn.selected_text {
            if !text.is_empty() {
                section.push_str(&format!("> 选中文本: {}\n", text));
            }
        }

        let response = remap_citations_in_text(&turn.assistant_response, turn_idx, citation_remap);
        section.push_str(&format!("\n**答**: {}", response));

        parts.push(section);
    }

    parts.join("\n\n---\n\n")
}

fn citation_remap_key(turn_idx: usize, id: &str) -> String {
    format!("{turn_idx}:{id}")
}

fn remap_citations_in_text(
    text: &str,
    turn_idx: usize,
    citation_remap: &HashMap<String, u32>,
) -> String {
    if citation_remap.is_empty() || !text.contains("[citation](") {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    const PREFIX: &str = "[citation](";

    while let Some(start) = rest.find(PREFIX) {
        output.push_str(&rest[..start]);
        let marker_start = start + PREFIX.len();
        let after_prefix = &rest[marker_start..];
        let Some(end) = after_prefix.find(')') else {
            output.push_str(&rest[start..]);
            return output;
        };

        let marker = &after_prefix[..end];
        let original = &rest[start..marker_start + end + 1];
        let mut parts = marker.splitn(2, ':');
        let index = parts.next().unwrap_or_default();
        let id = parts.next().unwrap_or_default();

        if !index.is_empty()
            && !id.is_empty()
            && index.chars().all(|c| c.is_ascii_digit())
            && id.chars().all(|c| c.is_ascii_hexdigit())
        {
            if let Some(new_index) = citation_remap.get(&citation_remap_key(turn_idx, id)) {
                output.push_str(&format!("[citation]({new_index}:{id})"));
            } else {
                output.push_str(original);
            }
        } else {
            output.push_str(original);
        }

        rest = &after_prefix[end + 1..];
    }

    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        LlmFeatureConfig, SearchConfig, SearchProviderConfig, SearchProviderType, SharedLlmConfig,
        DEFAULT_ASSISTANT_QA_PROMPT, DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT,
    };
    use crate::openai_client::Role;
    use crate::search::{AssistantToolCall, SearchResultItem};
    use crate::ConversationTurn;

    fn create_test_config() -> AssistantConfig {
        AssistantConfig {
            enabled: true,
            llm: LlmFeatureConfig {
                use_shared: false,
                provider_id: None,
                endpoint: Some("https://api.example.com/v1/chat/completions".to_string()),
                model: Some("test-model".to_string()),
                api_key: Some("test-key".to_string()),
                reasoning: None,
                custom_body: None,
            },
            qa_llm: None,
            text_processing_llm: None,
            qa_system_prompt: DEFAULT_ASSISTANT_QA_PROMPT.to_string(),
            text_processing_system_prompt: DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT.to_string(),
            enable_web_search: false,
            web_search_max_loops: 3,
            web_search_in_text_mode: false,
        }
    }

    #[test]
    fn test_processor_creation() {
        let config = create_test_config();
        let shared = SharedLlmConfig::default();
        let processor = AssistantProcessor::new(config, &shared);
        assert!(!processor.qa_system_prompt.is_empty());
        assert!(!processor.text_processing_system_prompt.is_empty());
    }

    // === 多轮消息构建测试 ===

    fn make_turn(instruction: &str, selected: Option<&str>, response: &str) -> ConversationTurn {
        ConversationTurn {
            user_instruction: instruction.to_string(),
            selected_text: selected.map(|s| s.to_string()),
            assistant_response: response.to_string(),
            asr_time_ms: 100,
            llm_time_ms: 200,
            search_time_ms: None,
            tool_calls: Vec::new(),
        }
    }

    fn make_search_turn(instruction: &str, response: &str) -> ConversationTurn {
        ConversationTurn {
            user_instruction: instruction.to_string(),
            selected_text: None,
            assistant_response: response.to_string(),
            asr_time_ms: 100,
            llm_time_ms: 200,
            search_time_ms: Some(300),
            tool_calls: vec![AssistantToolCall {
                id: "call_abc".to_string(),
                name: "search_web".to_string(),
                query: "OpenAI news".to_string(),
                status: "success".to_string(),
                results: vec![SearchResultItem {
                    index: 1,
                    id: "abc123def456".to_string(),
                    title: "OpenAI News".to_string(),
                    url: "https://example.com/openai".to_string(),
                    snippet: "full snippet should not be copied into historical tool context"
                        .to_string(),
                    source: Some("example.com".to_string()),
                }],
                error: None,
                elapsed_ms: 300,
                round: 1,
            }],
        }
    }

    fn usable_search_config() -> SearchConfig {
        SearchConfig {
            providers: vec![SearchProviderConfig {
                id: "default".to_string(),
                provider_type: SearchProviderType::Tavily,
                display_name: "Tavily".to_string(),
                enabled: true,
                endpoint: None,
                api_key: Some("search-key".to_string()),
                basic_auth_username: None,
                basic_auth_password: None,
                serper_gl: None,
                serper_hl: None,
                serper_tbs: None,
                searxng_language: None,
                searxng_time_range: None,
            }],
            default_provider_id: Some("default".to_string()),
            max_results: 5,
            timeout_secs: 6,
            enable_fallback: true,
        }
    }

    #[test]
    fn explicit_turn_web_search_can_enable_tools_even_when_default_off() {
        let processor = AssistantProcessor::new(create_test_config(), &SharedLlmConfig::default());
        let search_config = Some(usable_search_config());

        let (enabled, reason) = processor.resolve_tools_availability(
            &PromptMode::QA,
            WebSearchPreference::Enabled,
            &search_config,
        );

        assert!(enabled);
        assert!(reason.is_none());
    }

    #[test]
    fn use_config_web_search_defaults_on_when_search_api_is_usable() {
        let processor = AssistantProcessor::new(create_test_config(), &SharedLlmConfig::default());
        let search_config = Some(usable_search_config());

        let (enabled, reason) = processor.resolve_tools_availability(
            &PromptMode::QA,
            WebSearchPreference::UseConfig,
            &search_config,
        );

        assert!(enabled);
        assert!(reason.is_none());
    }

    #[test]
    fn explicit_turn_web_search_disabled_blocks_tools_even_when_default_on() {
        let mut config = create_test_config();
        config.enable_web_search = true;
        let processor = AssistantProcessor::new(config, &SharedLlmConfig::default());
        let search_config = Some(usable_search_config());

        let (enabled, reason) = processor.resolve_tools_availability(
            &PromptMode::QA,
            WebSearchPreference::Disabled,
            &search_config,
        );

        assert!(!enabled);
        assert!(reason.is_none());
    }

    #[test]
    fn realtime_questions_request_presearch_when_tools_are_enabled() {
        assert!(should_presearch_for_realtime_question("今天白天天气怎么样"));
        assert!(should_presearch_for_realtime_question("明天上海会下雨吗"));
        assert!(should_presearch_for_realtime_question("北京当前空气质量"));
        assert!(should_presearch_for_realtime_question(
            "现在美元人民币汇率是多少"
        ));
        assert!(should_presearch_for_realtime_question(
            "最近 OpenAI 有什么新闻"
        ));
    }

    #[test]
    fn static_questions_do_not_request_presearch() {
        assert!(!should_presearch_for_realtime_question(
            "帮我解释一下 Rust ownership"
        ));
        assert!(!should_presearch_for_realtime_question(
            "把这段话改得更正式"
        ));
        assert!(!should_presearch_for_realtime_question(
            "今天帮我写一段日报开头"
        ));
    }

    #[test]
    fn search_loop_limit_prompt_forces_final_answer_without_more_tools() {
        let prompt = search_loop_limit_final_answer_prompt(2);

        assert!(prompt.contains("2 轮"));
        assert!(prompt.contains("不要再调用 search_web 工具"));
        assert!(prompt.contains("给出最终回答"));
    }

    #[test]
    fn test_build_followup_messages_basic() {
        // 1 轮历史（QA 模式，无选中文本）+ 追问 1 条纯语音
        let history = vec![make_turn("你好", None, "你好！有什么可以帮你？")];
        let system_prompt = "你是一个助手";

        let messages = build_followup_messages(system_prompt, &history, "今天天气怎么样", None);

        // [system, user₁, assistant₁, user₂] = 4 条
        assert_eq!(messages.len(), 4);
        assert!(matches!(messages[0].role, Role::System));
        assert_eq!(messages[0].content, "你是一个助手");
        assert!(matches!(messages[1].role, Role::User));
        assert!(messages[1].content.contains("你好"));
        assert!(matches!(messages[2].role, Role::Assistant));
        assert!(messages[2].content.contains("你好！有什么可以帮你？"));
        assert!(matches!(messages[3].role, Role::User));
        assert!(messages[3].content.contains("今天天气怎么样"));
    }

    #[test]
    fn test_build_followup_messages_with_selected_text() {
        // 1 轮历史 + 追问时带有新选中文本
        let history = vec![make_turn("翻译这段话", Some("Hello world"), "你好世界")];
        let system_prompt = "你是一个文本处理助手";

        let messages = build_followup_messages(
            system_prompt,
            &history,
            "改成正式语气",
            Some("这是新选中的文本"),
        );

        // [system, user₁, assistant₁, user₂] = 4 条
        assert_eq!(messages.len(), 4);
        // 历史 user₁ 应包含选中文本
        assert!(messages[1].content.contains("Hello world"));
        assert!(messages[1].content.contains("翻译这段话"));
        // 新追问 user₂ 应包含新选中文本
        assert!(messages[3].content.contains("这是新选中的文本"));
        assert!(messages[3].content.contains("改成正式语气"));
        assert!(messages[3].content.contains("【选中的文本】"));
        assert!(messages[3].content.contains("【用户指令】"));
    }

    #[test]
    fn test_build_followup_messages_sliding_window() {
        // 25 轮历史（超过 MAX_CONVERSATION_TURNS=20）
        let history: Vec<ConversationTurn> = (0..25)
            .map(|i| make_turn(&format!("问题{}", i), None, &format!("回答{}", i)))
            .collect();
        let system_prompt = "系统提示";

        let messages = build_followup_messages(system_prompt, &history, "最新问题", None);

        // 1 (system) + 20*2 (user+assistant) + 1 (new user) = 42
        assert_eq!(messages.len(), 42);
        // 第一条是 system
        assert!(matches!(messages[0].role, Role::System));
        // 应该跳过前 5 轮，从第 5 轮开始
        assert!(messages[1].content.contains("问题5"));
        // 最后一条是新问题
        assert!(messages[41].content.contains("最新问题"));
    }

    #[test]
    fn test_build_followup_messages_text_processing_mode() {
        // TextProcessing 模式的 system prompt 选择验证
        let history = vec![make_turn("润色这段话", Some("原始文本"), "润色后的文本")];
        let tp_prompt = "你是一个文本处理专家";

        let messages = build_followup_messages(tp_prompt, &history, "再简洁一些", None);

        // system prompt 使用传入的 text_processing prompt
        assert_eq!(messages[0].content, "你是一个文本处理专家");
    }

    #[test]
    fn test_build_turn_messages_preserves_historical_tool_context_compactly() {
        let history = vec![make_search_turn(
            "今天 OpenAI 有什么新闻",
            "OpenAI 发布了新闻 [citation](1:abc123def456)",
        )];

        let messages = build_turn_messages("系统提示", &history, "价格呢", None);

        assert_eq!(messages.len(), 6);
        assert!(matches!(messages[1].role, Role::User));
        assert!(matches!(messages[2].role, Role::Assistant));
        assert!(messages[2]
            .tool_calls
            .as_ref()
            .is_some_and(|calls| calls.len() == 1));
        assert!(matches!(messages[3].role, Role::Tool));
        assert!(messages[3].content.contains("\"title\":\"OpenAI News\""));
        assert!(messages[3]
            .content
            .contains("\"url\":\"https://example.com/openai\""));
        assert!(messages[3].content.contains("\"id\":\"abc123def456\""));
        assert!(!messages[3]
            .content
            .contains("full snippet should not be copied"));
        assert!(matches!(messages[4].role, Role::Assistant));
        assert!(matches!(messages[5].role, Role::User));
    }

    #[test]
    fn test_format_conversation_for_copy() {
        // 2 轮对话：第 1 轮有选中文本，第 2 轮无
        let turns = vec![
            make_turn("翻译这段话", Some("Hello world"), "你好世界"),
            make_turn("再简洁一些", None, "世界你好"),
        ];

        let output = format_conversation_for_copy(&turns);

        // 第 1 轮：包含问、选中文本、答
        assert!(output.contains("**问**: 翻译这段话"));
        assert!(output.contains("> 选中文本: Hello world"));
        assert!(output.contains("**答**: 你好世界"));
        // 分隔线
        assert!(output.contains("---"));
        // 第 2 轮：包含问、答，无选中文本
        assert!(output.contains("**问**: 再简洁一些"));
        assert!(output.contains("**答**: 世界你好"));
        // 第 2 轮不应包含 "选中文本:" 相关内容（排除第 1 轮的匹配）
        let second_turn_start = output.find("---").unwrap();
        let second_part = &output[second_turn_start..];
        assert!(!second_part.contains("> 选中文本:"));
    }

    #[test]
    fn test_format_conversation_for_copy_single_turn() {
        // 单轮对话不应包含分隔线
        let turns = vec![make_turn("你好", None, "你好！有什么可以帮你？")];

        let output = format_conversation_for_copy(&turns);

        assert!(output.contains("**问**: 你好"));
        assert!(output.contains("**答**: 你好！有什么可以帮你？"));
        assert!(!output.contains("---"));
    }
}
