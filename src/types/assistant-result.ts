/** AI 助手模式结果事件 payload（旧版单轮，保留供兼容） */
export interface AssistantResultPayload {
  /** 结果唯一 ID */
  id: string;
  /** LLM 输出文本（可能是 Markdown） */
  result_text: string;
  /** 用户语音指令（ASR 转写） */
  instruction: string;
  /** 发起时的选中文本 */
  selected_text?: string;
  /** 是否有选中文本 */
  has_selection: boolean;
  /** ASR 耗时（毫秒） */
  asr_time_ms: number;
  /** LLM 耗时（毫秒） */
  llm_time_ms: number;
}

// ================== 多轮对话类型 ==================

/** 单轮对话记录（与后端 ConversationTurnPayload 对齐） */
export interface ConversationTurn {
  user_instruction: string;
  selected_text?: string;
  has_selection: boolean;
  assistant_response: string;
  asr_time_ms: number;
  llm_time_ms: number;
  search_time_ms?: number | null;
  tool_calls?: AssistantToolCall[];
}

/** 完整会话状态（pull 模式返回值） */
export interface ConversationStatePayload {
  session_id: string;
  turns: ConversationTurn[];
  pending_turn?: TurnPendingPayload | null;
  draft_assistant_response?: string;
  draft_tool_calls?: AssistantToolCall[];
  is_processing?: boolean;
  status?: "idle" | "processing" | "cancelled" | "error" | string;
  warning_message?: string | null;
  web_search_enabled?: boolean | null;
}

/** 追问录音完成后立即发出（前端显示用户消息 + loading） */
export interface TurnPendingPayload {
  turn_id?: string;
  user_instruction: string;
  selected_text?: string;
  has_selection: boolean;
}

/** 一轮完成事件 payload */
export interface TurnCompletePayload {
  session_id: string;
  turn: ConversationTurn;
  is_followup: boolean;
}

/** LLM 调用失败事件 payload */
export interface TurnErrorPayload {
  session_id: string;
  error_message: string;
}

export interface TurnDeltaPayload {
  session_id: string;
  turn_id?: string;
  content_delta: string;
  draft_assistant_response?: string;
}

export interface AssistantToolCall {
  id: string;
  name: string;
  query: string;
  status: "searching" | "success" | "error" | string;
  results: SearchCitation[];
  error?: string | null;
  elapsed_ms: number;
  round: number;
}

export interface SearchCitation {
  index: number;
  id: string;
  title: string;
  url: string;
  snippet: string;
  source?: string | null;
}

/**
 * 截断过长文本，超出部分用省略号替代
 *
 * @param text - 原始文本
 * @param maxLength - 最大字符数
 * @returns 截断后的文本
 */
export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + "\u2026"; // "…"
}

/**
 * 将毫秒转为可读时长
 *
 * - < 60s: "1.2s"
 * - >= 60s: "1m 5s"
 *
 * @param ms - 毫秒数
 * @returns 格式化的时长字符串
 */
export function formatDuration(ms: number): string {
  const totalSeconds = ms / 1000;
  if (totalSeconds < 60) {
    return `${totalSeconds.toFixed(1)}s`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = Math.floor(totalSeconds % 60);
  return `${minutes}m ${seconds}s`;
}

/**
 * 将多轮对话格式化为可复制的 Markdown 文本
 *
 * 格式：
 * ```
 * **问**: 用户指令
 * > 选中文本: ...（仅当有选中文本时）
 *
 * **答**: AI 回复
 *
 * ---
 *
 * **问**: 追问指令
 *
 * **答**: AI 回复
 * ```
 *
 * @param turns - 对话轮次数组
 * @returns 格式化后的 Markdown 文本
 */
export function formatConversationForCopy(turns: ConversationTurn[]): string {
  return turns
    .map((turn, i) => {
      let block = `**问**: ${turn.user_instruction}\n`;
      if (turn.has_selection && turn.selected_text) {
        block += `> 选中文本: ${turn.selected_text}\n`;
      }
      block += `\n**答**: ${turn.assistant_response}`;
      // 轮次间用分隔线分开（最后一轮不追加）
      if (i < turns.length - 1) {
        block += "\n\n---\n";
      }
      return block;
    })
    .join("\n");
}

/**
 * 根据 ASR 耗时自适应生成耗时显示文本
 *
 * - 语音轮次 (asr > 0): "ASR 1.1s · LLM 1.2s · 总计 2.3s"
 * - 文本轮次 (asr = 0): "LLM 1.2s"
 *
 * @param asrTimeMs - ASR 耗时（毫秒），文本输入轮次为 0
 * @param llmTimeMs - LLM 耗时（毫秒）
 * @returns 格式化的耗时字符串
 */
export function formatTimingDisplay(
  asrTimeMs: number,
  llmTimeMs: number,
  searchTimeMs?: number | null,
): string {
  const searchMs = searchTimeMs ?? 0;
  if (asrTimeMs > 0) {
    const totalTime = asrTimeMs + llmTimeMs + searchMs;
    const parts = [`ASR ${formatDuration(asrTimeMs)}`];
    if (searchMs > 0) {
      parts.push(`搜索 ${formatDuration(searchMs)}`);
    }
    parts.push(`LLM ${formatDuration(llmTimeMs)}`);
    parts.push(`总计 ${formatDuration(totalTime)}`);
    return parts.join(" · ");
  }
  if (searchMs > 0) {
    return `搜索 ${formatDuration(searchMs)} · LLM ${formatDuration(llmTimeMs)}`;
  }
  return `LLM ${formatDuration(llmTimeMs)}`;
}

export function findCitationByMarker(
  citations: SearchCitation[] | undefined | null,
  index: number,
  id: string,
): SearchCitation | null {
  if (!citations) return null;
  return citations.find((item) => item.index === index && item.id === id) ?? null;
}
