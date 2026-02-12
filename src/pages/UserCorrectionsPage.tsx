import { Sparkles, Trash2 } from "lucide-react";
import { useState } from "react";
import type { UserCorrectionRecord } from "../types";

export type UserCorrectionsPageProps = {
  records: UserCorrectionRecord[];
  onUpdateAt: (index: number, correctedText: string) => Promise<boolean>;
  onDeleteAt: (index: number) => void;
  onClear: () => void;
};

export function UserCorrectionsPage({
  records,
  onUpdateAt,
  onDeleteAt,
  onClear,
}: UserCorrectionsPageProps) {
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editingValue, setEditingValue] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const displayedRecords = [...records].reverse();

  const beginEdit = (index: number, correctedText: string) => {
    if (isSaving) return;
    setEditingIndex(index);
    setEditingValue(correctedText);
  };

  const cancelEdit = () => {
    if (isSaving) return;
    setEditingIndex(null);
    setEditingValue("");
  };

  const submitEdit = async () => {
    if (editingIndex === null || isSaving) return;
    setIsSaving(true);
    const ok = await onUpdateAt(editingIndex, editingValue);
    setIsSaving(false);
    if (ok) {
      setEditingIndex(null);
      setEditingValue("");
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl overflow-hidden">
        <div className="px-6 py-5 border-b border-[var(--stone)] bg-[var(--paper)] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-[rgba(217,119,87,0.12)] rounded-xl text-[var(--crail)]">
              <Sparkles size={20} />
            </div>
            <div>
              <div className="text-lg font-bold text-[var(--ink)]">纠错记录</div>
              <div className="text-xs text-[var(--stone-dark)]">共 {records.length} 条</div>
            </div>
          </div>
          {records.length > 0 && (
            <button
              type="button"
              onClick={() => {
                if (window.confirm("确认清空全部纠错记录？")) {
                  onClear();
                }
              }}
              className="px-4 py-2 rounded-2xl bg-red-50 text-red-700 border border-red-100 hover:bg-red-100 transition-colors text-sm font-bold flex items-center gap-2"
            >
              <Trash2 size={16} />
              清空
            </button>
          )}
        </div>

        <div className="p-6 space-y-3">
          {displayedRecords.length === 0 ? (
            <div className="text-center text-stone-400 py-10">暂无纠错记录</div>
          ) : (
            displayedRecords.map((record, displayIndex) => {
              const sourceIndex = records.length - 1 - displayIndex;
              const isEditing = editingIndex === sourceIndex;
              return (
                <div
                  key={`${record.origin_text}-${sourceIndex}`}
                  className="p-4 rounded-2xl border border-[var(--stone)] bg-white hover:border-[rgba(176,174,165,0.75)] transition-colors"
                >
                  <div className="mb-3 flex items-center justify-between">
                    <span className="text-xs text-stone-500 font-semibold">
                      记录 #{records.length - displayIndex}
                    </span>
                    <div className="flex items-center gap-2">
                      {isEditing ? (
                        <>
                          <button
                            type="button"
                            onClick={() => void submitEdit()}
                            disabled={isSaving}
                            className="px-2 py-1 rounded-lg border border-[var(--crail)] bg-[var(--crail)] text-[11px] font-semibold text-white hover:opacity-90 disabled:opacity-50"
                            title="保存纠正文"
                          >
                            {isSaving ? "保存中..." : "保存"}
                          </button>
                          <button
                            type="button"
                            onClick={cancelEdit}
                            disabled={isSaving}
                            className="px-2 py-1 rounded-lg border border-[var(--stone)] bg-white text-[11px] font-semibold text-stone-600 hover:text-[var(--steel)] hover:border-[rgba(176,174,165,0.75)] disabled:opacity-50"
                            title="取消编辑"
                          >
                            取消
                          </button>
                        </>
                      ) : (
                        <button
                          type="button"
                          onClick={() => beginEdit(sourceIndex, record.corrected_text)}
                          className="px-2 py-1 rounded-lg border border-[var(--stone)] bg-white text-[11px] font-semibold text-stone-600 hover:text-[var(--steel)] hover:border-[rgba(176,174,165,0.75)]"
                          title="编辑纠正文"
                        >
                          编辑
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => onDeleteAt(sourceIndex)}
                        className="px-2 py-1 rounded-lg border border-red-100 bg-red-50 text-[11px] font-semibold text-red-700 hover:bg-red-100"
                        title="删除这条记录"
                      >
                        删除
                      </button>
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div className="rounded-xl border border-stone-200 bg-stone-50/60 p-3">
                      <div className="mb-1 text-[11px] font-bold text-stone-500">原文</div>
                      <p className="text-xs leading-relaxed whitespace-pre-wrap break-words text-stone-700">
                        {record.origin_text}
                      </p>
                    </div>
                    <div className="rounded-xl border border-[rgba(217,119,87,0.35)] bg-[rgba(217,119,87,0.08)] p-3">
                      <div className="mb-1 text-[11px] font-bold text-[var(--crail)]">纠正文</div>
                      {isEditing ? (
                        <textarea
                          value={editingValue}
                          onChange={(event) => setEditingValue(event.target.value)}
                          onKeyDown={(event) => {
                            if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
                              event.preventDefault();
                              void submitEdit();
                            }
                            if (event.key === "Escape") {
                              event.preventDefault();
                              cancelEdit();
                            }
                          }}
                          className="w-full h-20 resize-none rounded-lg border border-[var(--stone)] bg-white px-2 py-1.5 text-xs text-stone-800 outline-none focus:border-[var(--crail)]"
                          disabled={isSaving}
                          placeholder="请输入新的纠正文"
                        />
                      ) : (
                        <p className="text-xs leading-relaxed whitespace-pre-wrap break-words text-stone-800">
                          {record.corrected_text}
                        </p>
                      )}
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
