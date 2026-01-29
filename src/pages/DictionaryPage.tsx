import { useEffect, useMemo, useRef, useState } from "react";
import { AlertCircle, CheckCircle2, Plus, Trash2 } from "lucide-react";
import { SourceBadge } from "../components/learning/SourceBadge";
import type { DictionaryEntry } from "../types";
import {
  BUILTIN_DICTIONARY_DOMAINS,
  BUILTIN_DICTIONARY_LIMIT,
  getBuiltinWordsForDomains,
} from "../utils/builtinDictionary";

export type DictionaryPageProps = {
  dictionary: DictionaryEntry[];
  newWord: string;
  setNewWord: (next: string) => void;
  duplicateHint: boolean;
  setDuplicateHint: (next: boolean) => void;
  editingIndex: number | null;
  editingValue: string;
  setEditingValue: (next: string) => void;
  handleAddWord: () => void;
  handleDeleteWord: (id: string) => void;
  handleStartEdit: (index: number) => void;
  handleSaveEdit: () => void;
  handleCancelEdit: () => void;
  handleBatchDelete: (ids: string[]) => void;
  builtinDictionaryDomains: string[];
  setBuiltinDictionaryDomains: (next: string[]) => void;
  isRunning: boolean;
};

type FilterType = "all" | "manual" | "auto";
type DictionaryTab = "personal" | "builtin";

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
  handleDeleteWord: _handleDeleteWord,
  handleStartEdit,
  handleSaveEdit,
  handleCancelEdit,
  handleBatchDelete,
  builtinDictionaryDomains,
  setBuiltinDictionaryDomains,
  isRunning,
}: DictionaryPageProps) {
  const [activeTab, setActiveTab] = useState<DictionaryTab>("personal");
  const [filter, setFilter] = useState<FilterType>("all");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const selectionErrorTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (selectionErrorTimeoutRef.current) {
        window.clearTimeout(selectionErrorTimeoutRef.current);
      }
    };
  }, []);

  const selectedDomainSet = useMemo(
    () => new Set(builtinDictionaryDomains),
    [builtinDictionaryDomains]
  );
  const selectedBuiltinWordCount = useMemo(
    () => getBuiltinWordsForDomains(builtinDictionaryDomains).length,
    [builtinDictionaryDomains]
  );

  const showSelectionError = (message: string) => {
    setSelectionError(message);
    if (selectionErrorTimeoutRef.current) {
      window.clearTimeout(selectionErrorTimeoutRef.current);
    }
    selectionErrorTimeoutRef.current = window.setTimeout(() => {
      setSelectionError(null);
    }, 2000);
  };

  // 筛选词条
  const filteredDictionary = dictionary.filter((entry) => {
    if (filter === "all") return true;
    return entry.source === filter;
  });

  // 统计
  const manualCount = dictionary.filter((e) => e.source === "manual").length;
  const autoCount = dictionary.filter((e) => e.source === "auto").length;

  // 切换选择
  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  // 全选/取消全选
  const toggleSelectAll = () => {
    if (selectedIds.size === filteredDictionary.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(filteredDictionary.map((e) => e.id)));
    }
  };

  // 批量删除
  const handleBatchDeleteClick = () => {
    if (selectedIds.size === 0) return;
    // 将 ID 映射为 word（后端按 word 匹配删除）
    const wordsToDelete = dictionary
      .filter((e) => selectedIds.has(e.id))
      .map((e) => e.word);
    handleBatchDelete(wordsToDelete);
    setSelectedIds(new Set());
  };

  const handleToggleBuiltinDomain = (domainName: string) => {
    if (isRunning) return;
    if (selectedDomainSet.has(domainName)) {
      setBuiltinDictionaryDomains(
        builtinDictionaryDomains.filter((name) => name !== domainName)
      );
      return;
    }

    if (builtinDictionaryDomains.length >= BUILTIN_DICTIONARY_LIMIT) {
      showSelectionError(`最多可选择 ${BUILTIN_DICTIONARY_LIMIT} 个领域`);
      return;
    }

    setBuiltinDictionaryDomains([...builtinDictionaryDomains, domainName]);
  };

  const handleClearBuiltinDomains = () => {
    if (isRunning || builtinDictionaryDomains.length === 0) return;
    setBuiltinDictionaryDomains([]);
  };

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-6">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
            <span>词库</span>
          </div>
          <div className="flex items-center gap-1 text-xs font-semibold">
            <button
              onClick={() => setActiveTab("personal")}
              className={`px-3 py-1.5 rounded-full transition-colors ${activeTab === "personal"
                ? "bg-[var(--ink)] text-white"
                : "bg-stone-100 text-stone-600 hover:bg-stone-200"
                }`}
            >
              个人词库
            </button>
            <button
              onClick={() => setActiveTab("builtin")}
              className={`px-3 py-1.5 rounded-full transition-colors ${activeTab === "builtin"
                ? "bg-[var(--ink)] text-white"
                : "bg-stone-100 text-stone-600 hover:bg-stone-200"
                }`}
            >
              内置词库
            </button>
          </div>
        </div>

        {activeTab === "personal" && (
          <>
            <div className="flex items-center gap-2 p-3 bg-[rgba(120,113,108,0.08)] border border-[rgba(120,113,108,0.18)] rounded-xl text-xs text-[var(--ink)]">
              <AlertCircle size={14} className="flex-shrink-0 text-[var(--ink)]" />
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
            </div>

            {/* 筛选器 + 统计 */}
            <div className="flex items-center justify-between">
              <div className="flex gap-2">
                <button
                  onClick={() => setFilter("all")}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-full transition-colors ${filter === "all"
                    ? "bg-[var(--ink)] text-white"
                    : "bg-stone-100 text-stone-600 hover:bg-stone-200"
                    }`}
                >
                  全部 ({dictionary.length})
                </button>
                <button
                  onClick={() => setFilter("manual")}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-full transition-colors ${filter === "manual"
                    ? "bg-[var(--steel)] text-white"
                    : "bg-stone-100 text-stone-600 hover:bg-stone-200"
                    }`}
                >
                  手动 ({manualCount})
                </button>
                <button
                  onClick={() => setFilter("auto")}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-full transition-colors ${filter === "auto"
                    ? "bg-[var(--sage)] text-white"
                    : "bg-stone-100 text-stone-600 hover:bg-stone-200"
                    }`}
                >
                  自动 ({autoCount})
                </button>
              </div>

              {/* 批量操作 */}
              {selectedIds.size > 0 && (
                <div className="flex items-center gap-2">
                  <span className="text-xs text-stone-500">已选 {selectedIds.size} 项</span>
                  <button
                    onClick={handleBatchDeleteClick}
                    disabled={isRunning}
                    className="flex items-center gap-1 px-3 py-1.5 text-xs font-semibold text-red-600 bg-red-50 rounded-full hover:bg-red-100 transition-colors disabled:opacity-50"
                  >
                    <Trash2 size={12} />
                    删除
                  </button>
                </div>
              )}
            </div>

            {filteredDictionary.length > 0 && (
              <div className="flex items-center justify-between">
                <button
                  onClick={toggleSelectAll}
                  disabled={isRunning}
                  className="text-xs font-bold text-stone-400 hover:text-[var(--crail)] transition-colors flex items-center gap-1.5"
                >
                  <div className={`w-3.5 h-3.5 rounded border flex items-center justify-center transition-colors ${selectedIds.size === filteredDictionary.length && filteredDictionary.length > 0
                      ? "bg-[var(--crail)] border-[var(--crail)]"
                      : "border-stone-300"
                    }`}>
                    {selectedIds.size === filteredDictionary.length && filteredDictionary.length > 0 && (
                      <CheckCircle2 size={10} className="text-white" />
                    )}
                  </div>
                  全选当前列表
                </button>
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              {filteredDictionary.map((entry, index) =>
                editingIndex === index ? (
                  <div
                    key={entry.id}
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
                    key={entry.id}
                    onClick={() => !isRunning && toggleSelect(entry.id)}
                    className={`group relative flex items-center gap-1.5 px-3 py-1.5 border rounded-full text-sm transition-all cursor-pointer select-none ${selectedIds.has(entry.id)
                      ? "border-[var(--crail)] bg-[rgba(217,119,87,0.08)] text-[var(--crail)]"
                      : "border-stone-200 bg-white text-stone-700 hover:border-stone-300 hover:bg-stone-50"
                      }`}
                  >
                    {selectedIds.has(entry.id) && (
                      <CheckCircle2 size={14} className="text-[var(--crail)] animate-in fade-in zoom-in duration-200" />
                    )}
                    <span className="font-semibold" onDoubleClick={(e) => {
                      e.stopPropagation();
                      !isRunning && handleStartEdit(index)
                    }}>
                      {entry.word}
                    </span>
                    <SourceBadge source={entry.source} />
                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity ml-1">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleStartEdit(index);
                        }}
                        disabled={isRunning}
                        className="p-0.5 text-stone-400 hover:text-[var(--steel)] transition-colors disabled:opacity-50"
                        title="编辑"
                      >
                        ✎
                      </button>
                    </div>
                  </div>
                ),
              )}
            </div>

            {filteredDictionary.length === 0 && (
              <div className="text-center py-8 text-stone-400 text-sm">
                {filter === "all" ? "暂无词条，开始添加吧" : `暂无${filter === "manual" ? "手动" : "自动"}添加的词条`}
              </div>
            )}
          </>
        )}

        {activeTab === "builtin" && (
          <div className="space-y-4">
            <div className="flex items-center gap-2 p-3 bg-[rgba(120,113,108,0.08)] border border-[rgba(120,113,108,0.18)] rounded-xl text-xs text-[var(--ink)]">
              <AlertCircle size={14} className="flex-shrink-0 text-[var(--sage)]" />
              <span>选择领域词库后，将与个人词库一起用于识别与纠错。最多选择 5 个领域。</span>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
              <span className="text-stone-500">
                已选 {builtinDictionaryDomains.length}/{BUILTIN_DICTIONARY_LIMIT} 个领域 · 共 {selectedBuiltinWordCount} 个词条
              </span>
              <button
                onClick={handleClearBuiltinDomains}
                disabled={isRunning || builtinDictionaryDomains.length === 0}
                className="text-stone-400 hover:text-[var(--crail)] transition-colors disabled:opacity-40"
              >
                清空选择
              </button>
            </div>

            {selectionError && (
              <div className="flex items-center gap-2 text-xs text-red-600">
                <AlertCircle size={14} />
                <span>{selectionError}</span>
              </div>
            )}

            {BUILTIN_DICTIONARY_DOMAINS.length === 0 ? (
              <div className="text-center py-8 text-stone-400 text-sm">
                未加载到内置词库
              </div>
            ) : (
              <div className="flex gap-4">
                {/* 左侧：领域列表 */}
                <div className="w-1/3 flex-shrink-0 space-y-2">
                  <div className="text-xs font-semibold text-stone-500 uppercase tracking-wide mb-2">
                    领域列表
                  </div>
                  <div className="space-y-2 max-h-80 overflow-y-auto pr-1">
                    {BUILTIN_DICTIONARY_DOMAINS.map((domain) => {
                      const isSelected = selectedDomainSet.has(domain.name);
                      const isDisabled =
                        isRunning ||
                        (!isSelected &&
                          builtinDictionaryDomains.length >= BUILTIN_DICTIONARY_LIMIT);

                      return (
                        <button
                          key={domain.name}
                          onClick={() => handleToggleBuiltinDomain(domain.name)}
                          disabled={isDisabled}
                          className={`group w-full text-left rounded-xl border px-3 py-2 transition-colors ${isSelected
                            ? "border-[var(--crail)] bg-[rgba(217,119,87,0.08)] text-[var(--crail)]"
                            : "border-stone-200 bg-white text-stone-700 hover:border-stone-300 hover:bg-stone-50"
                            } ${isDisabled ? "opacity-60 cursor-not-allowed" : ""}`}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-sm font-semibold">{domain.name}</span>
                            {isSelected && (
                              <CheckCircle2 size={14} className="text-[var(--crail)]" />
                            )}
                          </div>
                          <span className="text-xs text-stone-500">{domain.words.length} 词</span>
                        </button>
                      );
                    })}
                  </div>
                </div>

                {/* 右侧：词库预览 */}
                <div className="flex-1 min-w-0">
                  <div className="text-xs font-semibold text-stone-500 uppercase tracking-wide mb-2">
                    词库预览
                  </div>
                  <div className="border border-stone-200 rounded-xl p-3 bg-stone-50 max-h-80 overflow-y-auto">
                    {builtinDictionaryDomains.length === 0 ? (
                      <div className="text-center py-8 text-stone-400 text-sm">
                        请从左侧选择领域以预览词库
                      </div>
                    ) : (
                      <div className="flex flex-wrap gap-1.5">
                        {getBuiltinWordsForDomains(builtinDictionaryDomains).map((word) => (
                          <span
                            key={word}
                            className="px-2 py-1 text-xs font-medium bg-white border border-stone-200 rounded-full text-stone-600"
                          >
                            {word}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
