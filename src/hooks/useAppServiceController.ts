import type React from "react";
import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AsrCache,
  AsrConfig,
  AssistantConfig,
  DualHotkeyConfig,
  HotkeyKey,
  LlmConfig,
  SmartCommandConfig,
} from "../types";
import {
  DEFAULT_ASSISTANT_CONFIG,
  DEFAULT_DUAL_HOTKEY_CONFIG,
  DEFAULT_LLM_CONFIG,
  DEFAULT_SMART_COMMAND_CONFIG,
} from "../constants";
import { isAsrConfigValid } from "../utils";

const DICTIONARY_STORAGE_KEY = "pushtotalk_dictionary";

export type UseAppServiceControllerParams = {
  asrCache: AsrCache;
  setAsrCache: React.Dispatch<React.SetStateAction<AsrCache>>;
  setAsrConfig: React.Dispatch<React.SetStateAction<AsrConfig>>;

  apiKey: string;
  setApiKey: React.Dispatch<React.SetStateAction<string>>;

  fallbackApiKey: string;
  setFallbackApiKey: React.Dispatch<React.SetStateAction<string>>;

  useRealtime: boolean;
  setUseRealtime: React.Dispatch<React.SetStateAction<boolean>>;

  enablePostProcess: boolean;
  setEnablePostProcess: React.Dispatch<React.SetStateAction<boolean>>;

  llmConfig: LlmConfig;
  setLlmConfig: React.Dispatch<React.SetStateAction<LlmConfig>>;

  smartCommandConfig: SmartCommandConfig;

  assistantConfig: AssistantConfig;
  setAssistantConfig: React.Dispatch<React.SetStateAction<AssistantConfig>>;

  asrConfig: AsrConfig;

  dualHotkeyConfig: DualHotkeyConfig;
  setDualHotkeyConfig: React.Dispatch<React.SetStateAction<DualHotkeyConfig>>;

  dictionary: string[];
  setDictionary: React.Dispatch<React.SetStateAction<string[]>>;

  status: "idle" | "running" | "recording" | "transcribing";
  setStatus: React.Dispatch<
    React.SetStateAction<"idle" | "running" | "recording" | "transcribing">
  >;

  setError: React.Dispatch<React.SetStateAction<string | null>>;

  enableAutostart: boolean;
  setEnableAutostart: React.Dispatch<React.SetStateAction<boolean>>;

  enableMuteOtherApps: boolean;
  setEnableMuteOtherApps: React.Dispatch<React.SetStateAction<boolean>>;

  closeAction: "close" | "minimize" | null;
  setCloseAction: React.Dispatch<React.SetStateAction<"close" | "minimize" | null>>;

  rememberChoice: boolean;
  setRememberChoice: React.Dispatch<React.SetStateAction<boolean>>;
  setShowCloseDialog: React.Dispatch<React.SetStateAction<boolean>>;

  setShowSuccessToast: React.Dispatch<React.SetStateAction<boolean>>;
};

