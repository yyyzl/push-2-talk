import type React from "react";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { nanoid } from "nanoid";
import type {
  AppConfig,
  AppStatus,
  AsrConfig,
  AssistantConfig,
  DictionaryEntry,
  HistoryRecord,
  LlmConfig,
  TranscriptionResult,
  UsageStats,
} from "../types";
import { MAX_HISTORY } from "../constants";
import { saveHistory, loadUsageStats } from "../utils";
import { parseEntry } from "../utils/dictionaryUtils";
import { normalizeBuiltinDictionaryDomains } from "../utils/builtinDictionary";

type UnlistenFn = () => void;

export type UseTauriEventListenersParams = {
  llmConfigRef: React.RefObject<LlmConfig>;
  enablePostProcessRef?: React.RefObject<boolean>;
  enableDictionaryEnhancementRef?: React.RefObject<boolean>;
  setActivePresetName?: React.Dispatch<React.SetStateAction<string | null>>;

  setStatus: React.Dispatch<React.SetStateAction<AppStatus>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  setTranscript: React.Dispatch<React.SetStateAction<string>>;
  setOriginalTranscript: React.Dispatch<React.SetStateAction<string | null>>;
  setCurrentMode: React.Dispatch<React.SetStateAction<string | null>>;
  setAsrTime: React.Dispatch<React.SetStateAction<number | null>>;
  setLlmTime: React.Dispatch<React.SetStateAction<number | null>>;
  setTotalTime: React.Dispatch<React.SetStateAction<number | null>>;
  setShowCloseDialog: React.Dispatch<React.SetStateAction<boolean>>;

  setApiKey?: React.Dispatch<React.SetStateAction<string>>;
  setFallbackApiKey?: React.Dispatch<React.SetStateAction<string>>;
  setAsrConfig?: React.Dispatch<React.SetStateAction<AsrConfig>>;
  setUseRealtime?: React.Dispatch<React.SetStateAction<boolean>>;
  setEnablePostProcess?: React.Dispatch<React.SetStateAction<boolean>>;
  setEnableDictionaryEnhancement?: React.Dispatch<React.SetStateAction<boolean>>;
  setLlmConfig?: React.Dispatch<React.SetStateAction<LlmConfig>>;
  setAssistantConfig?: React.Dispatch<React.SetStateAction<AssistantConfig>>;
  setEnableMuteOtherApps?: React.Dispatch<React.SetStateAction<boolean>>;
  setTheme?: React.Dispatch<React.SetStateAction<string>>;
  setCloseAction?: React.Dispatch<React.SetStateAction<"close" | "minimize" | null>>;
  setDictionary?: React.Dispatch<React.SetStateAction<DictionaryEntry[]>>;
  setBuiltinDictionaryDomains?: React.Dispatch<React.SetStateAction<string[]>>;
  onExternalConfigUpdated?: (config: AppConfig) => void;

  setHistory: React.Dispatch<React.SetStateAction<HistoryRecord[]>>;
  setUsageStats?: React.Dispatch<React.SetStateAction<UsageStats>>;

  /** 润色失败时的回调（用于显示 Toast 提示） */
  onPolishingFailed?: (errorMessage: string) => void;
};

