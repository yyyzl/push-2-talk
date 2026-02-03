import type React from "react";
import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppStatus,
  AsrConfig,
  AssistantConfig,
  DictionaryEntry,
  DualHotkeyConfig,
  HotkeyKey,
  LlmConfig,
} from "../types";
import {
  DEFAULT_ASSISTANT_CONFIG,
  DEFAULT_DUAL_HOTKEY_CONFIG,
  DEFAULT_LLM_CONFIG,
} from "../constants";
import { isAsrConfigValid } from "../utils";
import { entriesToWords, parseEntry, entriesToStorageFormat } from "../utils/dictionaryUtils";
import { getBuiltinWordsForDomains, normalizeBuiltinDictionaryDomains } from "../utils/builtinDictionary";

const DICTIONARY_STORAGE_KEY = "pushtotalk_dictionary";

const buildRuntimeDictionary = (
  dictionaryEntries: DictionaryEntry[],
  builtinDomains: string[],
): string[] => {
  const userWords = entriesToWords(dictionaryEntries);
  const builtinWords = getBuiltinWordsForDomains(builtinDomains);
  if (builtinWords.length === 0) return userWords;

  const merged = new Set<string>();
  const result: string[] = [];

  for (const word of userWords) {
    if (merged.has(word)) continue;
    merged.add(word);
    result.push(word);
  }

  for (const word of builtinWords) {
    if (merged.has(word)) continue;
    merged.add(word);
    result.push(word);
  }

  return result;
};

export type UseAppServiceControllerParams = {
  setAsrConfig: React.Dispatch<React.SetStateAction<AsrConfig>>;

  apiKey: string;
  setApiKey: React.Dispatch<React.SetStateAction<string>>;

  fallbackApiKey: string;
  setFallbackApiKey: React.Dispatch<React.SetStateAction<string>>;

  useRealtime: boolean;
  setUseRealtime: React.Dispatch<React.SetStateAction<boolean>>;

  enablePostProcess: boolean;
  setEnablePostProcess: React.Dispatch<React.SetStateAction<boolean>>;

  enableDictionaryEnhancement: boolean;
  setEnableDictionaryEnhancement: React.Dispatch<React.SetStateAction<boolean>>;

  llmConfig: LlmConfig;
  setLlmConfig: React.Dispatch<React.SetStateAction<LlmConfig>>;

  assistantConfig: AssistantConfig;
  setAssistantConfig: React.Dispatch<React.SetStateAction<AssistantConfig>>;

  asrConfig: AsrConfig;

  dualHotkeyConfig: DualHotkeyConfig;
  setDualHotkeyConfig: React.Dispatch<React.SetStateAction<DualHotkeyConfig>>;

  dictionary: DictionaryEntry[];
  setDictionary: React.Dispatch<React.SetStateAction<DictionaryEntry[]>>;

  builtinDictionaryDomains: string[];
  setBuiltinDictionaryDomains: React.Dispatch<React.SetStateAction<string[]>>;

  status: AppStatus;
  setStatus: React.Dispatch<React.SetStateAction<AppStatus>>;

  setError: React.Dispatch<React.SetStateAction<string | null>>;

  enableAutostart: boolean;
  setEnableAutostart: React.Dispatch<React.SetStateAction<boolean>>;

  enableMuteOtherApps: boolean;
  setEnableMuteOtherApps: React.Dispatch<React.SetStateAction<boolean>>;

  theme: string;
  setTheme: React.Dispatch<React.SetStateAction<string>>;

  closeAction: "close" | "minimize" | null;
  setCloseAction: React.Dispatch<React.SetStateAction<"close" | "minimize" | null>>;

  rememberChoice: boolean;
  setRememberChoice: React.Dispatch<React.SetStateAction<boolean>>;
  setShowCloseDialog: React.Dispatch<React.SetStateAction<boolean>>;

  setShowSuccessToast: React.Dispatch<React.SetStateAction<boolean>>;

  /** 即时保存前的回调，用于取消 debounce timer */
  onBeforeImmediateSave?: () => void;
};

