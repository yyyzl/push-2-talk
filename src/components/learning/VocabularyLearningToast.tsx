import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, X, Sparkles } from "lucide-react";
import { DiffDisplay } from "./DiffDisplay";
import type { VocabularyLearningSuggestion } from "../../types";

interface VocabularyLearningToastProps {
  suggestion: VocabularyLearningSuggestion;
  onDismiss: () => void;
  onAdd: () => void;
}

export function VocabularyLearningToast({
  suggestion,
  onDismiss,
  onAdd,
}: VocabularyLearningToastProps) {
  const [isExiting, setIsExiting] = useState(false);
  const [countdown, setCountdown] = useState(5);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isPaused, setIsPaused] = useState(false);

  // 处理添加
  const handleAdd = useCallback(async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);

    try {
      await invoke("add_learned_word", {
        word: suggestion.word,
        source: "auto",
      });
      console.log("词汇已添加:", suggestion.word);
      setIsExiting(true);
      setTimeout(onAdd, 300);
    } catch (error) {
      console.error("添加词汇失败:", error);
      setIsSubmitting(false);
    }
  }, [suggestion.word, onAdd, isSubmitting]);

  // 处理忽略
  const handleDismiss = useCallback(async () => {
    try {
      await invoke("dismiss_learning_suggestion", { id: suggestion.id });
    } catch (error) {
      console.error("忽略建议失败:", error);
    }
    setIsExiting(true);
    setTimeout(onDismiss, 300);
  }, [suggestion.id, onDismiss]);

  // 自动消失倒计时（鼠标悬停时暂停）
  useEffect(() => {
    const timer = setInterval(() => {
      // 暂停时不递减
      if (isPaused) return;

      setCountdown((prev) => {
        if (prev <= 1) {
          clearInterval(timer);
          // 直接调用 onDismiss 和 invoke，避免依赖 handleDismiss
          invoke("dismiss_learning_suggestion", { id: suggestion.id }).catch(console.error);
          setIsExiting(true);
          setTimeout(onDismiss, 300);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(timer);
  }, [suggestion.id, onDismiss, isPaused]);

  // 键盘支持
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAdd();
      } else if (e.key === "Escape") {
        e.preventDefault();
        handleDismiss();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleAdd, handleDismiss]);

  // 分类标签
  const categoryLabels: Record<string, string> = {
    proper_noun: "专有名词",
    term: "专业术语",
    frequent: "高频词汇",
  };

  return (
    <div
      className={`
        w-80 bg-white border border-[var(--stone)] rounded-2xl shadow-lg
        transform transition-all duration-300 ease-out
        ${isExiting ? "opacity-0 translate-x-4" : "opacity-100 translate-x-0"}
      `}
      role="alert"
      aria-live="assertive"
      onMouseEnter={() => setIsPaused(true)}
      onMouseLeave={() => setIsPaused(false)}
    >
      {/* 头部 */}
      <div className="flex items-center justify-between px-4 pt-4 pb-2">
        <div className="flex items-center gap-2">
          <Sparkles size={16} className="text-[var(--sage)]" aria-hidden="true" />
          <span className="text-xs font-bold text-stone-500 uppercase tracking-widest">
            学习建议
          </span>
        </div>
        <span className="text-xs text-stone-400">{countdown}s</span>
      </div>

      {/* 内容 */}
      <div className="px-4 pb-3 space-y-3">
        {/* 词汇 */}
        <div className="flex items-center gap-2">
          <span className="text-lg font-bold text-[var(--ink)]">{suggestion.word}</span>
          <span className="px-2 py-0.5 text-xs font-medium bg-[rgba(120,140,93,0.12)] text-[var(--sage)] rounded-full">
            {categoryLabels[suggestion.category] || suggestion.category}
          </span>
        </div>

        {/* 差异对比 */}
        <DiffDisplay original={suggestion.original} corrected={suggestion.corrected} />

        {/* 原因 */}
        <p className="text-xs text-stone-500 leading-relaxed">{suggestion.reason}</p>
      </div>

      {/* 操作按钮 */}
      <div className="flex border-t border-[var(--stone)]">
        <button
          onClick={handleDismiss}
          className="flex-1 flex items-center justify-center gap-2 py-3 text-sm font-medium text-stone-500 hover:bg-stone-50 transition-colors rounded-bl-2xl"
          aria-label="忽略此建议"
        >
          <X size={16} aria-hidden="true" />
          忽略
        </button>
        <div className="w-px bg-[var(--stone)]" />
        <button
          onClick={handleAdd}
          disabled={isSubmitting}
          className="flex-1 flex items-center justify-center gap-2 py-3 text-sm font-bold text-[var(--sage)] hover:bg-[rgba(120,140,93,0.08)] transition-colors rounded-br-2xl disabled:opacity-50"
          aria-label="添加到词典"
        >
          <Check size={16} aria-hidden="true" />
          添加
        </button>
      </div>
    </div>
  );
}
