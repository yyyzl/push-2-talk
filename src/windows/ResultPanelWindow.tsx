import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Copy,
  X,
  MessageSquare,
  FileText,
  Clock,
  ChevronDown,
  CopyCheck,
  Loader2,
  AlertTriangle,
  SendHorizontal,
  Globe2,
  Search,
  Square,
} from "lucide-react";
import MarkdownRenderer from "../components/MarkdownRenderer";
import type {
  ConversationTurn,
  ConversationStatePayload,
  TurnPendingPayload,
  TurnCompletePayload,
  TurnErrorPayload,
  TurnDeltaPayload,
  AssistantToolCall,
} from "../types/assistant-result";
import {
  truncateText,
  formatTimingDisplay,
} from "../types/assistant-result";
import type { AppConfig } from "../types";
import { resolveInitialWebSearchEnabled } from "../utils/searchRuntime";

/** 选中文本摘要最大长度 */
const SELECTED_TEXT_MAX_LENGTH = 100;

/** 复制成功反馈显示时长（毫秒） */
const COPY_FEEDBACK_DURATION_MS = 2000;

/** 轮询间隔（毫秒）— 无结果时向后端拉取 */
const POLL_INTERVAL_MS = 300;

/** 滚动到底部的判定阈值（像素） */
const SCROLL_BOTTOM_THRESHOLD = 30;

/** 开始窗口拖动（透明窗口下 data-tauri-drag-region 不可靠） */
const startDrag = () => {
  getCurrentWindow().startDragging().catch(() => {});
};

const isSameConversationTurn = (
  left: ConversationTurn | undefined,
  right: ConversationTurn,
) =>
  !!left &&
  left.user_instruction === right.user_instruction &&
  left.assistant_response === right.assistant_response &&
  left.llm_time_ms === right.llm_time_ms;