export function useAppServiceController({
  setAsrConfig,
  apiKey,
  setApiKey,
  fallbackApiKey,
  setFallbackApiKey,
  useRealtime,
  setUseRealtime,
  enablePostProcess,
  setEnablePostProcess,
  enableDictionaryEnhancement,
  setEnableDictionaryEnhancement,
  llmConfig,
  setLlmConfig,
  assistantConfig,
  setAssistantConfig,
  asrConfig,
  dualHotkeyConfig,
  setDualHotkeyConfig,
  dictionary,
  setDictionary,
  builtinDictionaryDomains,
  setBuiltinDictionaryDomains,
  status,
  setStatus,
  setError,
  enableAutostart,
  setEnableAutostart,
  enableMuteOtherApps,
  setEnableMuteOtherApps,
  theme,
  setTheme,
  closeAction,
  setCloseAction,
  rememberChoice,
  setRememberChoice,
  setShowCloseDialog,
  setShowSuccessToast,
  onBeforeImmediateSave,
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
      enableDictionaryEnhancement: boolean;
      llmConfig: LlmConfig;
      smartCommandConfig: null;
      assistantConfig: AssistantConfig;
      asrConfig: AsrConfig | null;
      dualHotkeyConfig: DualHotkeyConfig;
      enableMuteOtherApps: boolean;
      dictionary: string[];
      theme: string;
    }) => {
      await invoke<string>("start_app", payload);
    },
    [],
  );

  const stopApp = useCallback(async () => {
    await invoke<string>("stop_app");
  }, []);

  // 热更新运行时配置（无需重启服务）
  const applyRuntimeConfig = useCallback(
    async (updates: {
      enablePostProcess?: boolean;
      enableDictionaryEnhancement?: boolean;
      llmConfig?: LlmConfig;
      assistantConfig?: AssistantConfig;
      enableMuteOtherApps?: boolean;
      dictionary?: DictionaryEntry[];
    }): Promise<boolean> => {
      if (status !== "running") return false;
      try {
        await invoke<string>("update_runtime_config", {
          enablePostProcess: updates.enablePostProcess,
          enableDictionaryEnhancement: updates.enableDictionaryEnhancement,
          llmConfig: updates.llmConfig,
          assistantConfig: updates.assistantConfig,
          enableMuteOtherApps: updates.enableMuteOtherApps,
          dictionary: updates.dictionary
            ? buildRuntimeDictionary(updates.dictionary, builtinDictionaryDomains)
            : undefined,
        });
        return true;
      } catch (err) {
        console.error("热更新配置失败:", err);
        return false;
      }
    },
    [builtinDictionaryDomains, status],
  );

  const loadConfig = useCallback(async () => {
    try {
      let config = await invoke<AppConfig>("load_config");

      // ========== 迁移逻辑：从 localStorage 迁移到后端 (幂等) ==========
      const backendCreds = config.asr_config?.credentials;
      const backendHasAnyCredential = Boolean(
        backendCreds?.qwen_api_key?.trim() ||
        backendCreds?.sensevoice_api_key?.trim() ||
        backendCreds?.doubao_app_id?.trim() ||
        backendCreds?.doubao_access_token?.trim()
      );

      if (!backendHasAnyCredential) {
        try {
          const savedCache = localStorage.getItem('pushtotalk_asr_cache');
          if (savedCache) {
            console.log('[迁移] 检测到后端配置为空且发现 localStorage 配置，开始迁移');
            const parsedCache = JSON.parse(savedCache);

            const activeProvider =
              parsedCache.active_provider === 'qwen' ||
                parsedCache.active_provider === 'doubao' ||
                parsedCache.active_provider === 'siliconflow'
                ? parsedCache.active_provider
                : 'qwen';

            const migratedAsrConfig: AsrConfig = {
              credentials: {
                qwen_api_key: parsedCache.qwen?.api_key || '',
                sensevoice_api_key: parsedCache.siliconflow?.api_key || '',
                doubao_app_id: parsedCache.doubao?.app_id || '',
                doubao_access_token: parsedCache.doubao?.access_token || '',
              },
              selection: {
                active_provider: activeProvider,
                enable_fallback: false,
                fallback_provider: null,
              },
            };

            let localDictionary: string[] = [];
            try {
              const savedDict = localStorage.getItem(DICTIONARY_STORAGE_KEY);
              if (savedDict) {
                const parsed = JSON.parse(savedDict);
                if (Array.isArray(parsed)) {
                  localDictionary = parsed.filter((w) => typeof w === "string");
                }
              }
            } catch {
              // ignore
            }

            const mergedDictionary = Array.from(
              new Set([...(config.dictionary || []), ...localDictionary])
            ).filter((w) => typeof w === "string" && w.trim());

            await invoke("save_config", {
              apiKey: config.dashscope_api_key || '',
              fallbackApiKey: config.siliconflow_api_key || '',
              useRealtime: config.use_realtime_asr ?? true,
              enablePostProcess: config.enable_llm_post_process ?? false,
              enableDictionaryEnhancement: config.enable_dictionary_enhancement ?? true,
              llmConfig: config.llm_config || DEFAULT_LLM_CONFIG,
              smartCommandConfig: null,
              assistantConfig: config.assistant_config || DEFAULT_ASSISTANT_CONFIG,
              asrConfig: migratedAsrConfig,
              dualHotkeyConfig: config.dual_hotkey_config || DEFAULT_DUAL_HOTKEY_CONFIG,
              enableMuteOtherApps: config.enable_mute_other_apps ?? false,
              dictionary: mergedDictionary,
              builtinDictionaryDomains: normalizeBuiltinDictionaryDomains(
                config.builtin_dictionary_domains || []
              ),
              theme: config.theme || 'light',
            });

            console.log('[迁移] 配置已保存到后端，清理 localStorage');
            localStorage.removeItem('pushtotalk_asr_cache');
            localStorage.removeItem(DICTIONARY_STORAGE_KEY);
            config = await invoke<AppConfig>("load_config");
          }
        } catch (err) {
          console.error('[迁移] 迁移失败:', err);
        }
      }
      // ========== 迁移逻辑结束 ==========

      setApiKey(config.dashscope_api_key);
      setFallbackApiKey(config.siliconflow_api_key || "");

      if (config.asr_config) {
        setAsrConfig(config.asr_config);
      }

      setUseRealtime(config.use_realtime_asr ?? true);
      setEnablePostProcess(config.enable_llm_post_process ?? false);
      setEnableDictionaryEnhancement(config.enable_dictionary_enhancement ?? true);

      // 智能补齐 llm_config
      const loadedLlmConfig = config.llm_config || DEFAULT_LLM_CONFIG;
      if (!loadedLlmConfig.presets || loadedLlmConfig.presets.length === 0) {
        console.warn('[配置修复] 检测到空 presets，使用默认值');
        loadedLlmConfig.presets = DEFAULT_LLM_CONFIG.presets;
        loadedLlmConfig.active_preset_id = DEFAULT_LLM_CONFIG.active_preset_id;
      } else {
        const activeExists = loadedLlmConfig.presets.find(
          (p) => p.id === loadedLlmConfig.active_preset_id,
        );
        if (!activeExists) {
          loadedLlmConfig.active_preset_id = loadedLlmConfig.presets[0].id;
        }
      }
      setLlmConfig(loadedLlmConfig);

      // 智能补齐 assistant_config
      let loadedAssistantConfig = config.assistant_config || DEFAULT_ASSISTANT_CONFIG;
      if (!loadedAssistantConfig.qa_system_prompt || !loadedAssistantConfig.text_processing_system_prompt) {
        console.warn('[配置修复] 检测到不完整的 assistant_config，使用默认值');
        loadedAssistantConfig = DEFAULT_ASSISTANT_CONFIG;
      }
      setAssistantConfig(loadedAssistantConfig);

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
      setTheme(config.theme || "light");

      const configDictionary =
        config.dictionary && Array.isArray(config.dictionary) ? config.dictionary : [];

      // 处理词典：支持新格式 DictionaryEntry[] 和旧格式 string[]
      let loadedDictionary: DictionaryEntry[];
      if (configDictionary.length > 0 && typeof configDictionary[0] === "object") {
        // 新格式：DictionaryEntry[]
        loadedDictionary = configDictionary as unknown as DictionaryEntry[];
      } else {
        // 旧格式：string[]，需要转换（支持 "word" 和 "word|auto" 格式）
        const words = (configDictionary as unknown as string[]).filter(
          (w) => typeof w === "string" && w.trim()
        );
        loadedDictionary = words.map(parseEntry);
      }
      setDictionary(loadedDictionary);

      const loadedBuiltinDictionaryDomains = normalizeBuiltinDictionaryDomains(
        config.builtin_dictionary_domains || []
      );
      setBuiltinDictionaryDomains(loadedBuiltinDictionaryDomains);

      const loadedAsrConfig = config.asr_config || null;
      const loadedDualHotkeyConfig = config.dual_hotkey_config || {
        dictation:
          config.hotkey_config ||
          ({ keys: ["control_left", "meta_left"] as HotkeyKey[] } as const),
        assistant: { keys: ["alt_left", "space"] as HotkeyKey[] },
      };

      if (loadedAsrConfig && isAsrConfigValid(loadedAsrConfig)) {
        await new Promise((resolve) => window.setTimeout(resolve, 100));
        await startApp({
          apiKey: config.dashscope_api_key,
          fallbackApiKey: config.siliconflow_api_key || "",
          useRealtime: config.use_realtime_asr ?? true,
          enablePostProcess: config.enable_llm_post_process ?? false,
          enableDictionaryEnhancement: config.enable_dictionary_enhancement ?? true,
          llmConfig: loadedLlmConfig,
          smartCommandConfig: null,
          assistantConfig: loadedAssistantConfig,
          asrConfig: loadedAsrConfig,
          dualHotkeyConfig: loadedDualHotkeyConfig,
          enableMuteOtherApps: config.enable_mute_other_apps ?? false,
          dictionary: buildRuntimeDictionary(
            loadedDictionary,
            loadedBuiltinDictionaryDomains
          ),
          theme: config.theme || "light",
        });
        setStatus("running");
        setError(null);
      }
    } catch (err) {
      console.error("加载配置失败:", err);
    }
  }, [
    setApiKey,
    setAsrConfig,
    setAssistantConfig,
    setCloseAction,
    setDictionary,
    setBuiltinDictionaryDomains,
    setDualHotkeyConfig,
    setEnableAutostart,
    setEnableMuteOtherApps,
    setEnablePostProcess,
    setEnableDictionaryEnhancement,
    setFallbackApiKey,
    setLlmConfig,
    setStatus,
    setError,
    setUseRealtime,
    startApp,
  ]);

  const handleSaveConfig = useCallback(async () => {
    try {
      // 保存时使用存储格式（保留 source 信息）
      const storageDictionary = entriesToStorageFormat(dictionary);
      // 运行时使用纯词汇（用于 ASR API）
      const runtimeDictionary = buildRuntimeDictionary(dictionary, builtinDictionaryDomains);

      console.log("[handleSaveConfig] 保存配置, theme=", theme);

      await invoke<string>("save_config", {
        apiKey,
        fallbackApiKey,
        useRealtime,
        enablePostProcess,
        enableDictionaryEnhancement,
        llmConfig,
        smartCommandConfig: null,
        assistantConfig,
        asrConfig,
        dualHotkeyConfig,
        enableMuteOtherApps,
        dictionary: storageDictionary,
        builtinDictionaryDomains,
        theme,
      });

      console.log("[handleSaveConfig] 配置已保存到后端");

      // 不需要更新 dictionary 状态，因为它已经是正确的格式

      if (status === "running") {
        await stopApp();
        await startApp({
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          enableDictionaryEnhancement,
          llmConfig,
          smartCommandConfig: null,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary: runtimeDictionary,
          theme,
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
    enableDictionaryEnhancement,
    llmConfig,
    assistantConfig,
    asrConfig,
    dualHotkeyConfig,
    enableMuteOtherApps,
    dictionary,
    builtinDictionaryDomains,
    status,
    flashSuccessToast,
    setError,
    startApp,
    stopApp,
  ]);

  /**
   * 即时保存配置并重启服务（绕过 debounce）
   * 用于 ASR 切换、实时/HTTP 模式切换等需要立即生效的场景
   *
   * @param overrides - 可选的配置覆盖，用于传入最新的状态值（解决 React setState 异步问题）
   */
  const immediatelySaveConfig = useCallback(async (overrides?: {
    useRealtime?: boolean;
    enablePostProcess?: boolean;
    enableDictionaryEnhancement?: boolean;
    llmConfig?: LlmConfig;
    assistantConfig?: AssistantConfig;
    asrConfig?: AsrConfig;
    dualHotkeyConfig?: DualHotkeyConfig;
    enableMuteOtherApps?: boolean;
    dictionary?: DictionaryEntry[];
    builtinDictionaryDomains?: string[];
    theme?: string;
  }) => {
    // 先取消 debounce timer
    onBeforeImmediateSave?.();

    try {
      // 合并当前状态和传入的 overrides
      const finalUseRealtime = overrides?.useRealtime ?? useRealtime;
      const finalEnablePostProcess = overrides?.enablePostProcess ?? enablePostProcess;
      const finalEnableDictionaryEnhancement =
        overrides?.enableDictionaryEnhancement ?? enableDictionaryEnhancement;
      const finalLlmConfig = overrides?.llmConfig ?? llmConfig;
      const finalAssistantConfig = overrides?.assistantConfig ?? assistantConfig;
      const finalAsrConfig = overrides?.asrConfig ?? asrConfig;
      const finalDualHotkeyConfig = overrides?.dualHotkeyConfig ?? dualHotkeyConfig;
      const finalEnableMuteOtherApps = overrides?.enableMuteOtherApps ?? enableMuteOtherApps;
      const finalDictionary = overrides?.dictionary ?? dictionary;
      const finalBuiltinDictionaryDomains =
        overrides?.builtinDictionaryDomains ?? builtinDictionaryDomains;
      const finalTheme = overrides?.theme ?? theme;
      // 保存时使用存储格式（保留 source 信息）
      const storageDictionary = entriesToStorageFormat(finalDictionary);
      // 运行时使用纯词汇（用于 ASR API）
      const runtimeDictionary = buildRuntimeDictionary(
        finalDictionary,
        finalBuiltinDictionaryDomains
      );

      await invoke<string>("save_config", {
        apiKey,
        fallbackApiKey,
        useRealtime: finalUseRealtime,
        enablePostProcess: finalEnablePostProcess,
        enableDictionaryEnhancement: finalEnableDictionaryEnhancement,
        llmConfig: finalLlmConfig,
        smartCommandConfig: null,
        assistantConfig: finalAssistantConfig,
        asrConfig: finalAsrConfig,
        dualHotkeyConfig: finalDualHotkeyConfig,
        enableMuteOtherApps: finalEnableMuteOtherApps,
        dictionary: storageDictionary,
        builtinDictionaryDomains: finalBuiltinDictionaryDomains,
        theme: finalTheme,
      });

      if (overrides?.dictionary) setDictionary(overrides.dictionary);
      if (overrides?.builtinDictionaryDomains) {
        setBuiltinDictionaryDomains(overrides.builtinDictionaryDomains);
      }
      if (overrides?.theme) setTheme(overrides.theme);

      if (status === "running") {
        await stopApp();
        await startApp({
          apiKey,
          fallbackApiKey,
          useRealtime: finalUseRealtime,
          enablePostProcess: finalEnablePostProcess,
          enableDictionaryEnhancement: finalEnableDictionaryEnhancement,
          llmConfig: finalLlmConfig,
          smartCommandConfig: null,
          assistantConfig: finalAssistantConfig,
          asrConfig: finalAsrConfig,
          dualHotkeyConfig: finalDualHotkeyConfig,
          enableMuteOtherApps: finalEnableMuteOtherApps,
          dictionary: runtimeDictionary,
          theme: finalTheme,
        });
      }

      setError(null);
      // 即时保存不显示 toast，由组件自己的状态指示器显示反馈
    } catch (err) {
      setError(String(err));
      throw err; // 重新抛出，让调用方可以处理回滚
    }
  }, [
    onBeforeImmediateSave,
    apiKey,
    fallbackApiKey,
    useRealtime,
    enablePostProcess,
    enableDictionaryEnhancement,
    llmConfig,
    assistantConfig,
    asrConfig,
    dualHotkeyConfig,
    enableMuteOtherApps,
    dictionary,
    builtinDictionaryDomains,
    status,
    setDictionary,
    setBuiltinDictionaryDomains,
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
        if (!isAsrConfigValid(asrConfig)) {
          setError("请先配置 ASR API Key");
          return;
        }

        // 保存时使用存储格式（保留 source 信息）
        const storageDictionary = entriesToStorageFormat(dictionary);
        // 运行时使用纯词汇（用于 ASR API）
        const runtimeDictionary = buildRuntimeDictionary(dictionary, builtinDictionaryDomains);

        await invoke<string>("save_config", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          enableDictionaryEnhancement,
          llmConfig,
          smartCommandConfig: null,
          assistantConfig,
          asrConfig,
          closeAction,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary: storageDictionary,
          builtinDictionaryDomains,
          theme,
        });

        await startApp({
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          enableDictionaryEnhancement,
          llmConfig,
          smartCommandConfig: null,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary: runtimeDictionary,
          theme,
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
    builtinDictionaryDomains,
    dualHotkeyConfig,
    enableMuteOtherApps,
    enablePostProcess,
    enableDictionaryEnhancement,
    fallbackApiKey,
    llmConfig,
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
            enableDictionaryEnhancement,
            llmConfig,
            smartCommandConfig: null,
            assistantConfig,
            asrConfig,
            closeAction: action,
            dualHotkeyConfig,
            enableMuteOtherApps,
            dictionary: entriesToStorageFormat(dictionary),
            builtinDictionaryDomains,
            theme,
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
      dictionary,
      builtinDictionaryDomains,
      dualHotkeyConfig,
      enableMuteOtherApps,
      enablePostProcess,
      fallbackApiKey,
      llmConfig,
      rememberChoice,
      setCloseAction,
      setRememberChoice,
      setShowCloseDialog,
      useRealtime,
    ],
  );

  return {
    loadConfig,
    handleSaveConfig,
    immediatelySaveConfig,
    handleAutostartToggle,
    handleStartStop,
    handleCancelTranscription,
    handleCloseAction,
    applyRuntimeConfig,
  };
}
