import { useState, useRef, useEffect, type MouseEvent, type RefObject } from "react";
import { Activity, Copy, Mic, MessageSquare, Sparkles, ChevronDown, ChevronUp } from "lucide-react";

export type TranscriptDisplayProps = {
  transcript: string;
  originalTranscript: string | null;
  currentMode: string | null;
  asrTime: number | null;
  llmTime: number | null;
  totalTime: number | null;
  activePresetName: string | null;
  transcriptEndRef: RefObject<HTMLDivElement>;
  onCopy: (text: string, e?: MouseEvent) => void;
  variant?: "default" | "compact";
};

export function TranscriptDisplay({
  transcript,
  originalTranscript,
  currentMode,
  asrTime,
  llmTime,
  totalTime,
  activePresetName,
  transcriptEndRef,
  onCopy,
  variant = "default",
}: TranscriptDisplayProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const textRef = useRef<HTMLDivElement>(null);
  const isRealtimeCompact = variant === "compact" && !originalTranscript;
  // 双栏模式：自适应高度，内容少时收缩，多时滚动
  // 单栏模式：保持原有固定高度
  const heightClass = originalTranscript
    ? "min-h-[10rem]"
    : variant === "compact" ? "h-48" : "h-64";
  const realtimeCompactText = transcript
    ? transcript.replace(/\s+/g, " ").trim()
    : "";

  // 检测实际溢出（仅在内容变化时检测，展开/收起时保持状态）
  useEffect(() => {
    if (textRef.current && !isExpanded) {
      const el = textRef.current;
      // 临时移除 max-height 限制来测量真实高度
      const originalMaxHeight = el.style.maxHeight;
      el.style.maxHeight = "none";
      const realHeight = el.scrollHeight;
      el.style.maxHeight = originalMaxHeight;
      setIsOverflowing(realHeight > 48 + 4);
    }
  }, [realtimeCompactText]);

  if (isRealtimeCompact) {
    return (
      <div className="bg-white border border-[var(--stone)] rounded-2xl px-5 py-4 shadow-sm">
        <div className="flex items-center justify-between gap-3 mb-2">
          <label className="text-xs font-bold text-stone-400 uppercase tracking-wider flex items-center gap-1 shrink-0">
            <Activity size={14} />
            实时转写
          </label>

          {realtimeCompactText && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-stone-400 bg-stone-50 px-2 py-1 rounded-md shrink-0">
                {transcript.length} 字
              </span>
              {(isOverflowing || isExpanded) && (
                <button
                  onClick={() => setIsExpanded(!isExpanded)}
                  className="p-1.5 rounded-xl bg-[var(--paper)] border border-[var(--stone)] text-stone-400 hover:text-[var(--steel)] hover:border-[rgba(176,174,165,0.75)] transition-colors shrink-0"
                  title={isExpanded ? "收起" : "展开"}
                >
                  {isExpanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
                </button>
              )}
              <button
                onClick={(e) => onCopy(transcript, e)}
                className="p-1.5 rounded-xl bg-[var(--paper)] border border-[var(--stone)] text-stone-400 hover:text-[var(--steel)] hover:border-[rgba(176,174,165,0.75)] transition-colors shrink-0"
                title="复制文本"
              >
                <Copy size={13} />
              </button>
            </div>
          )}
        </div>

        <div className="relative">
          <div
            ref={textRef}
            className={[
              "text-base leading-relaxed overflow-hidden",
              realtimeCompactText ? "text-stone-800" : "text-gray-400/50 italic",
              isExpanded ? "overflow-y-auto custom-scroll" : "",
            ].join(" ")}
            style={{
              maxHeight: isExpanded ? 160 : 48,
              transition: "max-height 0.4s cubic-bezier(0.4, 0, 0.2, 1)",
            }}
          >
            {realtimeCompactText || "按下快捷键开始说话..."}
          </div>

          <div
            className="absolute bottom-0 left-0 right-0 h-5 pointer-events-none"
            style={{
              background: "linear-gradient(to bottom, transparent, white)",
              opacity: !isExpanded && isOverflowing ? 1 : 0,
              transition: "opacity 0.3s ease-out",
            }}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="relative">
      <div className={["flex flex-col bg-white border border-[var(--stone)] rounded-2xl p-6 shadow-sm", heightClass].join(" ")}>
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <label className="text-xs font-bold text-stone-400 uppercase tracking-wider flex items-center gap-1">
              <Activity size={14} />
              {originalTranscript
                ? currentMode === "assistant"
                  ? "AI 助手"
                  : "转写结果"
                : "实时转写内容"}
            </label>
            {transcript && !originalTranscript && (
              <button
                onClick={(e) => onCopy(transcript, e)}
                className="p-1.5 rounded-xl bg-[var(--paper)] border border-[var(--stone)] text-stone-400 hover:text-[var(--steel)] hover:border-[rgba(176,174,165,0.75)] transition-colors"
                title="复制文本"
              >
                <Copy size={13} />
              </button>
            )}
          </div>
          {transcript && (
            <div className="flex items-center gap-2 flex-wrap justify-end">
              {asrTime !== null && (
                <span className="text-xs text-[var(--steel)] bg-[rgba(106,155,204,0.12)] px-2 py-1 rounded-md" title="语音转录耗时">
                  ASR {(asrTime / 1000).toFixed(2)}s
                </span>
              )}
              {llmTime !== null && (
                <span className="text-xs text-[var(--crail)] bg-[rgba(217,119,87,0.12)] px-2 py-1 rounded-md" title="LLM 润色耗时">
                  LLM {(llmTime / 1000).toFixed(2)}s
                </span>
              )}
              {totalTime !== null && (
                <span className="text-xs text-stone-600 bg-stone-50 px-2 py-1 rounded-md" title="总耗时">
                  共 {(totalTime / 1000).toFixed(2)}s
                </span>
              )}
              <span className="text-xs text-stone-400 bg-stone-50 px-2 py-1 rounded-md">
                {transcript.length} 字
              </span>
            </div>
          )}
        </div>

        {originalTranscript ? (
          <div className="grid grid-cols-2 gap-4">
            <div className="flex flex-col border-r border-[var(--stone)] pr-4">
              <div className="flex items-center justify-between mb-2">
                <div className="text-xs text-stone-400 flex items-center gap-1">
                  <Mic size={12} /> {currentMode === "assistant" ? "用户问题" : "原始转录"}
                </div>
                <button
                  onClick={(e) => onCopy(originalTranscript, e)}
                  className="p-1 rounded-md text-stone-400 hover:text-[var(--steel)] hover:bg-[var(--panel)] transition-colors"
                  title="复制原始文本"
                >
                  <Copy size={12} />
                </button>
              </div>
              <div className="overflow-y-auto pr-2 custom-scroll max-h-[18rem]">
                <p className="text-stone-600 text-base leading-relaxed whitespace-pre-wrap">{originalTranscript}</p>
              </div>
            </div>

            <div className="flex flex-col">
              <div className="flex items-center justify-between mb-2">
                <div className="text-xs text-[var(--crail)] flex items-center gap-1">
                  {currentMode === "assistant" ? <MessageSquare size={12} /> : <Sparkles size={12} />}
                  {currentMode === "assistant"
                    ? "AI 助手"
                    : `${activePresetName || "智能"}润色`}
                </div>
                <button
                  onClick={(e) => onCopy(transcript, e)}
                  className="p-1 rounded-md text-stone-400 hover:text-[var(--steel)] hover:bg-[var(--panel)] transition-colors"
                  title="复制结果"
                >
                  <Copy size={12} />
                </button>
              </div>
              <div className="overflow-y-auto pr-2 custom-scroll max-h-[18rem]">
                <p className="text-stone-800 text-base leading-relaxed whitespace-pre-wrap">{transcript}</p>
                <div ref={transcriptEndRef} />
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto pr-2 custom-scroll">
            {transcript ? (
              <p className="text-stone-800 text-base leading-relaxed whitespace-pre-wrap">{transcript}</p>
            ) : (
              <div className="h-full flex flex-col items-center justify-center text-gray-400/50 space-y-3">
                <Mic size={48} strokeWidth={1} className="opacity-60" />
                <p className="text-sm font-medium italic">按下快捷键开始说话...</p>
              </div>
            )}
            <div ref={transcriptEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
