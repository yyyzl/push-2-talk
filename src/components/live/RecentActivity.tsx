import type { MouseEvent } from "react";
import { Clock, Copy } from "lucide-react";
import type { HistoryRecord } from "../../types";
import { formatTimestamp, formatMsShort } from "../../utils";

export type RecentActivityProps = {
  history: HistoryRecord[];
  onCopyText: (text: string, e?: MouseEvent) => void;
  onOpenHistory: () => void;
};

export function RecentActivity({ history, onCopyText, onOpenHistory }: RecentActivityProps) {
  const items = history.slice(0, 12);

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <div className="text-xs font-bold text-stone-400 uppercase tracking-widest">
          最近
        </div>
        <button
          onClick={onOpenHistory}
          className="text-[11px] font-bold text-[var(--steel)] hover:opacity-80 transition-opacity"
        >
          查看全部
        </button>
      </div>

      <div className="space-y-3">
        {items.length === 0 ? (
          <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 text-center text-stone-400">
            暂无历史记录
          </div>
        ) : (
          items.map((r) => {
            const badge =
              r.mode === "assistant"
                ? {
                    // AI 助手模式：白底+蓝色边框
                    bg: "bg-white border border-[rgba(59,130,246,0.5)]",
                    fg: "text-blue-500",
                    text: "AI 助手"
                  }
                : r.success && r.polishedText
                  ? {
                      // 润色后：白底+橙色边框，与原始转写视觉重量一致
                      bg: "bg-white border border-[rgba(217,119,87,0.5)]",
                      fg: "text-[var(--crail)]",
                      text: r.presetName || "智能润色"
                    }
                  : r.success
                    ? {
                        // 原始转写：白底+灰边框
                        bg: "bg-white border border-stone-300",
                        fg: "text-stone-600",
                        text: "原始转写"
                      }
                    : {
                        bg: "bg-white border border-red-200",
                        fg: "text-red-600",
                        text: "失败"
                      };

            const text = r.polishedText ?? r.originalText ?? r.errorMessage ?? "";

            return (
              <div
                key={r.id}
                className="bg-white border border-[var(--stone)] rounded-2xl p-4 hover:border-[rgba(176,174,165,0.75)] transition-colors group"
              >
                <div className="flex items-center justify-between gap-3 mb-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="text-[10px] font-bold text-stone-300 mono flex items-center gap-1 shrink-0">
                      <Clock size={12} />
                      {formatTimestamp(r.timestamp)}
                    </span>
                    <span className={["px-1.5 py-[2px] text-[10px] font-bold rounded shrink-0", badge.bg, badge.fg].join(" ")}>
                      {badge.text}
                    </span>
                    {r.success && (
                      <span className="text-[10px] font-medium font-mono text-stone-400 px-1 shrink-0">
                        {formatMsShort(r.totalTimeMs)}s
                      </span>
                    )}
                  </div>

                  {text && r.success && (
                    <button
                      onClick={(e) => onCopyText(text, e)}
                      className="p-2 rounded-xl bg-[var(--paper)] border border-[var(--stone)] text-stone-400 hover:text-[var(--steel)] hover:border-[rgba(176,174,165,0.75)] transition-colors opacity-0 group-hover:opacity-100"
                      title="复制"
                    >
                      <Copy size={14} />
                    </button>
                  )}
                </div>

                <p
                  className={[
                    "text-sm leading-relaxed font-medium",
                    r.success ? "text-stone-800" : "text-red-600",
                  ].join(" ")}
                >
                  {text || "（空）"}
                </p>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
