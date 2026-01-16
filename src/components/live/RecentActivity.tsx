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
          Recent Activity
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
              r.success && r.polishedText
                ? { bg: "bg-[rgba(217,119,87,0.12)]", fg: "text-[var(--crail)]", text: r.presetName || "AI 智能润色" }
                : r.success
                  ? { bg: "bg-stone-50", fg: "text-stone-400", text: "原始转写" }
                  : { bg: "bg-red-50", fg: "text-red-600", text: "失败" };

            const text = r.polishedText ?? r.originalText ?? r.errorMessage ?? "";

            return (
              <div
                key={r.id}
                className="bg-white border border-[var(--stone)] rounded-2xl p-5 hover:border-[rgba(176,174,165,0.75)] transition-colors group"
              >
                <div className="flex items-center justify-between gap-3 mb-3">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="text-[10px] font-bold text-stone-300 mono flex items-center gap-1 shrink-0">
                      <Clock size={12} />
                      {formatTimestamp(r.timestamp)}
                    </span>
                    <span className={["px-1.5 py-0.5 text-[9px] font-bold rounded shrink-0", badge.bg, badge.fg].join(" ")}>
                      {badge.text}
                    </span>
                    {r.success && (
                      <span className="text-[9px] font-bold text-stone-300 bg-stone-50 border border-stone-200 rounded px-1.5 py-0.5 shrink-0">
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