export function useAppServiceController({
  asrCache,
  setAsrCache,
  setAsrConfig,
  apiKey,
  setApiKey,
  fallbackApiKey,
  setFallbackApiKey,
  useRealtime,
  setUseRealtime,
  enablePostProcess,
  setEnablePostProcess,
  llmConfig,
  setLlmConfig,
  smartCommandConfig,
  assistantConfig,
  setAssistantConfig,
  asrConfig,
  dualHotkeyConfig,
  setDualHotkeyConfig,
  dictionary,
  setDictionary,
  status,
  setStatus,
  setError,
  enableAutostart,
  setEnableAutostart,
  enableMuteOtherApps,
  setEnableMuteOtherApps,
  closeAction,
  setCloseAction,
  rememberChoice,
  setRememberChoice,
  setShowCloseDialog,
  setShowSuccessToast,
}: UseAppServiceControllerParams) {
  const flashSuccessToast = useCallback(() => {
    setShowSuccessToast(true);
    window.setTimeout(() => setShowSuccessToast(false), 3000);
  }, [setShowSuccessToast]);

  const startApp = useCallback(
    async (payload: {
      apiKey: string;
      fallbackApiKey: string;
      useRealtime: boolean;
      enablePostProcess: boolean;
      llmConfig: LlmConfig;
      smartCommandConfig: SmartCommandConfig;
      assistantConfig: AssistantConfig;
      asrConfig: AsrConfig | null;
      dualHotkeyConfig: DualHotkeyConfig;
      enableMuteOtherApps: boolean;
      dictionary: string[];
    }) => {
      await invoke<string>("start_app", payload);
    },
    [],
  );

  const stopApp = useCallback(async () => {
    await invoke<string>("stop_app");
  }, []);

  const loadConfig = useCallback(async () => {
    try {
      const config = await invoke<AppConfig>("load_config");

      setApiKey(config.dashscope_api_key);
      setFallbackApiKey(config.siliconflow_api_key || "");

      if (config.asr_config) {
        const backendPrimary = config.asr_config.primary;
        const nextCache: AsrCache = {
          ...asrCache,
          qwen: { ...asrCache.qwen },
          doubao: { ...asrCache.doubao },
          siliconflow: { ...asrCache.siliconflow },
        };

        if (backendPrimary.provider === "qwen") {
          nextCache.qwen.api_key = backendPrimary.api_key;
        } else if (config.dashscope_api_key) {
          nextCache.qwen.api_key = config.dashscope_api_key;
        }

        if (backendPrimary.provider === "doubao") {
          nextCache.doubao.app_id = backendPrimary.app_id || "";
          nextCache.doubao.access_token = backendPrimary.access_token || "";
        }

        if (config.asr_config.fallback?.provider === "siliconflow") {
          nextCache.siliconflow.api_key = config.asr_config.fallback.api_key;
        } else if (config.siliconflow_api_key) {
          nextCache.siliconflow.api_key = config.siliconflow_api_key;
        }

        setAsrCache(nextCache);

        const preferredProvider = nextCache.active_provider;
        const primary =
          preferredProvider === "qwen"
            ? {
                provider: "qwen" as const,
                api_key: config.dashscope_api_key || nextCache.qwen.api_key,
              }
            : preferredProvider === "doubao"
              ? {
                  provider: "doubao" as const,
                  api_key: "",
                  app_id: nextCache.doubao.app_id,
                  access_token: nextCache.doubao.access_token,
                }
              : {
                  provider: "siliconflow" as const,
                  api_key: nextCache.siliconflow.api_key,
                };

        setAsrConfig({
          ...config.asr_config,
          primary,
        });
      }

      setUseRealtime(config.use_realtime_asr ?? true);
      setEnablePostProcess(config.enable_llm_post_process ?? false);

      const loadedLlmConfig = config.llm_config || DEFAULT_LLM_CONFIG;
      if (loadedLlmConfig.presets && loadedLlmConfig.presets.length > 0) {
        const activeExists = loadedLlmConfig.presets.find(
          (p) => p.id === loadedLlmConfig.active_preset_id,
        );
        if (!activeExists) {
          loadedLlmConfig.active_preset_id = loadedLlmConfig.presets[0].id;
        }
      }
      setLlmConfig(loadedLlmConfig);

      if (config.assistant_config) {
        setAssistantConfig(config.assistant_config);
      } else {
        setAssistantConfig(DEFAULT_ASSISTANT_CONFIG);
      }

      if (config.dual_hotkey_config) {
        setDualHotkeyConfig(config.dual_hotkey_config);
      } else if (config.hotkey_config && config.hotkey_config.keys.length > 0) {
        setDualHotkeyConfig({
          dictation: config.hotkey_config,
          assistant: { keys: ["alt_left", "space"] },
        });
      } else {
        setDualHotkeyConfig(DEFAULT_DUAL_HOTKEY_CONFIG);
      }

      if (config.close_action) {
        setCloseAction(config.close_action);
      }

      try {
        const autostart = await invoke<boolean>("get_autostart");
        setEnableAutostart(autostart);
      } catch (err) {
        console.error("获取开机自启状态失败:", err);
      }

      setEnableMuteOtherApps(config.enable_mute_other_apps ?? false);

      const configDictionary =
        config.dictionary && Array.isArray(config.dictionary) ? config.dictionary : [];
      let localDictionary: string[] = [];
      try {
        const saved = localStorage.getItem(DICTIONARY_STORAGE_KEY);
        if (saved) {
          const parsed = JSON.parse(saved);
          if (Array.isArray(parsed)) {
            localDictionary = parsed.filter((w) => typeof w === "string");
          }
        }
      } catch {
        // ignore
      }
      const loadedDictionary = Array.from(new Set([...configDictionary, ...localDictionary])).filter(
        (w) => typeof w === "string" && w.trim(),
      );
      setDictionary(loadedDictionary);

      const loadedAsrConfig = config.asr_config || null;
      const loadedDualHotkeyConfig = config.dual_hotkey_config || {
        dictation:
          config.hotkey_config ||
          ({ keys: ["control_left", "meta_left"] as HotkeyKey[] } as const),
        assistant: { keys: ["alt_left", "space"] as HotkeyKey[] },
      };
      const loadedSmartCommandConfig =
        config.smart_command_config || DEFAULT_SMART_COMMAND_CONFIG;
      const loadedAssistantConfig = config.assistant_config || DEFAULT_ASSISTANT_CONFIG;

      if (loadedAsrConfig && isAsrConfigValid(loadedAsrConfig.primary)) {
        await new Promise((resolve) => window.setTimeout(resolve, 100));
        await startApp({
          apiKey: config.dashscope_api_key,
          fallbackApiKey: config.siliconflow_api_key || "",
          useRealtime: config.use_realtime_asr ?? true,
          enablePostProcess: config.enable_llm_post_process ?? false,
          llmConfig: loadedLlmConfig,
          smartCommandConfig: loadedSmartCommandConfig,
          assistantConfig: loadedAssistantConfig,
          asrConfig: loadedAsrConfig,
          dualHotkeyConfig: loadedDualHotkeyConfig,
          enableMuteOtherApps: config.enable_mute_other_apps ?? false,
          dictionary: loadedDictionary,
        });
        setStatus("running");
        setError(null);
      }
    } catch (err) {
      console.error("加载配置失败:", err);
    }
  }, [
    asrCache,
    setApiKey,
    setAsrCache,
    setAsrConfig,
    setAssistantConfig,
    setCloseAction,
    setDictionary,
    setDualHotkeyConfig,
    setEnableAutostart,
    setEnableMuteOtherApps,
    setEnablePostProcess,
    setFallbackApiKey,
    setLlmConfig,
    setStatus,
    setError,
    setUseRealtime,
    startApp,
  ]);

  const handleSaveConfig = useCallback(async () => {
    try {
      const validDictionary = dictionary.filter((w) => w.trim());

      await invoke<string>("save_config", {
        apiKey,
        fallbackApiKey,
        useRealtime,
        enablePostProcess,
        llmConfig,
        smartCommandConfig,
        assistantConfig,
        asrConfig,
        dualHotkeyConfig,
        enableMuteOtherApps,
        dictionary: validDictionary,
      });

      setDictionary(validDictionary);

      if (status === "running") {
        await stopApp();
        await startApp({
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary: validDictionary,
        });
      }

      setError(null);
      flashSuccessToast();
    } catch (err) {
      setError(String(err));
    }
  }, [
    apiKey,
    fallbackApiKey,
    useRealtime,
    enablePostProcess,
    llmConfig,
    smartCommandConfig,
    assistantConfig,
    asrConfig,
    dualHotkeyConfig,
    enableMuteOtherApps,
    dictionary,
    status,
    flashSuccessToast,
    setDictionary,
    setError,
    startApp,
    stopApp,
  ]);

  const handleAutostartToggle = useCallback(async () => {
    try {
      const newValue = !enableAutostart;
      await invoke<string>("set_autostart", { enabled: newValue });
      setEnableAutostart(newValue);
      flashSuccessToast();
    } catch (err) {
      setError(String(err));
    }
  }, [enableAutostart, flashSuccessToast, setEnableAutostart, setError]);

  const handleStartStop = useCallback(async () => {
    try {
      if (status === "idle") {
        if (!isAsrConfigValid(asrConfig.primary)) {
          setError("请先配置 ASR API Key");
          return;
        }

        await invoke<string>("save_config", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          closeAction,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary,
        });

        await startApp({
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary,
        });

        setStatus("running");
        setError(null);
        return;
      }

      await stopApp();
      setStatus("idle");
    } catch (err) {
      setError(String(err));
    }
  }, [
    apiKey,
    assistantConfig,
    asrConfig,
    closeAction,
    dictionary,
    dualHotkeyConfig,
    enableMuteOtherApps,
    enablePostProcess,
    fallbackApiKey,
    llmConfig,
    smartCommandConfig,
    setError,
    setStatus,
    startApp,
    status,
    stopApp,
    useRealtime,
  ]);

  const handleCancelTranscription = useCallback(async () => {
    try {
      await invoke<string>("cancel_transcription");
    } catch (err) {
      setError(String(err));
    }
  }, [setError]);

  const handleCloseAction = useCallback(
    async (action: "close" | "minimize") => {
      if (rememberChoice) {
        setCloseAction(action);
        try {
          await invoke("save_config", {
            apiKey,
            fallbackApiKey,
            useRealtime,
            enablePostProcess,
            llmConfig,
            smartCommandConfig,
            assistantConfig,
            asrConfig,
            closeAction: action,
            dualHotkeyConfig,
          });
        } catch (err) {
          console.error("保存关闭配置失败:", err);
        }
      }

      setShowCloseDialog(false);
      setRememberChoice(false);

      if (action === "close") {
        await invoke("quit_app");
      } else {
        await invoke("hide_to_tray");
      }
    },
    [
      apiKey,
      assistantConfig,
      asrConfig,
      dualHotkeyConfig,
      enablePostProcess,
      fallbackApiKey,
      llmConfig,
      rememberChoice,
      setCloseAction,
      setRememberChoice,
      setShowCloseDialog,
      smartCommandConfig,
      useRealtime,
    ],
  );

  return {
    loadConfig,
    handleSaveConfig,
    handleAutostartToggle,
    handleStartStop,
    handleCancelTranscription,
    handleCloseAction,
  };
}
