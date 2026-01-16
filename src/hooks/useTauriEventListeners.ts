import type React from "react";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { nanoid } from "nanoid";
import type { AppConfig, HistoryRecord, LlmConfig, TranscriptionResult, UsageStats } from "../types";
import { MAX_HISTORY } from "../constants";
import { saveHistory, saveUsageStats } from "../utils";

type UnlistenFn = () => void;

export type UseTauriEventListenersParams = {
  llmConfigRef: React.RefObject<LlmConfig>;

  setStatus: React.Dispatch<
    React.SetStateAction<"idle" | "running" | "recording" | "transcribing">
  >;
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

    let recordingStartAtMs: number | null = null;
    let pendingRecordingDurationMs: number | null = null;

    const addHistoryRecord = (record: HistoryRecord) => {
      setHistory((prev) => {
        const updated = [record, ...prev].slice(0, MAX_HISTORY);
        saveHistory(updated);
        return updated;
      });
    };

    const countRecognizedChars = (text: string) => text.replace(/\s+/g, "").length;

    const commitUsageStats = (patch: (prev: UsageStats) => UsageStats) => {
      if (!setUsageStats) return;
      setUsageStats((prev) => {
        const next = patch(prev);
        saveUsageStats(next);
        return next;
      });
    };

    const setup = async () => {
      try {
        unlistenFns.push(
          await listen("recording_started", () => {
            setStatus("recording");
            setError(null);
            recordingStartAtMs = Date.now();
            pendingRecordingDurationMs = null;
          }),
        );

        unlistenFns.push(
          await listen("recording_stopped", () => {
            setStatus("transcribing");
            if (recordingStartAtMs != null) {
              pendingRecordingDurationMs = Math.max(0, Date.now() - recordingStartAtMs);
            }
          }),
        );

        unlistenFns.push(
          await listen("transcribing", () => {
            setStatus("transcribing");
          }),
        );

        unlistenFns.push(
          await listen<TranscriptionResult>("transcription_complete", (event) => {
            const result = event.payload;

            setTranscript(result.text);
            setOriginalTranscript(result.original_text);
            setCurrentMode(result.mode || null);
            setAsrTime(result.asr_time_ms);
            setLlmTime(result.llm_time_ms);
            setTotalTime(result.total_time_ms);
            setStatus("running");

            const recognizedBaseText = result.original_text || result.text;
            const recognizedChars = countRecognizedChars(recognizedBaseText);

            const durationMs =
              pendingRecordingDurationMs != null
                ? pendingRecordingDurationMs
                : recordingStartAtMs != null
                  ? Math.max(0, Date.now() - recordingStartAtMs)
                  : 0;

            if (durationMs > 0 || recognizedChars > 0) {
              commitUsageStats((prev) => ({
                totalRecordingMs: prev.totalRecordingMs + durationMs,
                totalRecordingCount: prev.totalRecordingCount + 1,
                totalRecognizedChars: prev.totalRecognizedChars + recognizedChars,
              }));
            }

            recordingStartAtMs = null;
            pendingRecordingDurationMs = null;

            const llmConfig = llmConfigRef.current;
            const presetName = result.original_text
              ? llmConfig?.presets.find((p) => p.id === llmConfig.active_preset_id)?.name || null
              : null;

            addHistoryRecord({
              id: nanoid(8),
              timestamp: Date.now(),
              originalText: result.original_text || result.text,
              polishedText: result.original_text ? result.text : null,
              presetName,
              asrTimeMs: result.asr_time_ms,
              llmTimeMs: result.llm_time_ms,
              totalTimeMs: result.total_time_ms,
              success: true,
              errorMessage: null,
            });
          }),
        );

        unlistenFns.push(
          await listen<string>("error", (event) => {
            const errMsg = event.payload;
            setError(errMsg);
            setStatus("running");

            const durationMs =
              pendingRecordingDurationMs != null
                ? pendingRecordingDurationMs
                : recordingStartAtMs != null
                  ? Math.max(0, Date.now() - recordingStartAtMs)
                  : null;

            if (durationMs != null) {
              commitUsageStats((prev) => ({
                totalRecordingMs: prev.totalRecordingMs + durationMs,
                totalRecordingCount: prev.totalRecordingCount + 1,
                totalRecognizedChars: prev.totalRecognizedChars,
              }));
            }

            recordingStartAtMs = null;
            pendingRecordingDurationMs = null;

            addHistoryRecord({
              id: nanoid(8),
              timestamp: Date.now(),
              originalText: "",
              polishedText: null,
              presetName: null,
              asrTimeMs: 0,
              llmTimeMs: null,
              totalTimeMs: 0,
              success: false,
              errorMessage: errMsg,
            });
          }),
        );

        unlistenFns.push(
          await listen("transcription_cancelled", () => {
            setStatus("running");
            setError(null);
            recordingStartAtMs = null;
            pendingRecordingDurationMs = null;
          }),
        );

        unlistenFns.push(
          await listen("close_requested", async () => {
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
          }),
        );
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