export function useTauriEventListeners({
  llmConfigRef,
  enablePostProcessRef,
  enableDictionaryEnhancementRef,
  setActivePresetName,
  setStatus,
  setError,
  setTranscript,
  setOriginalTranscript,
  setCurrentMode,
  setAsrTime,
  setLlmTime,
  setTotalTime,
  setShowCloseDialog,
  setApiKey,
  setFallbackApiKey,
  setAsrConfig,
  setUseRealtime,
  setEnablePostProcess,
  setEnableDictionaryEnhancement,
  setLlmConfig,
  setAssistantConfig,
  setEnableMuteOtherApps,
  setTheme,
  setCloseAction,
  setDictionary,
  setBuiltinDictionaryDomains,
  onExternalConfigUpdated,
  setHistory,
  setUsageStats,
  onPolishingFailed,
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
          // 只要有 original_text 就显示双栏（原始转写 + 润色结果）
          setOriginalTranscript(result.original_text || null);
          setCurrentMode(result.mode || null);
          setAsrTime(result.asr_time_ms);
          setLlmTime(result.llm_time_ms);
          setTotalTime(result.total_time_ms);
          setStatus("running");

          // 后端已自动更新统计数据，前端只需重新加载
          reloadUsageStats();

          const llmConfig = llmConfigRef.current;
          const mode = (result.mode as "normal" | "assistant") || null;
          const enablePostProcess = enablePostProcessRef?.current ?? false;
          const enableDictionaryEnhancement = enableDictionaryEnhancementRef?.current ?? false;

          // presetName 逻辑：
          // 1. 如果没有 original_text（未启用润色），不显示任何润色标签
          // 2. 如果是 assistant 模式，不显示润色标签
          // 3. 如果开启了润色，显示预设名称
          // 4. 如果开启了词库增强，显示"词库增强"
          // 5. 其他情况（仅 TNL 处理），显示"文本规范化"
          const hasPolishing = !!result.original_text;
          const presetName = hasPolishing && mode !== "assistant"
            ? enablePostProcess
              ? llmConfig?.presets.find((p) => p.id === llmConfig.active_preset_id)?.name || null
              : (enableDictionaryEnhancement ? "词库增强" : "文本规范化")
            : null;

          setActivePresetName?.(presetName);

          addHistoryRecord({
            id: nanoid(8),
            timestamp: Date.now(),
            originalText: result.original_text || result.text,
            // 只要有 original_text 就设置 polishedText
            polishedText: hasPolishing ? result.text : null,
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

        if (!(await registerListener<AppConfig>("config_updated", (config) => {
          onExternalConfigUpdated?.(config);

          setApiKey?.(config.dashscope_api_key || "");
          setFallbackApiKey?.(config.siliconflow_api_key || "");
          if (config.asr_config) setAsrConfig?.(config.asr_config);
          setUseRealtime?.(config.use_realtime_asr ?? true);
          setEnablePostProcess?.(config.enable_llm_post_process ?? false);
          setEnableDictionaryEnhancement?.(config.enable_dictionary_enhancement ?? true);
          setLlmConfig?.(config.llm_config || llmConfigRef.current);
          if (config.assistant_config) setAssistantConfig?.(config.assistant_config);
          setEnableMuteOtherApps?.(config.enable_mute_other_apps ?? false);
          setTheme?.(config.theme || "light");

          const nextCloseAction = config.close_action === "close" || config.close_action === "minimize"
            ? config.close_action
            : null;
          setCloseAction?.(nextCloseAction);

          if (setDictionary) {
            const configDictionary = Array.isArray(config.dictionary) ? config.dictionary : [];
            const normalizedDictionary = configDictionary
              .filter((entry) => typeof entry === "string" && entry.trim())
              .map((entry) => parseEntry(entry));
            setDictionary(normalizedDictionary);
          }

          if (setBuiltinDictionaryDomains) {
            setBuiltinDictionaryDomains(
              normalizeBuiltinDictionaryDomains(config.builtin_dictionary_domains || [])
            );
          }
        }))) return;

        // 监听润色失败事件（让用户知道润色尝试过但失败了）
        if (!(await registerListener<string>("polishing_failed", (errorMessage) => {
          console.warn("润色失败:", errorMessage);
          onPolishingFailed?.(errorMessage);
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
    setActivePresetName,
    setCurrentMode,
    setError,
    setHistory,
    setLlmTime,
    setOriginalTranscript,
    setApiKey,
    setFallbackApiKey,
    setAsrConfig,
    setUseRealtime,
    setEnablePostProcess,
    setEnableDictionaryEnhancement,
    setLlmConfig,
    setAssistantConfig,
    setEnableMuteOtherApps,
    setTheme,
    setCloseAction,
    setDictionary,
    setBuiltinDictionaryDomains,
    onExternalConfigUpdated,
    setShowCloseDialog,
    setStatus,
    setTotalTime,
    setTranscript,
  ]);
}
