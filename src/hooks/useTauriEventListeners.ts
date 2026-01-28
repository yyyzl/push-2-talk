import type React from "react";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { nanoid } from "nanoid";
import type { AppConfig, AppStatus, HistoryRecord, LlmConfig, TranscriptionResult, UsageStats } from "../types";
import { MAX_HISTORY } from "../constants";
import { saveHistory, loadUsageStats } from "../utils";

type UnlistenFn = () => void;

export type UseTauriEventListenersParams = {
  llmConfigRef: React.RefObject<LlmConfig>;
  enablePostProcessRef?: React.RefObject<boolean>;
  enableDictionaryEnhancementRef?: React.RefObject<boolean>;

  setStatus: React.Dispatch<React.SetStateAction<AppStatus>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  setTranscript: React.Dispatch<React.SetStateAction<string>>;
  setOriginalTranscript: React.Dispatch<React.SetStateAction<string | null>>;
  setCurrentMode: React.Dispatch<React.SetStateAction<string | null>>;
  setAsrTime: React.Dispatch<React.SetStateAction<number | null>>;
  setLlmTime: React.Dispatch<React.SetStateAction<number | null>>;
  setTotalTime: React.Dispatch<React.SetStateAction<number | null>>;
  setShowCloseDialog: React.Dispatch<React.SetStateAction<boolean>>;

  setHistory: React.Dispatch<React.SetStateAction<HistoryRecord[]>>;
  setUsageStats?: React.Dispatch<React.SetStateAction<UsageStats>>;
};

export function useTauriEventListeners({
  llmConfigRef,
  enablePostProcessRef,
  enableDictionaryEnhancementRef,
  setStatus,
  setError,
  setTranscript,
  setOriginalTranscript,
  setCurrentMode,
  setAsrTime,
  setLlmTime,
  setTotalTime,
  setShowCloseDialog,
  setHistory,
  setUsageStats,
}: UseTauriEventListenersParams) {
  useEffect(() => {
    let unlistenFns: UnlistenFn[] = [];
    let cancelled = false;

    const addHistoryRecord = (record: HistoryRecord) => {
      setHistory((prev) => {
        const updated = [record, ...prev].slice(0, MAX_HISTORY);
        saveHistory(updated);
        return updated;
      });
    };

    // 从后端重新加载统计数据（后端已自动更新）
    const reloadUsageStats = async () => {
      if (!setUsageStats) return;
      try {
        const stats = await loadUsageStats();
        setUsageStats(stats);
      } catch (error) {
        console.error('重新加载统计数据失败:', error);
      }
    };

    const setup = async () => {
      // 辅助函数：注册监听器并检查取消状态，解决 StrictMode 竞态条件
      const registerListener = async <T>(
        event: string,
        handler: (payload: T) => void,
      ): Promise<boolean> => {
        const unlisten = await listen<T>(event, (e) => handler(e.payload as T));
        if (cancelled) {
          unlisten();
          return false;
        }
        unlistenFns.push(unlisten);
        return true;
      };

      try {
        if (!(await registerListener("recording_started", () => {
          setStatus("recording");
          setError(null);
        }))) return;

        if (!(await registerListener("recording_stopped", () => {
          setStatus("transcribing");
        }))) return;

        if (!(await registerListener("transcribing", () => {
          setStatus("transcribing");
        }))) return;

        if (!(await registerListener<string>("post_processing", (mode) => {
          if (mode === "polishing") {
            setStatus("polishing");
          } else if (mode === "assistant") {
            setStatus("assistant_processing");
          }
        }))) return;

        if (!(await registerListener<TranscriptionResult>("transcription_complete", (result) => {
          setTranscript(result.text);
          setOriginalTranscript(result.original_text);
          setCurrentMode(result.mode || null);
          setAsrTime(result.asr_time_ms);
          setLlmTime(result.llm_time_ms);
          setTotalTime(result.total_time_ms);
          setStatus("running");

          // 后端已自动更新统计数据，前端只需重新加载
          reloadUsageStats();

          const llmConfig = llmConfigRef.current;
          const mode = (result.mode as "normal" | "assistant") || null;
          const enablePostProcess = enablePostProcessRef?.current ?? true;
          const enableDictionaryEnhancement = enableDictionaryEnhancementRef?.current ?? false;
          const presetName = result.original_text && mode !== "assistant"
            ? enablePostProcess
              ? llmConfig?.presets.find((p) => p.id === llmConfig.active_preset_id)?.name || null
              : (enableDictionaryEnhancement ? "词库增强" : null)
            : null;

          addHistoryRecord({
            id: nanoid(8),
            timestamp: Date.now(),
            originalText: result.original_text || result.text,
            polishedText: result.original_text ? result.text : null,
            presetName,
            mode,
            asrTimeMs: result.asr_time_ms,
            llmTimeMs: result.llm_time_ms,
            totalTimeMs: result.total_time_ms,
            success: true,
            errorMessage: null,
          });
        }))) return;

        if (!(await registerListener<string>("error", (errMsg) => {
          setError(errMsg);
          setStatus("running");

          // 注意：后端在错误情况下不会更新统计数据（只统计成功的录音）
          // 这里重新加载是为了保持UI状态同步，但数据不会变化
          reloadUsageStats();

          addHistoryRecord({
            id: nanoid(8),
            timestamp: Date.now(),
            originalText: "",
            polishedText: null,
            presetName: null,
            mode: null,
            asrTimeMs: 0,
            llmTimeMs: null,
            totalTimeMs: 0,
            success: false,
            errorMessage: errMsg,
          });
        }))) return;

        if (!(await registerListener("transcription_cancelled", () => {
          setStatus("running");
          setError(null);
        }))) return;

        if (!(await registerListener("close_requested", async () => {
          try {
            const config = await invoke<AppConfig>("load_config");
            if (config.close_action === "close") {
              await invoke("quit_app");
            } else if (config.close_action === "minimize") {
              await invoke("hide_to_tray");
            } else {
              setShowCloseDialog(true);
            }
          } catch {
            setShowCloseDialog(true);
          }
        }))) return;
      } catch (err) {
        if (!cancelled) {
          console.error("setupEventListeners failed:", err);
        }
      }
    };

    void setup();

    return () => {
      cancelled = true;
      unlistenFns.forEach((fn) => fn());
      unlistenFns = [];
    };
  }, [
    llmConfigRef,
    setAsrTime,
    setCurrentMode,
    setError,
    setHistory,
    setLlmTime,
    setOriginalTranscript,
    setShowCloseDialog,
    setStatus,
    setTotalTime,
    setTranscript,
  ]);
}
