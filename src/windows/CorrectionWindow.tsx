import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";

type ManualCorrectionPayload = {
  origin_text: string;
};

export default function CorrectionWindow() {
  const [originText, setOriginText] = useState("");
  const [correctedText, setCorrectedText] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const correctionWindowRef = useRef(getCurrentWindow());
  const lastWindowHeightRef = useRef<number | null>(null);

  const resizeWindowToFitContent = useCallback(() => {
    const content = contentRef.current;
    if (!content) return;

    const WINDOW_WIDTH = 420;
    const MIN_HEIGHT = 320;
    const MAX_HEIGHT = 520;
    const nextHeight = Math.min(
      MAX_HEIGHT,
      Math.max(MIN_HEIGHT, Math.ceil(content.scrollHeight + 16)),
    );

    if (lastWindowHeightRef.current === nextHeight) return;
    lastWindowHeightRef.current = nextHeight;

    void correctionWindowRef.current
      .setSize(new LogicalSize(WINDOW_WIDTH, nextHeight))
      .catch((error) => {
        console.warn("调整用户纠错窗口高度失败:", error);
      });
  }, []);

  const handleCancel = useCallback(async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    setErrorMessage(null);
    try {
      await invoke("cancel_manual_correction");
    } catch (error) {
      console.error("取消用户纠错失败:", error);
      setErrorMessage(String(error));
      setIsSubmitting(false);
    }
  }, [isSubmitting]);

  const handleSubmit = useCallback(async () => {
    if (isSubmitting) return;
    const trimmed = correctedText.trim();
    if (!trimmed) {
      setErrorMessage("纠错文本不能为空");
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);
    try {
      await invoke("submit_manual_correction", { correctedText: trimmed });
    } catch (error) {
      console.error("提交用户纠错失败:", error);
      setErrorMessage(String(error));
      setIsSubmitting(false);
    }
  }, [correctedText, isSubmitting]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    const setupListener = async () => {
      unlisten = await listen<ManualCorrectionPayload>(
        "manual_correction_requested",
        (event) => {
          const nextOriginText = event.payload?.origin_text ?? "";
          setOriginText(nextOriginText);
          setCorrectedText(nextOriginText);
          setIsSubmitting(false);
          setErrorMessage(null);

          requestAnimationFrame(() => {
            textareaRef.current?.focus();
            textareaRef.current?.setSelectionRange(0, nextOriginText.length);
          });
        },
      );
      if (cancelled && unlisten) {
        unlisten();
        unlisten = undefined;
      }
    };

    void setupListener();

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void handleCancel();
        return;
      }
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault();
        void handleSubmit();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handleCancel, handleSubmit]);

  useEffect(() => {
    const rafId = window.requestAnimationFrame(() => {
      resizeWindowToFitContent();
    });
    return () => window.cancelAnimationFrame(rafId);
  }, [errorMessage, originText, isSubmitting, resizeWindowToFitContent]);

  return (
    <div className="w-full p-2 overflow-hidden">
      <div
        ref={contentRef}
        className="w-full custom-scroll rounded-xl border border-[var(--stone)] bg-white shadow-lg p-3 space-y-2 font-sans"
      >
        <div className="space-y-0.5">
          <h2 className="text-xs font-bold text-[var(--ink)]">用户自主纠错</h2>
          <p className="text-[10px] text-stone-500 font-medium">Esc 取消 · Ctrl+Enter 提交</p>
        </div>

        <div className="rounded-lg border border-[var(--stone)] bg-[var(--paper)] px-2.5 py-2">
          <div className="text-[10px] font-bold text-stone-500">原文</div>
          <p className="mt-1 max-h-14 overflow-auto text-[11px] text-stone-600 whitespace-pre-wrap break-words">
            {originText || "（暂无选中文本）"}
          </p>
        </div>

        <div className="space-y-1">
          <label htmlFor="manual-correction-input" className="text-[10px] font-bold text-stone-500">
            纠正文
          </label>
          <textarea
            id="manual-correction-input"
            ref={textareaRef}
            value={correctedText}
            onChange={(event) => setCorrectedText(event.target.value)}
            className="w-full h-[86px] resize-none rounded-lg border border-[var(--stone)] bg-white px-2.5 py-2 text-xs text-[var(--ink)] outline-none focus:border-[var(--crail)]"
            placeholder="请输入纠正后的文本"
            disabled={isSubmitting}
          />
        </div>

        {errorMessage && (
          <div className="rounded-lg border border-red-200 bg-red-50 px-2.5 py-2 text-[10px] font-semibold text-red-600">
            {errorMessage}
          </div>
        )}

        <div className="flex items-center justify-end gap-1.5">
          <button
            type="button"
            className="px-2.5 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-[11px] font-bold text-stone-600 hover:bg-stone-50 disabled:opacity-50"
            onClick={() => void handleCancel()}
            disabled={isSubmitting}
          >
            取消
          </button>
          <button
            type="button"
            className="px-2.5 py-1.5 rounded-lg border border-[var(--crail)] bg-[var(--crail)] text-[11px] font-bold text-white hover:opacity-90 disabled:opacity-50"
            onClick={() => void handleSubmit()}
            disabled={isSubmitting}
          >
            {isSubmitting ? "提交中..." : "确认纠错"}
          </button>
        </div>
      </div>
    </div>
  );
}
