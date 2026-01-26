import { MessageSquare, Clock, Copy, History, Trash2, Sparkles, Mic } from "lucide-react";
import type { HistoryRecord } from "../types";
import { formatTimestamp } from "../utils";
import type { MouseEvent as ReactMouseEvent } from "react";

export type HistoryPageProps = {
  history: HistoryRecord[];
  onCopyText: (text: string, e?: ReactMouseEvent) => void;
  onClear: () => void;
};

export function HistoryPage({ history, onCopyText, onClear }: HistoryPageProps) {
  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl overflow-hidden">
        <div className="px-6 py-5 border-b border-[var(--stone)] bg-[var(--paper)] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-[rgba(106,155,204,0.12)] rounded-xl text-[var(--steel)]">
              <History size={20} />
            </div>
            <div>
              <div className="text-lg font-bold text-[var(--ink)]">历史记录</div>
              <div className="text-xs text-[var(--stone-dark)]">共 {history.length} 条</div>
            </div>
          </div>
          {history.length > 0 && (
            <button
              onClick={onClear}
              className="px-4 py-2 rounded-2xl bg-red-50 text-red-700 border border-red-100 hover:bg-red-100 transition-colors text-sm font-bold flex items-center gap-2"
            >
              <Trash2 size={16} />
              清空
            </button>
          )}
        </div>

        <div className="p-6 space-y-3">
          {history.length === 0 ? (
            <div className="text-center text-stone-400 py-10">暂无历史记录</div>
          ) : (
            history.map((record) => (
              <div
                key={record.id}
                className={[
                  "p-5 rounded-2xl border transition-colors",
                  record.success
                    ? "bg-white border-[var(--stone)] hover:border-[rgba(176,174,165,0.75)]"
                    : "bg-red-50/60 border-red-100",
                ].join(" ")}
              >
                <div className="flex items-center justify-between mb-3">
                  <span className="text-xs text-stone-400 flex items-center gap-1">
                    <Clock size={12} />
                    {formatTimestamp(record.timestamp)}
                  </span>
                  {record.success ? (
                    <div className="flex items-center gap-2">
                      {record.mode === "assistant" ? (
                        <span className="text-[10px] bg-[rgba(59,130,246,0.12)] text-blue-500 px-1.5 py-0.5 rounded">
                          AI 助手
                        </span>
                      ) : record.presetName && (
                        <span className="text-[10px] bg-[rgba(217,119,87,0.12)] text-[var(--crail)] px-1.5 py-0.5 rounded">
                          {record.presetName}
                        </span>
                      )}
                      <span className="text-[10px] bg-stone-50 text-stone-600 px-1.5 py-0.5 rounded">
                        {(record.totalTimeMs / 1000).toFixed(1)}s
                      </span>
                    </div>
                  ) : (
                    <span className="text-[10px] bg-red-100 text-red-600 px-1.5 py-0.5 rounded">失败</span>
                  )}
                </div>

                {record.success ? (
                  record.polishedText ? (
                    <div className="grid grid-cols-2 gap-3">
                      <div className="flex flex-col min-h-0 bg-stone-50/60 rounded-xl p-3 border border-stone-200">
                        <div className="flex items-center justify-between mb-2">
                          <div className="text-xs font-semibold text-stone-500 tracking-wide flex items-center gap-1.5">
                            <Mic size={12} /> {record.mode === "assistant" ? "语音指令" : "原始转写"}
                          </div>
                          <button
                            onClick={(e) => onCopyText(record.originalText, e)}
                            className="p-1.5 rounded-xl bg-white border border-[var(--stone)] hover:border-[rgba(176,174,165,0.75)] text-stone-500 hover:text-[var(--steel)] transition-colors shadow-sm"
                            title="复制原始文本"
                          >
                            <Copy size={13} />
                          </button>
                        </div>
                        <p className="text-xs text-stone-600 line-clamp-4 leading-relaxed">{record.originalText}</p>
                      </div>

                      <div className={`flex flex-col min-h-0 rounded-xl p-3 border border-[var(--stone)] ${record.mode === "assistant"
                          ? "bg-[rgba(59,130,246,0.08)]"
                          : "bg-[rgba(217,119,87,0.08)]"
                        }`}>
                        <div className="flex items-center justify-between mb-2">
                          <div className={`text-xs font-semibold tracking-wide flex items-center gap-1.5 ${record.mode === "assistant" ? "text-blue-500" : "text-[var(--crail)]"
                            }`}>
                            {record.mode === "assistant" ? <MessageSquare size={12} /> : <Sparkles size={12} />}
                            {record.mode === "assistant" ? "AI 助手" : (record.presetName || "润色后")}
                          </div>
                          <button
                            onClick={(e) => onCopyText(record.polishedText!, e)}
                            className="p-1.5 rounded-xl bg-white border border-[var(--stone)] hover:border-[rgba(176,174,165,0.75)] text-stone-500 hover:text-[var(--steel)] transition-colors shadow-sm"
                            title="复制处理后文本"
                          >
                            <Copy size={13} />
                          </button>
                        </div>
                        <p className="text-xs text-stone-800 line-clamp-4 leading-relaxed font-semibold">{record.polishedText}</p>
                      </div>
                    </div>
                  ) : (
                    <div className="flex flex-col bg-stone-50/60 rounded-xl p-3 border border-stone-200">
                      <div className="flex items-center justify-between mb-2">
                        <div className="text-xs font-semibold text-stone-500 tracking-wide">转写结果</div>
                        <button
                          onClick={(e) => onCopyText(record.originalText, e)}
                          className="px-2.5 py-1.5 rounded-xl bg-white border border-[var(--stone)] hover:border-[rgba(176,174,165,0.75)] text-stone-700 hover:text-[var(--steel)] transition-colors flex items-center gap-1.5 shadow-sm"
                          title="复制文本"
                        >
                          <Copy size={14} />
                          <span className="text-xs font-medium">复制</span>
                        </button>
                      </div>
                      <p className="text-sm text-stone-800 line-clamp-5 leading-relaxed">{record.originalText}</p>
                    </div>
                  )
                ) : (
                  <p className="text-sm text-red-700 line-clamp-3">{record.errorMessage}</p>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
