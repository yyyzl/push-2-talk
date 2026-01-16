import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { HistoryRecord } from "../types";
import { loadHistory, saveHistory } from "../utils";

export type UseHistoryControllerResult = {
  history: HistoryRecord[];
  setHistory: React.Dispatch<React.SetStateAction<HistoryRecord[]>>;

  showHistory: boolean;
  setShowHistory: React.Dispatch<React.SetStateAction<boolean>>;

  copyToast: string | null;
  showToast: (message: string, durationMs?: number) => void;

  handleCopyText: (text: string, e?: React.MouseEvent) => void;
  handleClearHistory: () => void;
};

export function useHistoryController(): UseHistoryControllerResult {
  const [history, setHistory] = useState<HistoryRecord[]>(() => loadHistory());
  const [showHistory, setShowHistory] = useState(false);
  const [copyToast, setCopyToast] = useState<string | null>(null);
  const toastTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (toastTimeoutRef.current) {
        window.clearTimeout(toastTimeoutRef.current);
      }
    };
  }, []);

  const showToast = useCallback((message: string, durationMs: number = 2000) => {
    setCopyToast(message);
    if (toastTimeoutRef.current) {
      window.clearTimeout(toastTimeoutRef.current);
    }
    toastTimeoutRef.current = window.setTimeout(() => setCopyToast(null), durationMs);
  }, []);

  const handleCopyText = useCallback(
    (text: string, e?: React.MouseEvent) => {
      if (e) e.stopPropagation();
      void navigator.clipboard.writeText(text);
      showToast("已复制到剪贴板");
    },
    [showToast],
  );

  const handleClearHistory = useCallback(() => {
    setHistory([]);
    saveHistory([]);
  }, []);

  return {
    history,
    setHistory,
    showHistory,
    setShowHistory,
    copyToast,
    showToast,
    handleCopyText,
    handleClearHistory,
  };
}