export default function ResultPanelWindow() {
  // ==========================================
  // State
  // ==========================================
  const [turns, setTurns] = useState<ConversationTurn[]>([]);
  const [pendingTurn, setPendingTurn] = useState<TurnPendingPayload | null>(
    null,
  );
  const [streamingResponse, setStreamingResponse] = useState("");
  const [pendingToolCalls, setPendingToolCalls] = useState<AssistantToolCall[]>([]);
  const [conversationStatus, setConversationStatus] = useState<string>("idle");
  const [activeTurnId, setActiveTurnId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [warningMessage, setWarningMessage] = useState<string | null>(null);
  const [theme, setTheme] = useState("light");
  const [webSearchEnabled, setWebSearchEnabled] = useState(false);
  const [copyFeedback, setCopyFeedback] = useState<"latest" | "all" | false>(
    false,
  );
  const [isAtBottom, setIsAtBottom] = useState(true);

  const containerRef = useRef<HTMLDivElement>(null);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeTurnIdRef = useRef<string | null>(null);

  const isDark = theme === "dark";
  const isProcessing = conversationStatus === "processing" && !!pendingTurn;
  const isCancelled = conversationStatus === "cancelled" && !!pendingTurn;

  useEffect(() => {
    activeTurnIdRef.current = activeTurnId;
  }, [activeTurnId]);

  const isCurrentTurnEvent = useCallback((turnId?: string | null) => {
    const current = activeTurnIdRef.current;
    return !turnId || !current || turnId === current;
  }, []);

  // ==========================================
  // 初始化主题
  // ==========================================
  useEffect(() => {
    invoke<AppConfig>("load_config")
      .then((config) => {
        setTheme(config.theme || "light");
        setWebSearchEnabled(resolveInitialWebSearchEnabled(config));
      })
      .catch(console.error);
  }, []);

  // ==========================================
  // 滚动控制
  // ==========================================
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const atBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight <
      SCROLL_BOTTOM_THRESHOLD;
    setIsAtBottom(atBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    containerRef.current?.scrollTo({
      top: containerRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, []);

  // 新 turn 或 pendingTurn 到达时自动滚动
  useEffect(() => {
    if (isAtBottom) {
      // 用 requestAnimationFrame 等 DOM 更新完成
      requestAnimationFrame(() => {
        containerRef.current?.scrollTo({
          top: containerRef.current.scrollHeight,
          behavior: "smooth",
        });
      });
    }
  }, [turns.length, pendingTurn, isAtBottom]);

  // ==========================================
  // Push 模式 — 监听 3 个事件 + config_updated
  // ==========================================
  // 注意：React 18 StrictMode 在 dev 模式下会双重挂载组件。
  // 异步 listen() 尚未 resolve 时 cleanup 就执行，导致旧 listener 泄漏。
  // 使用 cancelled flag + 延迟 unsubscribe 确保正确清理。
  useEffect(() => {
    let cancelled = false;
    const cleanups: (() => void)[] = [];

    const setup = async () => {
      const u1 = await listen<TurnCompletePayload>(
        "assistant_turn_complete",
        (event) => {
          if (cancelled) return;
          const { turn, is_followup } = event.payload;
          console.log(
            `[ResultPanel] turn_complete (push), followup=${is_followup}`,
          );

          if (!is_followup) {
            // 首轮：重置为新对话
            setTurns([turn]);
          } else {
            // 追问：追加
            setTurns((prev) =>
              isSameConversationTurn(prev[prev.length - 1], turn)
                ? prev
                : [...prev, turn],
            );
          }
          setPendingTurn(null);
          setActiveTurnId(null);
          setStreamingResponse("");
          setPendingToolCalls([]);
          setConversationStatus("idle");
          setErrorMessage(null);
          setWarningMessage(null);
          setCopyFeedback(false);
        },
      );
      if (cancelled) { u1(); return; }
      cleanups.push(u1);

      const u2 = await listen<TurnPendingPayload>(
        "assistant_turn_pending",
        (event) => {
          if (cancelled) return;
          console.log("[ResultPanel] turn_pending (push)");
          setPendingTurn(event.payload);
          setActiveTurnId(event.payload.turn_id ?? null);
          setStreamingResponse("");
          setPendingToolCalls([]);
          setConversationStatus("processing");
          setErrorMessage(null);
          setWarningMessage(null);
        },
      );
      if (cancelled) { u2(); return; }
      cleanups.push(u2);

      const u3 = await listen<TurnErrorPayload>(
        "assistant_turn_error",
        (event) => {
          if (cancelled) return;
          console.log("[ResultPanel] turn_error (push):", event.payload);
          setErrorMessage(event.payload.error_message);
          setPendingTurn(null);
          setActiveTurnId(null);
          setStreamingResponse("");
          setPendingToolCalls([]);
          setConversationStatus("error");
          setWarningMessage(null);
        },
      );
      if (cancelled) { u3(); return; }
      cleanups.push(u3);

      const uDelta = await listen<TurnDeltaPayload>(
        "assistant_turn_delta",
        (event) => {
          if (cancelled) return;
          if (!isCurrentTurnEvent(event.payload.turn_id)) return;
          if (event.payload.draft_assistant_response !== undefined) {
            setStreamingResponse(event.payload.draft_assistant_response);
          } else {
            setStreamingResponse((prev) => prev + event.payload.content_delta);
          }
        },
      );
      if (cancelled) { uDelta(); return; }
      cleanups.push(uDelta);

      const uToolStarted = await listen<{
        turn_id?: string;
        id: string;
        name: string;
        query: string;
        round: number;
      }>("assistant_tool_call_started", (event) => {
        if (cancelled) return;
        if (!isCurrentTurnEvent(event.payload.turn_id)) return;
        const started: AssistantToolCall = {
          id: event.payload.id,
          name: event.payload.name,
          query: event.payload.query,
          status: "searching",
          results: [],
          elapsed_ms: 0,
          round: event.payload.round,
        };
        setPendingToolCalls((prev) => [...prev.filter((item) => item.id !== started.id), started]);
      });
      if (cancelled) { uToolStarted(); return; }
      cleanups.push(uToolStarted);

      const uToolFinished = await listen<{ turn_id?: string; call: AssistantToolCall }>(
        "assistant_tool_call_finished",
        (event) => {
          if (cancelled) return;
          if (!isCurrentTurnEvent(event.payload.turn_id)) return;
          setPendingToolCalls((prev) => [
            ...prev.filter((item) => item.id !== event.payload.call.id),
            event.payload.call,
          ]);
        },
      );
      if (cancelled) { uToolFinished(); return; }
      cleanups.push(uToolFinished);

      const uWarning = await listen<{ turn_id?: string; message: string }>(
        "assistant_turn_warning",
        (event) => {
          if (cancelled) return;
          if (!isCurrentTurnEvent(event.payload.turn_id)) return;
          setWarningMessage(event.payload.message);
        },
      );
      if (cancelled) { uWarning(); return; }
      cleanups.push(uWarning);

      const uCancelled = await listen<{
        turn_id?: string;
        partial_content?: string;
        tool_calls?: AssistantToolCall[];
        message?: string;
      }>("assistant_turn_cancelled", (event) => {
        if (cancelled) return;
        if (!isCurrentTurnEvent(event.payload.turn_id)) return;
        setConversationStatus("cancelled");
        if (event.payload.partial_content !== undefined) {
          setStreamingResponse(event.payload.partial_content);
        }
        if (event.payload.tool_calls) {
          setPendingToolCalls(event.payload.tool_calls);
        }
        setWarningMessage(event.payload.message ?? "已停止生成");
      });
      if (cancelled) { uCancelled(); return; }
      cleanups.push(uCancelled);

      const u4 = await listen<AppConfig>("config_updated", (event) => {
        if (cancelled) return;
        setTheme(event.payload.theme || "light");
        setWebSearchEnabled(resolveInitialWebSearchEnabled(event.payload));
      });
      if (cancelled) { u4(); return; }
      cleanups.push(u4);
    };

    setup();

    return () => {
      cancelled = true;
      cleanups.forEach((fn) => fn());
    };
  }, []);

  // ==========================================
  // Poll 模式 — 解决隐藏 WebView 丢失 push 事件
  // ==========================================
  useEffect(() => {
    const fetchState = async () => {
      try {
        const state = await invoke<ConversationStatePayload | null>(
          "get_conversation_state",
        );
        if (state) {
          console.log("[ResultPanel] 拉取到会话 (poll):", state.session_id);
          const nextStatus =
            state.status ?? (state.is_processing ? "processing" : "idle");
          const stateMessage = state.warning_message ?? null;
          setTurns(state.turns);
          setPendingTurn(state.pending_turn ?? null);
          setActiveTurnId(state.pending_turn?.turn_id ?? null);
          setStreamingResponse(state.draft_assistant_response ?? "");
          setPendingToolCalls(state.draft_tool_calls ?? []);
          setConversationStatus(nextStatus);
          setErrorMessage(nextStatus === "error" ? stateMessage ?? "AI 助手处理失败" : null);
          setWarningMessage(nextStatus === "error" ? null : stateMessage);
          if (typeof state.web_search_enabled === "boolean") {
            setWebSearchEnabled(state.web_search_enabled);
          }
          setCopyFeedback(false);
        }
      } catch {
        // 静默忽略
      }
    };

    fetchState();
    const interval = setInterval(fetchState, POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, []);

  // ==========================================
  // 操作处理
  // ==========================================
  const handleCopyLatest = useCallback(async () => {
    if (turns.length === 0) return;
    try {
      await invoke("copy_latest_reply");
      setCopyFeedback("latest");
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        setCopyFeedback(false);
      }, COPY_FEEDBACK_DURATION_MS);
    } catch (err) {
      console.error("[ResultPanel] 复制最新回复失败:", err);
    }
  }, [turns.length]);

  const handleCopyAll = useCallback(async () => {
    if (turns.length === 0) return;
    try {
      await invoke("copy_full_conversation");
      setCopyFeedback("all");
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        setCopyFeedback(false);
      }, COPY_FEEDBACK_DURATION_MS);
    } catch (err) {
      console.error("[ResultPanel] 复制全部对话失败:", err);
    }
  }, [turns.length]);

  const handleDismiss = useCallback(async () => {
    try {
      await invoke("dismiss_conversation");
      setTurns([]);
      setPendingTurn(null);
      setActiveTurnId(null);
      setStreamingResponse("");
      setPendingToolCalls([]);
      setConversationStatus("idle");
      setErrorMessage(null);
      setWarningMessage(null);
      setCopyFeedback(false);
    } catch (err) {
      console.error("[ResultPanel] 关闭失败:", err);
    }
  }, [isCurrentTurnEvent]);

  const handleCancelGeneration = useCallback(async () => {
    try {
      await invoke("cancel_assistant_generation");
    } catch (err) {
      console.error("[ResultPanel] 停止生成失败:", err);
    }
  }, []);

  // ==========================================
  // 文本追问
  // ==========================================
  const handleTextSend = useCallback(async (text: string): Promise<string | null> => {
    try {
      await invoke("send_text_question", { text, webSearchEnabled });
      return null;
    } catch (err) {
      console.error("[ResultPanel] 文本追问失败:", err);
      return typeof err === "string" ? err : "发送失败，请重试";
    }
  }, [webSearchEnabled]);

  // ==========================================
  // 键盘快捷键：生成中 Esc = 停止，空闲时 Esc = 关闭
  // ==========================================
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        if (isProcessing) {
          void handleCancelGeneration();
        } else {
          handleDismiss();
        }
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleCancelGeneration, handleDismiss, isProcessing]);

  // 清理定时器
  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  // ==========================================
  // 无结果时的空状态
  // ==========================================
  if (turns.length === 0 && !pendingTurn && !errorMessage && !warningMessage) {
    return (
      <div
        onMouseDown={startDrag}
        className={`h-screen flex items-center justify-center select-none cursor-move ${isDark ? "theme-dark" : ""}`}
        style={{
          background: isDark ? "#141413" : "var(--paper)",
          color: isDark ? "#E8E6DC" : "var(--ink)",
          fontFamily: "var(--font-serif)",
          borderRadius: "12px",
        }}
      >
        <p
          style={{
            color: isDark ? "#888" : "var(--stone-dark)",
            fontSize: "14px",
          }}
        >
          等待 AI 助手结果...
        </p>
      </div>
    );
  }

  // ==========================================
  // 渲染：对话流视图
  // ==========================================
  return (
    <div
      className={`h-screen flex flex-col ${isDark ? "theme-dark" : ""}`}
      style={{
        background: "transparent",
        fontFamily: "var(--font-serif)",
      }}
    >
      {/* 外壳：圆角 + 阴影 */}
      <div
        className="flex flex-col flex-1 overflow-hidden"
        style={{
          borderRadius: "12px",
          background: isDark ? "#141413" : "var(--paper)",
          color: isDark ? "#E8E6DC" : "var(--ink)",
          boxShadow: isDark
            ? "0 8px 32px rgba(0,0,0,0.5), 0 2px 8px rgba(0,0,0,0.3)"
            : "0 8px 32px rgba(20,20,19,0.12), 0 2px 8px rgba(20,20,19,0.06)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.1)"
            : "1px solid var(--sand)",
        }}
      >
        {/* ==========================================
            标题栏 (可拖动)
            ========================================== */}
        <div
          onMouseDown={startDrag}
          className="flex items-center justify-between px-4 py-3 select-none cursor-move shrink-0"
          style={{
            background: isDark ? "#1E1E1D" : "var(--sand)",
            borderBottom: isDark
              ? "1px solid rgba(232,230,220,0.08)"
              : "1px solid rgba(232,230,220,0.6)",
            fontFamily: "var(--font-sans)",
          }}
        >
          {/* 左侧：图标 + 标题 */}
          <div className="flex items-center gap-2">
            <span style={{ fontSize: "16px" }}>🤖</span>
            <span
              className="font-semibold text-sm"
              style={{ color: isDark ? "#E8E6DC" : "var(--ink)" }}
            >
              AI 助手
            </span>
            {turns.length > 1 && (
              <span
                className="text-xs px-1.5 py-0.5 rounded-full"
                style={{
                  background: isDark
                    ? "rgba(255,255,255,0.08)"
                    : "rgba(0,0,0,0.06)",
                  color: isDark ? "#999" : "var(--stone-dark)",
                  fontFamily: "var(--font-mono)",
                  fontSize: "11px",
                }}
              >
                {turns.length} 轮
              </span>
            )}
          </div>

          {/* 右侧：关闭按钮 */}
          <button
            onClick={handleDismiss}
            onMouseDown={(e) => e.stopPropagation()}
            className="flex items-center justify-center rounded-md transition-colors"
            style={{
              width: "28px",
              height: "28px",
              color: isDark ? "#888" : "var(--stone-dark)",
              background: "transparent",
              border: "none",
              cursor: "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = isDark
                ? "rgba(255,255,255,0.08)"
                : "rgba(0,0,0,0.06)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
            }}
            title="关闭 (Esc)"
          >
            <X size={16} />
          </button>
        </div>

        {/* ==========================================
            对话流区域 (可滚动)
            ========================================== */}
        <div
          ref={containerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto custom-scrollbar"
          style={{
            background: isDark ? "#1A1A19" : "white",
          }}
        >
          {turns.map((turn, i) => (
            <div key={`turn-${i}`}>
              {/* 轮次分隔线 */}
              {i > 0 && <TurnDivider isDark={isDark} index={i} />}

              {/* 用户气泡 */}
              <UserBubble
                instruction={turn.user_instruction}
                selectedText={turn.selected_text}
                hasSelection={turn.has_selection}
                isDark={isDark}
              />

              {/* AI 回复 */}
              <AssistantBubble
                response={turn.assistant_response}
                asrTimeMs={turn.asr_time_ms}
                llmTimeMs={turn.llm_time_ms}
                searchTimeMs={turn.search_time_ms}
                toolCalls={turn.tool_calls}
                isDark={isDark}
              />
            </div>
          ))}

          {/* 追问 pending 状态 */}
          {pendingTurn && (
            <div>
              {turns.length > 0 && (
                <TurnDivider isDark={isDark} index={turns.length} />
              )}
              <UserBubble
                instruction={pendingTurn.user_instruction}
                selectedText={pendingTurn.selected_text}
                hasSelection={pendingTurn.has_selection}
                isDark={isDark}
              />
              {isCancelled && !streamingResponse && pendingToolCalls.length === 0 ? (
                <CancelledBubble
                  isDark={isDark}
                  onRetry={() => {
                    void handleTextSend(pendingTurn.user_instruction);
                  }}
                />
              ) : streamingResponse || pendingToolCalls.length > 0 || isCancelled ? (
                <AssistantBubble
                  response={streamingResponse}
                  asrTimeMs={0}
                  llmTimeMs={0}
                  searchTimeMs={null}
                  toolCalls={pendingToolCalls}
                  isDark={isDark}
                  isStreaming={isProcessing}
                  isCancelled={isCancelled}
                />
              ) : (
                <LoadingBubble isDark={isDark} />
              )}
            </div>
          )}

          {/* 错误气泡 */}
          {errorMessage && <ErrorBubble message={errorMessage} isDark={isDark} />}
          {warningMessage && <WarningBubble message={warningMessage} isDark={isDark} />}
        </div>

        {/* ==========================================
            "查看最新回复" 浮标
            ========================================== */}
        {!isAtBottom && (turns.length > 1 || pendingTurn) && (
          <div
            style={{
              position: "relative",
            }}
          >
            <button
              onClick={scrollToBottom}
              className="flex items-center gap-1 px-3 py-1.5 rounded-full text-xs transition-all"
              style={{
                position: "absolute",
                bottom: "8px",
                right: "16px",
                background: isDark
                  ? "rgba(255,255,255,0.12)"
                  : "rgba(0,0,0,0.06)",
                color: isDark ? "#ccc" : "var(--ink)",
                border: isDark
                  ? "1px solid rgba(255,255,255,0.15)"
                  : "1px solid rgba(0,0,0,0.08)",
                cursor: "pointer",
                backdropFilter: "blur(8px)",
                zIndex: 10,
                fontFamily: "var(--font-sans)",
              }}
            >
              <ChevronDown size={14} />
              <span>查看最新回复</span>
            </button>
          </div>
        )}

        {/* ==========================================
            文本输入栏
            ========================================== */}
        <TextInputBar
          isDark={isDark}
          isProcessing={isProcessing}
          webSearchEnabled={webSearchEnabled}
          onToggleWebSearch={() => setWebSearchEnabled((prev) => !prev)}
          onSend={handleTextSend}
        />

        {/* ==========================================
            操作栏
            ========================================== */}
        <div
          className="flex items-center justify-end gap-2 px-4 py-3 shrink-0"
          style={{
            background: isDark ? "#1E1E1D" : "var(--sand)",
            borderTop: isDark
              ? "1px solid rgba(232,230,220,0.08)"
              : "1px solid rgba(232,230,220,0.6)",
            fontFamily: "var(--font-sans)",
          }}
        >
          {/* 关闭按钮（次要） */}
          <button
            onClick={isProcessing ? handleCancelGeneration : handleDismiss}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-all"
            style={{
              border: isProcessing
                ? "1px solid rgba(239,68,68,0.25)"
                : isDark
                  ? "1px solid #333"
                  : "1px solid var(--sand)",
              background: isProcessing
                ? isDark
                  ? "rgba(239,68,68,0.08)"
                  : "rgba(239,68,68,0.05)"
                : isDark
                  ? "rgba(255,255,255,0.05)"
                  : "white",
              color: isProcessing ? "#dc2626" : isDark ? "#ccc" : "var(--ink)",
              cursor: "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = isDark
                ? isProcessing
                  ? "rgba(239,68,68,0.12)"
                  : "rgba(255,255,255,0.1)"
                : isProcessing
                  ? "rgba(239,68,68,0.08)"
                  : "rgba(0,0,0,0.03)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = isProcessing
                ? isDark
                  ? "rgba(239,68,68,0.08)"
                  : "rgba(239,68,68,0.05)"
                : isDark
                  ? "rgba(255,255,255,0.05)"
                  : "white";
            }}
          >
            {isProcessing ? <Square size={14} /> : <X size={14} />}
            <span>{isProcessing ? "停止生成" : "关闭"}</span>
          </button>

          {/* 复制全部（多轮时显示） */}
          {turns.length > 1 && (
            <button
              onClick={handleCopyAll}
              disabled={isProcessing}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-all"
              style={{
                border: isDark ? "1px solid #333" : "1px solid var(--sand)",
                background: isDark ? "rgba(255,255,255,0.05)" : "white",
                color: isDark ? "#ccc" : "var(--ink)",
                cursor: isProcessing ? "not-allowed" : "pointer",
                opacity: isProcessing ? 0.4 : 1,
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = isDark
                  ? "rgba(255,255,255,0.1)"
                  : "rgba(0,0,0,0.03)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = isDark
                  ? "rgba(255,255,255,0.05)"
                  : "white";
              }}
            >
              {copyFeedback === "all" ? (
                <CopyCheck size={14} />
              ) : (
                <Copy size={14} />
              )}
              <span>{copyFeedback === "all" ? "已复制全部" : "复制全部"}</span>
            </button>
          )}

          {/* 复制最新回复（主要） */}
          <button
            onClick={handleCopyLatest}
            disabled={isProcessing || turns.length === 0}
            className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-sm font-medium transition-all"
            style={{
              background: "var(--crail)",
              color: "white",
              border: "none",
              cursor: isProcessing || turns.length === 0 ? "not-allowed" : "pointer",
              opacity: isProcessing || turns.length === 0 ? 0.4 : 1,
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.opacity = "0.9";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.opacity = "1";
            }}
          >
            {copyFeedback === "latest" ? (
              <CopyCheck size={14} />
            ) : (
              <Copy size={14} />
            )}
            <span>
              {copyFeedback === "latest"
                ? "已复制"
                : turns.length > 1
                  ? "复制最新回复"
                  : "复制"}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}

// ==========================================
// 子组件
// ==========================================

/** 轮次分隔线 */
function TurnDivider({ isDark, index }: { isDark: boolean; index: number }) {
  return (
    <div
      className="flex items-center gap-3 px-4 py-2"
      style={{
        color: isDark ? "#666" : "var(--stone-dark)",
        fontSize: "11px",
        fontFamily: "var(--font-sans)",
      }}
    >
      <div
        className="flex-1"
        style={{
          height: "1px",
          background: isDark
            ? "rgba(232,230,220,0.08)"
            : "rgba(0,0,0,0.06)",
        }}
      />
      <span>追问 #{index}</span>
      <div
        className="flex-1"
        style={{
          height: "1px",
          background: isDark
            ? "rgba(232,230,220,0.08)"
            : "rgba(0,0,0,0.06)",
        }}
      />
    </div>
  );
}

/** 用户气泡 */
function UserBubble({
  instruction,
  selectedText,
  hasSelection,
  isDark,
}: {
  instruction: string;
  selectedText?: string;
  hasSelection: boolean;
  isDark: boolean;
}) {
  return (
    <div className="px-4 py-3" style={{ fontSize: "13px" }}>
      {/* 语音指令 */}
      <div className="flex items-start gap-2">
        <MessageSquare
          size={14}
          className="shrink-0 mt-0.5"
          style={{ color: isDark ? "#888" : "var(--stone-dark)" }}
        />
        <span
          style={{
            color: isDark ? "#ccc" : "var(--ink)",
            lineHeight: 1.5,
          }}
        >
          {instruction}
        </span>
      </div>

      {/* 选中文本摘要 */}
      {hasSelection && selectedText && (
        <SelectedTextPreview selectedText={selectedText} isDark={isDark} />
      )}
    </div>
  );
}

/** 用户本轮选中文本预览 */
function SelectedTextPreview({
  selectedText,
  isDark,
}: {
  selectedText: string;
  isDark: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = selectedText.length > SELECTED_TEXT_MAX_LENGTH;
  const displayText = expanded
    ? selectedText
    : truncateText(selectedText, SELECTED_TEXT_MAX_LENGTH);

  return (
    <div
      className="mt-2 rounded-lg px-3 py-2"
      style={{
        background: isDark ? "rgba(255,255,255,0.035)" : "rgba(120,140,93,0.055)",
        border: isDark
          ? "1px solid rgba(232,230,220,0.08)"
          : "1px solid rgba(120,140,93,0.16)",
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <div
          className="flex min-w-0 items-center gap-1.5"
          style={{
            color: isDark ? "#c8d2aa" : "var(--sage)",
            fontSize: "11px",
            fontWeight: 700,
          }}
        >
          <FileText size={12} className="shrink-0" />
          <span>选中文本</span>
        </div>
        {canExpand && (
          <button
            type="button"
            onClick={() => setExpanded((prev) => !prev)}
            aria-expanded={expanded}
            className="flex shrink-0 items-center gap-0.5 rounded-md px-1.5 py-0.5 transition-colors"
            style={{
              color: isDark ? "#aaa" : "var(--stone-dark)",
              background: isDark ? "rgba(255,255,255,0.04)" : "rgba(255,255,255,0.65)",
              border: isDark
                ? "1px solid rgba(255,255,255,0.06)"
                : "1px solid rgba(176,174,165,0.28)",
              fontSize: "11px",
            }}
            title={expanded ? "收起选中文本" : "展开完整选中文本"}
          >
            <ChevronDown
              size={12}
              style={{
                transform: expanded ? "rotate(180deg)" : "rotate(0deg)",
                transition: "transform 150ms ease",
              }}
            />
            {expanded ? "收起" : "展开"}
          </button>
        )}
      </div>
      <div
        className="mt-1.5 overflow-y-auto"
        style={{
          maxHeight: expanded ? "168px" : "42px",
          color: isDark ? "#aaa" : "var(--stone-dark)",
          fontSize: "12px",
          lineHeight: 1.5,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {displayText}
      </div>
    </div>
  );
}

/** AI 回复气泡 */
function AssistantBubble({
  response,
  asrTimeMs,
  llmTimeMs,
  searchTimeMs,
  toolCalls = [],
  isDark,
  isStreaming = false,
  isCancelled = false,
}: {
  response: string;
  asrTimeMs: number;
  llmTimeMs: number;
  searchTimeMs?: number | null;
  toolCalls?: AssistantToolCall[];
  isDark: boolean;
  isStreaming?: boolean;
  isCancelled?: boolean;
}) {
  const citations = toolCalls.flatMap((call) => call.results);

  return (
    <div className="px-4 pb-3">
      {toolCalls.length > 0 && <SearchToolCallPanel toolCalls={toolCalls} isDark={isDark} />}

      {/* Markdown 回复 */}
      <div
        className="rounded-lg px-3 py-2"
        style={{
          background: isDark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.06)"
            : "1px solid rgba(0,0,0,0.04)",
        }}
      >
        {response ? (
          <>
            <MarkdownRenderer
              content={response}
              darkMode={isDark}
              citations={citations}
            />
            {isCancelled && (
              <span
                className="mt-1 inline-block text-xs"
                style={{ color: isDark ? "#888" : "var(--stone-dark)" }}
              >
                已停止
              </span>
            )}
          </>
        ) : (
          <span style={{ color: isDark ? "#888" : "var(--stone-dark)" }}>
            {isStreaming ? "正在生成..." : isCancelled ? "已停止生成" : ""}
          </span>
        )}
      </div>

      {/* 耗时信息 */}
      <div
        className="flex items-center gap-2 mt-1.5 px-1"
        style={{
          fontSize: "11px",
          color: isDark ? "#555" : "var(--stone-dark)",
          fontFamily: "var(--font-mono)",
        }}
      >
        <Clock size={10} />
        <span>{formatTimingDisplay(asrTimeMs, llmTimeMs, searchTimeMs)}</span>
      </div>
    </div>
  );
}

function SearchToolCallPanel({
  toolCalls,
  isDark,
}: {
  toolCalls: AssistantToolCall[];
  isDark: boolean;
}) {
  const totalResultCount = toolCalls.reduce((sum, call) => sum + call.results.length, 0);
  const isSearching = toolCalls.some((call) => call.status === "searching");
  const hasError = toolCalls.some((call) => call.status === "error");
  const statusText = isSearching
    ? "搜索中"
    : hasError && totalResultCount === 0
      ? "搜索失败"
      : `搜索到 ${totalResultCount} 个结果`;

  return (
    <div
      className="mb-2 rounded-lg px-3 py-2"
      style={{
        background: isDark ? "rgba(255,255,255,0.035)" : "rgba(120,140,93,0.055)",
        border: isDark ? "1px solid rgba(232,230,220,0.08)" : "1px solid rgba(120,140,93,0.16)",
        color: isDark ? "#d6d3c8" : "var(--ink)",
        fontFamily: "var(--font-sans)",
      }}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          {isSearching ? (
            <Loader2 size={14} className="shrink-0 animate-spin" style={{ color: "var(--sage)" }} />
          ) : (
            <Search size={14} className="shrink-0" style={{ color: "var(--sage)" }} />
          )}
          <span className="text-xs font-bold">联网搜索</span>
          <span
            className="truncate rounded-full px-2 py-0.5 text-[11px] font-semibold"
            style={{
              background: isDark ? "rgba(255,255,255,0.06)" : "rgba(120,140,93,0.12)",
              color: isDark ? "#c8d2aa" : "var(--sage)",
            }}
          >
            {statusText}
          </span>
        </div>
      </div>

      <div className="mt-2 space-y-1.5">
        {toolCalls.map((call) => (
          <div
            key={call.id}
            className="rounded-md px-2 py-1.5"
            style={{
              background: isDark ? "rgba(255,255,255,0.025)" : "rgba(255,255,255,0.72)",
              border: isDark ? "1px solid rgba(232,230,220,0.06)" : "1px solid rgba(176,174,165,0.32)",
            }}
          >
            <div className="flex items-center justify-between gap-2">
              <span
                className="truncate text-[11px] font-medium"
                style={{ color: isDark ? "#d6d3c8" : "var(--ink)" }}
                title={call.query}
              >
                {call.status === "searching" ? "正在搜索" : "检索"}：{call.query}
              </span>
              {call.elapsed_ms > 0 && (
                <span className="shrink-0 text-[10px]" style={{ color: isDark ? "#777" : "var(--stone-dark)" }}>
                  {formatDurationLabel(call.elapsed_ms)}
                </span>
              )}
            </div>

            {call.error && (
              <div className="mt-1 text-[11px]" style={{ color: isDark ? "#fca5a5" : "#b91c1c" }}>
                {call.error}
              </div>
            )}

            {call.results.length > 0 && (
              <div className="mt-1.5 space-y-1">
                {call.results.slice(0, 3).map((result) => (
                  <div key={`${call.id}-${result.id}`} className="grid grid-cols-[auto_minmax(0,1fr)] gap-2">
                    <span
                      className="mt-0.5 flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] font-bold"
                      style={{
                        background: isDark ? "rgba(255,255,255,0.06)" : "rgba(120,140,93,0.12)",
                        color: isDark ? "#c8d2aa" : "var(--sage)",
                      }}
                    >
                      {result.index}
                    </span>
                    <div className="min-w-0">
                      <div
                        className="truncate text-[11px] font-semibold"
                        style={{ color: isDark ? "#e8e6dc" : "var(--ink)" }}
                        title={result.title || result.url}
                      >
                        {result.title || result.url}
                      </div>
                      <div
                        className="truncate text-[10px]"
                        style={{ color: isDark ? "#777" : "var(--stone-dark)" }}
                        title={result.source || result.url}
                      >
                        {result.source || result.url}
                      </div>
                    </div>
                  </div>
                ))}
                {call.results.length > 3 && (
                  <div className="pl-6 text-[10px]" style={{ color: isDark ? "#777" : "var(--stone-dark)" }}>
                    还有 {call.results.length - 3} 条结果会作为引用参与回答
                  </div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function formatDurationLabel(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)}ms` : `${(ms / 1000).toFixed(1)}s`;
}

/** 加载中气泡 */
function LoadingBubble({ isDark }: { isDark: boolean }) {
  return (
    <div className="px-4 pb-3">
      <div
        className="flex items-center gap-2 rounded-lg px-3 py-3"
        style={{
          background: isDark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.06)"
            : "1px solid rgba(0,0,0,0.04)",
          color: isDark ? "#888" : "var(--stone-dark)",
          fontSize: "13px",
        }}
      >
        <Loader2 size={14} className="animate-spin" />
        <span>AI 思考中...</span>
      </div>
    </div>
  );
}

/** 已停止气泡 */
function CancelledBubble({
  isDark,
  onRetry,
}: {
  isDark: boolean;
  onRetry: () => void;
}) {
  return (
    <div className="px-4 pb-3">
      <div
        className="flex items-center justify-between gap-2 rounded-lg px-3 py-2"
        style={{
          background: isDark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
          border: isDark
            ? "1px solid rgba(232,230,220,0.06)"
            : "1px solid rgba(0,0,0,0.04)",
          color: isDark ? "#888" : "var(--stone-dark)",
          fontSize: "13px",
        }}
      >
        <span>已停止生成</span>
        <button
          type="button"
          onClick={onRetry}
          className="rounded-md px-2 py-1 text-xs transition-colors"
          style={{
            background: isDark ? "rgba(255,255,255,0.08)" : "white",
            border: isDark ? "1px solid #333" : "1px solid var(--sand)",
            color: isDark ? "#ccc" : "var(--ink)",
          }}
        >
          重试
        </button>
      </div>
    </div>
  );
}

/** 错误气泡 */
function ErrorBubble({
  message,
  isDark,
}: {
  message: string;
  isDark: boolean;
}) {
  return (
    <div className="px-4 pb-3">
      <div
        className="flex items-start gap-2 rounded-lg px-3 py-2"
        style={{
          background: isDark
            ? "rgba(239,68,68,0.08)"
            : "rgba(239,68,68,0.05)",
          border: "1px solid rgba(239,68,68,0.2)",
          color: isDark ? "#f87171" : "#dc2626",
          fontSize: "13px",
        }}
      >
        <AlertTriangle size={14} className="shrink-0 mt-0.5" />
        <span style={{ lineHeight: 1.5 }}>{message}</span>
      </div>
    </div>
  );
}

/** 警告气泡 */
function WarningBubble({
  message,
  isDark,
}: {
  message: string;
  isDark: boolean;
}) {
  return (
    <div className="px-4 pb-3">
      <div
        className="flex items-start gap-2 rounded-lg px-3 py-2"
        style={{
          background: isDark
            ? "rgba(245,158,11,0.08)"
            : "rgba(245,158,11,0.08)",
          border: "1px solid rgba(245,158,11,0.22)",
          color: isDark ? "#fbbf24" : "#b45309",
          fontSize: "13px",
        }}
      >
        <AlertTriangle size={14} className="shrink-0 mt-0.5" />
        <span style={{ lineHeight: 1.5 }}>{message}</span>
      </div>
    </div>
  );
}

/** 文本追问输入栏 */
function TextInputBar({
  isDark,
  isProcessing,
  webSearchEnabled,
  onToggleWebSearch,
  onSend,
}: {
  isDark: boolean;
  isProcessing: boolean;
  webSearchEnabled: boolean;
  onToggleWebSearch: () => void;
  onSend: (text: string) => Promise<string | null>;
}) {
  const [inputText, setInputText] = useState("");
  const [inputError, setInputError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const INPUT_BAR_CONTROL_HEIGHT = 42;

  const canSend = inputText.trim().length > 0 && !isProcessing;

  const handleSend = useCallback(async () => {
    const trimmed = inputText.trim();
    if (!trimmed || isProcessing) return;
    const error = await onSend(trimmed);
    if (error) {
      setInputError(error);
      return;
    }
    setInputText("");
    setInputError(null);
    // 重置 textarea 高度
    if (textareaRef.current) {
      textareaRef.current.style.height = `${INPUT_BAR_CONTROL_HEIGHT}px`;
    }
  }, [inputText, isProcessing, onSend, INPUT_BAR_CONTROL_HEIGHT]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter 发送（非空时），Shift+Enter 换行
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void handleSend();
      }
      // Esc 不做任何拦截，让事件冒泡到 window listener 处理关闭
    },
    [handleSend],
  );

  // textarea 自动调整高度
  const handleInput = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setInputText(e.target.value);
      setInputError(null);
      const el = e.target;
      el.style.height = `${INPUT_BAR_CONTROL_HEIGHT}px`;
      el.style.height = `${Math.max(INPUT_BAR_CONTROL_HEIGHT, Math.min(el.scrollHeight, 84))}px`;
    },
    [INPUT_BAR_CONTROL_HEIGHT],
  );

  return (
    <div
      className="flex items-center gap-3 px-6 py-3 shrink-0"
      style={{
        borderTop: isDark
          ? "1px solid rgba(232,230,220,0.08)"
          : "1px solid rgba(232,230,220,0.6)",
        fontFamily: "var(--font-sans)",
      }}
    >
      <button
        type="button"
        onClick={onToggleWebSearch}
        disabled={isProcessing}
        className="flex items-center justify-center rounded-lg transition-all shrink-0"
        style={{
          width: `${INPUT_BAR_CONTROL_HEIGHT}px`,
          height: `${INPUT_BAR_CONTROL_HEIGHT}px`,
          background: webSearchEnabled
            ? isDark
              ? "rgba(59,130,246,0.18)"
              : "rgba(59,130,246,0.12)"
            : isDark
              ? "rgba(255,255,255,0.05)"
              : "rgba(0,0,0,0.04)",
          border: webSearchEnabled
            ? "1px solid rgba(59,130,246,0.35)"
            : isDark
              ? "1px solid rgba(255,255,255,0.1)"
              : "1px solid rgba(0,0,0,0.08)",
          color: webSearchEnabled
            ? isDark
              ? "#93c5fd"
              : "#1d4ed8"
            : isDark
              ? "#777"
              : "var(--stone-dark)",
          cursor: isProcessing ? "not-allowed" : "pointer",
          opacity: isProcessing ? 0.5 : 1,
        }}
        title={webSearchEnabled ? "联网搜索已开启" : "联网搜索已关闭"}
        aria-pressed={webSearchEnabled}
      >
        <Globe2 size={16} />
      </button>
      <div className="min-w-0 flex-1">
        <div className="flex min-h-[42px] min-w-0 flex-1 items-center">
          <textarea
            ref={textareaRef}
            value={inputText}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            disabled={isProcessing}
            placeholder="输入追问..."
            rows={1}
            className="block w-full text-sm rounded-lg px-3 outline-none transition-colors"
            style={{
              boxSizing: "border-box",
              resize: "none",
              height: `${INPUT_BAR_CONTROL_HEIGHT}px`,
              minHeight: `${INPUT_BAR_CONTROL_HEIGHT}px`,
              maxHeight: "84px",
              paddingTop: "10px",
              paddingBottom: "10px",
              lineHeight: "20px",
              overflowY: "auto",
              background: isDark ? "rgba(255,255,255,0.05)" : "rgba(0,0,0,0.03)",
              border: inputError
                ? "1px solid rgba(220,38,38,0.45)"
                : isDark
                  ? "1px solid rgba(255,255,255,0.1)"
                  : "1px solid rgba(0,0,0,0.08)",
              color: isDark ? "#E8E6DC" : "var(--ink)",
              opacity: isProcessing ? 0.5 : 1,
            }}
          />
        </div>
        {inputError && (
          <div
            className="mt-1 text-xs"
            style={{ color: isDark ? "#f87171" : "#dc2626" }}
          >
            {inputError}
          </div>
        )}
      </div>
      <button
        onClick={() => void handleSend()}
        disabled={!canSend}
        className="flex items-center justify-center rounded-lg transition-all shrink-0"
        style={{
          width: `${INPUT_BAR_CONTROL_HEIGHT}px`,
          height: `${INPUT_BAR_CONTROL_HEIGHT}px`,
          background: canSend ? "var(--crail)" : isDark ? "#333" : "#ddd",
          color: canSend ? "white" : isDark ? "#666" : "#999",
          border: "none",
          cursor: canSend ? "pointer" : "not-allowed",
          opacity: canSend ? 1 : 0.6,
        }}
      >
        <SendHorizontal size={16} />
      </button>
    </div>
  );
}
