import { AlertCircle, CheckCircle2, Plus } from "lucide-react";

export type DictionaryPageProps = {
  dictionary: string[];
  newWord: string;
  setNewWord: (next: string) => void;
  duplicateHint: boolean;
  setDuplicateHint: (next: boolean) => void;
  editingIndex: number | null;
  editingValue: string;
  setEditingValue: (next: string) => void;
  handleAddWord: () => void;
  handleDeleteWord: (index: number) => void;
  handleStartEdit: (index: number) => void;
  handleSaveEdit: () => void;
  handleCancelEdit: () => void;
  isRunning: boolean;
};

export function DictionaryPage({
  dictionary,
  newWord,
  setNewWord,
  duplicateHint,
  setDuplicateHint,
  editingIndex,
  editingValue,
  setEditingValue,
  handleAddWord,
  handleDeleteWord,
  handleStartEdit,
  handleSaveEdit,
  handleCancelEdit,
  isRunning,
}: DictionaryPageProps) {
  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-6">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <span>个人词典</span>
        </div>

        <div className="flex items-center gap-2 p-3 bg-[rgba(217,119,87,0.12)] border border-[rgba(217,119,87,0.22)] rounded-xl text-xs text-[var(--ink)]">
          <AlertCircle size={14} className="flex-shrink-0 text-[var(--crail)]" />
          <span>添加常用词汇（专业术语、人名、产品名等），提升语音识别准确率。</span>
        </div>

        <div className="space-y-2">
          <div className="flex gap-2">
            <input
              type="text"
              value={newWord}
              disabled={isRunning}
              onChange={(e) => {
                setNewWord(e.target.value);
                setDuplicateHint(false);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleAddWord();
              }}
              className={[
                "flex-1 px-4 py-3 bg-white border rounded-2xl text-sm focus:outline-none transition-colors",
                duplicateHint ? "border-red-300 focus:border-red-500" : "border-[var(--stone)] focus:border-[var(--steel)]",
                isRunning ? "opacity-60" : "",
              ].join(" ")}
              placeholder="输入词汇，按回车添加"
            />
            <button
              onClick={handleAddWord}
              disabled={!newWord.trim() || isRunning}
              className="px-4 py-3 bg-[var(--crail)] text-[var(--paper)] text-sm font-bold rounded-2xl hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            >
              <Plus size={16} />
              添加
            </button>
          </div>
          {duplicateHint && (
            <div className="flex items-center gap-2 text-xs text-red-600">
              <AlertCircle size={14} />
              <span>该词条已存在</span>
            </div>
          )}
          <div className="text-xs text-stone-400 font-semibold">共 {dictionary.length} 个词条</div>
        </div>

        <div className="flex flex-wrap gap-2">
          {dictionary.map((word, index) =>
            editingIndex === index ? (
              <div
                key={index}
                className="flex items-center gap-1 px-2 py-1 bg-white border-2 border-[var(--crail)] rounded-full shadow-sm"
              >
                <input
                  type="text"
                  value={editingValue}
                  disabled={isRunning}
                  onChange={(e) => setEditingValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSaveEdit();
                    if (e.key === "Escape") handleCancelEdit();
                  }}
                  className="w-28 px-2 py-0.5 bg-transparent text-sm focus:outline-none text-stone-700 disabled:opacity-60"
                  autoFocus
                />
                <button
                  onClick={handleSaveEdit}
                  disabled={isRunning}
                  className="p-0.5 text-[var(--sage)] hover:opacity-80 disabled:opacity-50"
                  title="保存"
                >
                  <CheckCircle2 size={14} />
                </button>
                <button
                  onClick={handleCancelEdit}
                  disabled={isRunning}
                  className="p-0.5 text-stone-400 hover:text-stone-600 disabled:opacity-50"
                  title="取消"
                >
                  ×
                </button>
              </div>
            ) : (
              <div
                key={index}
                className="group flex items-center gap-1.5 px-3 py-1.5 bg-white border border-[var(--stone)] rounded-full text-sm text-stone-700 hover:border-[rgba(217,119,87,0.35)] hover:shadow-sm transition-colors cursor-default"
              >
                <span className="font-semibold" onDoubleClick={() => !isRunning && handleStartEdit(index)}>
                  {word}
                </span>
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onClick={() => handleStartEdit(index)}
                    disabled={isRunning}
                    className="p-0.5 text-stone-400 hover:text-[var(--steel)] transition-colors disabled:opacity-50"
                    title="编辑"
                  >
                    ✎
                  </button>
                  <button
                    onClick={() => handleDeleteWord(index)}
                    disabled={isRunning}
                    className="p-0.5 text-stone-400 hover:text-red-600 transition-colors disabled:opacity-50"
                    title="删除"
                  >
                    ×
                  </button>
                </div>
              </div>
            ),
          )}
        </div>
      </div>
    </div>
  );
}
