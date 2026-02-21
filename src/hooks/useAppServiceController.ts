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
  LearningConfig,
  LlmConfig,
} from "../types";
import {
  DEFAULT_ASSISTANT_CONFIG,
  DEFAULT_DUAL_HOTKEY_CONFIG,
  DEFAULT_LEARNING_CONFIG,
  DEFAULT_LLM_CONFIG,
  FALLBACK_ASR_PROVIDER,
  VALID_ASR_PROVIDERS,
  normalizeLearningConfig,
} from "../constants";
import { isAsrConfigValid, normalizeAsrConfigWithFallback, getAsrProviderDisplayName } from "../utils";
import { features } from "./usePlatform";
import {
  entriesToWords,
  filterDictionaryEntriesByAutoLearning,
  parseEntry,
  entriesToStorageFormat,
} from "../utils/dictionaryUtils";
import {
  fetchBuiltinDomains,
  getBuiltinWordsForDomains,
  normalizeBuiltinDictionaryDomains,
  setBuiltinDomainsSnapshot,
} from "../utils/builtinDictionary";

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

const applyPlatformDictionaryFilter = (entries: DictionaryEntry[]): DictionaryEntry[] =>
  filterDictionaryEntriesByAutoLearning(entries, features.autoLearning);

type SaveConfigGatewayOverrides = {
  apiKey?: string;
  fallbackApiKey?: string;
  useRealtime?: boolean;
  enablePostProcess?: boolean;
  enableDictionaryEnhancement?: boolean;
  llmConfig?: LlmConfig;
  assistantConfig?: AssistantConfig;
  asrConfig?: AsrConfig;
  closeAction?: "close" | "minimize" | null;
  dualHotkeyConfig?: DualHotkeyConfig;
  learningConfig?: LearningConfig;
  enableMuteOtherApps?: boolean;
  dictionaryEntries?: DictionaryEntry[];
  storageDictionary?: string[];
  builtinDictionaryDomains?: string[];
  theme?: string;
};

type ConfigFieldPatchPayload = {
  learningEnabled?: boolean;
  theme?: string;
  enableMuteOtherApps?: boolean;
  closeAction?: "close" | "minimize" | null;
};

type ResolvedSaveConfig = {
  apiKey: string;
  fallbackApiKey: string;
  useRealtime: boolean;
  enablePostProcess: boolean;
  enableDictionaryEnhancement: boolean;
  llmConfig: LlmConfig;
  assistantConfig: AssistantConfig;
  asrConfig: AsrConfig;
  closeAction: "close" | "minimize" | null;
  dualHotkeyConfig: DualHotkeyConfig;
  learningConfig: LearningConfig;
  enableMuteOtherApps: boolean;
  dictionaryEntries: DictionaryEntry[];
  storageDictionary: string[];
  runtimeDictionary: string[];
  builtinDictionaryDomains: string[];
  theme: string;
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

  learningConfig: LearningConfig;
  setLearningConfig: React.Dispatch<React.SetStateAction<LearningConfig>>;

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
  showToast?: (message: string, durationMs?: number) => void;

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
  learningConfig,
  setLearningConfig,
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
  showToast,
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
        const runtimeDictionaryEntries = updates.dictionary
          ? applyPlatformDictionaryFilter(updates.dictionary)
          : undefined;
        await invoke<string>("update_runtime_config", {
          enablePostProcess: updates.enablePostProcess,
          enableDictionaryEnhancement: updates.enableDictionaryEnhancement,
          llmConfig: updates.llmConfig,
          assistantConfig: updates.assistantConfig,
          enableMuteOtherApps: updates.enableMuteOtherApps,
          dictionary: runtimeDictionaryEntries
            ? buildRuntimeDictionary(runtimeDictionaryEntries, builtinDictionaryDomains)
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

  const resolveSaveConfig = useCallback(
    (overrides: SaveConfigGatewayOverrides = {}): ResolvedSaveConfig => {
      const storageDictionaryFromOverrides = overrides.storageDictionary?.filter(
        (word) => typeof word === "string" && word.trim(),
      );
      const dictionaryEntriesFromStorage = storageDictionaryFromOverrides?.map(parseEntry);
      const rawDictionaryEntries =
        overrides.dictionaryEntries ?? dictionaryEntriesFromStorage ?? dictionary;
      const finalDictionaryEntries = applyPlatformDictionaryFilter(rawDictionaryEntries);
      const finalBuiltinDictionaryDomains = normalizeBuiltinDictionaryDomains(
        overrides.builtinDictionaryDomains ?? builtinDictionaryDomains,
      );
      const finalStorageDictionary =
        storageDictionaryFromOverrides && features.autoLearning
          ? storageDictionaryFromOverrides
          : entriesToStorageFormat(finalDictionaryEntries);
      const finalTheme = (overrides.theme ?? theme) || "light";
      const finalAsrConfig = overrides.asrConfig ?? asrConfig;
      const finalLearningConfig = normalizeLearningConfig(
        overrides.learningConfig ?? learningConfig,
      );

      return {
        apiKey: finalAsrConfig.credentials.qwen_api_key || overrides.apiKey || apiKey,
        fallbackApiKey:
          finalAsrConfig.credentials.sensevoice_api_key
          || overrides.fallbackApiKey
          || fallbackApiKey,
        useRealtime: overrides.useRealtime ?? useRealtime,
        enablePostProcess: overrides.enablePostProcess ?? enablePostProcess,
        enableDictionaryEnhancement:
          overrides.enableDictionaryEnhancement ?? enableDictionaryEnhancement,
        llmConfig: overrides.llmConfig ?? llmConfig,
        assistantConfig: overrides.assistantConfig ?? assistantConfig,
        asrConfig: finalAsrConfig,
        closeAction: overrides.closeAction ?? closeAction ?? null,
        dualHotkeyConfig: overrides.dualHotkeyConfig ?? dualHotkeyConfig,
        learningConfig: finalLearningConfig,
        enableMuteOtherApps: overrides.enableMuteOtherApps ?? enableMuteOtherApps,
        dictionaryEntries: finalDictionaryEntries,
        storageDictionary: finalStorageDictionary,
        runtimeDictionary: buildRuntimeDictionary(
          finalDictionaryEntries,
          finalBuiltinDictionaryDomains,
        ),
        builtinDictionaryDomains: finalBuiltinDictionaryDomains,
        theme: finalTheme,
      };
    },
    [
      apiKey,
      fallbackApiKey,
      useRealtime,
      enablePostProcess,
      enableDictionaryEnhancement,
      llmConfig,
      assistantConfig,
      asrConfig,
      closeAction,
      dualHotkeyConfig,
      learningConfig,
      enableMuteOtherApps,
      dictionary,
      builtinDictionaryDomains,
      theme,
    ],
  );

  const saveConfigThroughGateway = useCallback(
    async (overrides: SaveConfigGatewayOverrides = {}) => {
      const resolved = resolveSaveConfig(overrides);

      await invoke<string>("save_config", {
        apiKey: resolved.apiKey,
        fallbackApiKey: resolved.fallbackApiKey,
        useRealtime: resolved.useRealtime,
        enablePostProcess: resolved.enablePostProcess,
        enableDictionaryEnhancement: resolved.enableDictionaryEnhancement,
        llmConfig: resolved.llmConfig,
        smartCommandConfig: null,
        assistantConfig: resolved.assistantConfig,
        asrConfig: resolved.asrConfig,
        closeAction: resolved.closeAction,
        dualHotkeyConfig: resolved.dualHotkeyConfig,
        learningConfig: resolved.learningConfig,
        enableMuteOtherApps: resolved.enableMuteOtherApps,
        dictionary: resolved.storageDictionary,
        builtinDictionaryDomains: resolved.builtinDictionaryDomains,
        theme: resolved.theme,
      });

      return resolved;
    },
    [resolveSaveConfig],
  );

  const patchConfigFields = useCallback(
    async (patch: ConfigFieldPatchPayload) => {
      await invoke<string>("patch_config_fields", { patch });
    },
    [],
  );

  const loadConfig = useCallback(async () => {
    try {
      let config = await invoke<AppConfig>("load_config");
      try {
        const domains = await fetchBuiltinDomains();
        setBuiltinDomainsSnapshot(domains);
      } catch (error) {
        console.warn("预加载内置词库失败，继续使用当前快照:", error);
      }

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
              VALID_ASR_PROVIDERS.includes(parsedCache.active_provider)
                ? parsedCache.active_provider
                : FALLBACK_ASR_PROVIDER;

            const migratedAsrConfig: AsrConfig = {
              credentials: {
                qwen_api_key: parsedCache.qwen?.api_key || '',
                sensevoice_api_key: parsedCache.siliconflow?.api_key || '',
                doubao_app_id: parsedCache.doubao?.app_id || '',
                doubao_access_token: parsedCache.doubao?.access_token || '',
                // 豆包输入法 ASR 凭据 (自动注册获取，迁移时留空)
                doubao_ime_device_id: '',
                doubao_ime_token: '',
                doubao_ime_cdid: '',
              },
              selection: {
                active_provider: activeProvider,
                enable_fallback: false,
                fallback_provider: null,
              },
              language_mode: parsedCache.language_mode === 'zh' ? 'zh' : 'auto',
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

            await saveConfigThroughGateway({
              apiKey: config.dashscope_api_key || "",
              fallbackApiKey: config.siliconflow_api_key || "",
              useRealtime: config.use_realtime_asr ?? true,
              enablePostProcess: config.enable_llm_post_process ?? false,
              enableDictionaryEnhancement: config.enable_dictionary_enhancement ?? true,
              llmConfig: config.llm_config || DEFAULT_LLM_CONFIG,
              assistantConfig: config.assistant_config || DEFAULT_ASSISTANT_CONFIG,
              asrConfig: migratedAsrConfig,
              closeAction: config.close_action ?? null,
              dualHotkeyConfig: config.dual_hotkey_config || DEFAULT_DUAL_HOTKEY_CONFIG,
              learningConfig: config.learning_config || DEFAULT_LEARNING_CONFIG,
              enableMuteOtherApps: config.enable_mute_other_apps ?? false,
              storageDictionary: mergedDictionary,
              builtinDictionaryDomains: normalizeBuiltinDictionaryDomains(
                config.builtin_dictionary_domains || []
              ),
              theme: config.theme || "light",
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

      const loadedAsrConfig: AsrConfig | null = config.asr_config
        ? {
            ...config.asr_config,
            language_mode: config.asr_config.language_mode === "zh" ? "zh" : "auto",
          }
        : null;

      let effectiveAsrConfig = loadedAsrConfig;
      let asrDidFallback = false;
      if (effectiveAsrConfig) {
        const normalized = normalizeAsrConfigWithFallback(effectiveAsrConfig);
        effectiveAsrConfig = normalized.config;
        asrDidFallback = normalized.didFallback;
        if (asrDidFallback) {
          const fallbackName = getAsrProviderDisplayName(FALLBACK_ASR_PROVIDER);
          const fallbackMessage = `ASR Key 缺失，已自动切换至${fallbackName}`;
          console.warn(`[配置修复] ${fallbackMessage}`);
          showToast?.(fallbackMessage, 2600);
        }
      }
      if (effectiveAsrConfig) {
        setAsrConfig(effectiveAsrConfig);
      }

      setUseRealtime(config.use_realtime_asr ?? false);
      setEnablePostProcess(config.enable_llm_post_process ?? false);
      setEnableDictionaryEnhancement(config.enable_dictionary_enhancement ?? false);

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
          assistant: { keys: [...DEFAULT_DUAL_HOTKEY_CONFIG.assistant.keys] },
        });
      } else {
        setDualHotkeyConfig(DEFAULT_DUAL_HOTKEY_CONFIG);
      }

      const loadedLearningConfig = normalizeLearningConfig(
        config.learning_config || DEFAULT_LEARNING_CONFIG,
      );
      setLearningConfig(loadedLearningConfig);

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
      loadedDictionary = applyPlatformDictionaryFilter(loadedDictionary);
      setDictionary(loadedDictionary);

      const loadedBuiltinDictionaryDomains = normalizeBuiltinDictionaryDomains(
        config.builtin_dictionary_domains || []
      );
      setBuiltinDictionaryDomains(loadedBuiltinDictionaryDomains);

      const loadedDualHotkeyConfig = config.dual_hotkey_config || {
        dictation:
          config.hotkey_config ||
          ({ keys: [...DEFAULT_DUAL_HOTKEY_CONFIG.dictation.keys] as HotkeyKey[] } as const),
        assistant: { keys: [...DEFAULT_DUAL_HOTKEY_CONFIG.assistant.keys] as HotkeyKey[] },
      };

      if (effectiveAsrConfig && isAsrConfigValid(effectiveAsrConfig)) {
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
          asrConfig: effectiveAsrConfig,
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

        // 回退后持久化修正后的配置，避免下次启动重复回退
        if (asrDidFallback) {
          try {
            await saveConfigThroughGateway({
              apiKey: effectiveAsrConfig.credentials.qwen_api_key,
              fallbackApiKey: effectiveAsrConfig.credentials.sensevoice_api_key,
              useRealtime: config.use_realtime_asr ?? true,
              enablePostProcess: config.enable_llm_post_process ?? false,
              enableDictionaryEnhancement: config.enable_dictionary_enhancement ?? true,
              llmConfig: loadedLlmConfig,
              assistantConfig: loadedAssistantConfig,
              asrConfig: effectiveAsrConfig,
              closeAction: config.close_action ?? null,
              dualHotkeyConfig: loadedDualHotkeyConfig,
              learningConfig: loadedLearningConfig,
              enableMuteOtherApps: config.enable_mute_other_apps ?? false,
              dictionaryEntries: loadedDictionary,
              builtinDictionaryDomains: loadedBuiltinDictionaryDomains,
              theme: config.theme || "light",
            });
          } catch (err) {
            console.warn("[配置修复] 回退配置持久化失败:", err);
          }
        }
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
    setLearningConfig,
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
    saveConfigThroughGateway,
    showToast,
  ]);

  const handleSaveConfig = useCallback(async () => {
    try {
      const resolved = await saveConfigThroughGateway();

      console.log("[handleSaveConfig] 保存配置, theme=", theme);

      console.log("[handleSaveConfig] 配置已保存到后端");

      // 不需要更新 dictionary 状态，因为它已经是正确的格式

      if (status === "running") {
        await stopApp();
        await startApp({
          apiKey: resolved.apiKey,
          fallbackApiKey: resolved.fallbackApiKey,
          useRealtime: resolved.useRealtime,
          enablePostProcess: resolved.enablePostProcess,
          enableDictionaryEnhancement: resolved.enableDictionaryEnhancement,
          llmConfig: resolved.llmConfig,
          smartCommandConfig: null,
          assistantConfig: resolved.assistantConfig,
          asrConfig: resolved.asrConfig,
          dualHotkeyConfig: resolved.dualHotkeyConfig,
          enableMuteOtherApps: resolved.enableMuteOtherApps,
          dictionary: resolved.runtimeDictionary,
          theme: resolved.theme,
        });
      }

      setError(null);
      flashSuccessToast();
    } catch (err) {
      setError(String(err));
    }
  }, [
    theme,
    status,
    flashSuccessToast,
    saveConfigThroughGateway,
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
    learningConfig?: LearningConfig;
    enableMuteOtherApps?: boolean;
    dictionaryEntries?: DictionaryEntry[];
    builtinDictionaryDomains?: string[];
    theme?: string;
  }) => {
    // 先取消 debounce timer
    onBeforeImmediateSave?.();

    try {
      const resolved = await saveConfigThroughGateway({
        useRealtime: overrides?.useRealtime,
        enablePostProcess: overrides?.enablePostProcess,
        enableDictionaryEnhancement: overrides?.enableDictionaryEnhancement,
        llmConfig: overrides?.llmConfig,
        assistantConfig: overrides?.assistantConfig,
        asrConfig: overrides?.asrConfig,
        dualHotkeyConfig: overrides?.dualHotkeyConfig,
        learningConfig: overrides?.learningConfig,
        enableMuteOtherApps: overrides?.enableMuteOtherApps,
        dictionaryEntries: overrides?.dictionaryEntries,
        builtinDictionaryDomains: overrides?.builtinDictionaryDomains,
        theme: overrides?.theme,
      });

      if (overrides?.dictionaryEntries) setDictionary(resolved.dictionaryEntries);
      if (overrides?.builtinDictionaryDomains) {
        setBuiltinDictionaryDomains(resolved.builtinDictionaryDomains);
      }
      if (overrides?.theme) setTheme(resolved.theme);

      if (status === "running") {
        await stopApp();
        await startApp({
          apiKey: resolved.apiKey,
          fallbackApiKey: resolved.fallbackApiKey,
          useRealtime: resolved.useRealtime,
          enablePostProcess: resolved.enablePostProcess,
          enableDictionaryEnhancement: resolved.enableDictionaryEnhancement,
          llmConfig: resolved.llmConfig,
          smartCommandConfig: null,
          assistantConfig: resolved.assistantConfig,
          asrConfig: resolved.asrConfig,
          dualHotkeyConfig: resolved.dualHotkeyConfig,
          enableMuteOtherApps: resolved.enableMuteOtherApps,
          dictionary: resolved.runtimeDictionary,
          theme: resolved.theme,
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
    status,
    setDictionary,
    setBuiltinDictionaryDomains,
    setTheme,
    setError,
    saveConfigThroughGateway,
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
        const normalized = normalizeAsrConfigWithFallback(asrConfig);
        if (!isAsrConfigValid(normalized.config)) {
          setError("请先配置 ASR API Key");
          return;
        }
        const effectiveConfig = normalized.config;
        if (normalized.didFallback) {
          const fallbackName = getAsrProviderDisplayName(FALLBACK_ASR_PROVIDER);
          const fallbackMessage = `ASR Key 缺失，已自动切换至${fallbackName}`;
          setAsrConfig(effectiveConfig);
          console.warn(`[配置修复] ${fallbackMessage}`);
          showToast?.(fallbackMessage, 2600);
        }

        const resolved = await saveConfigThroughGateway({
          asrConfig: effectiveConfig,
        });

        await startApp({
          apiKey: resolved.apiKey,
          fallbackApiKey: resolved.fallbackApiKey,
          useRealtime: resolved.useRealtime,
          enablePostProcess: resolved.enablePostProcess,
          enableDictionaryEnhancement: resolved.enableDictionaryEnhancement,
          llmConfig: resolved.llmConfig,
          smartCommandConfig: null,
          assistantConfig: resolved.assistantConfig,
          asrConfig: resolved.asrConfig,
          dualHotkeyConfig: resolved.dualHotkeyConfig,
          enableMuteOtherApps: resolved.enableMuteOtherApps,
          dictionary: resolved.runtimeDictionary,
          theme: resolved.theme,
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
    asrConfig,
    saveConfigThroughGateway,
    setAsrConfig,
    setError,
    setStatus,
    showToast,
    startApp,
    status,
    stopApp,
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
          await patchConfigFields({ closeAction: action });
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
      rememberChoice,
      patchConfigFields,
      setCloseAction,
      setRememberChoice,
      setShowCloseDialog,
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
    patchConfigFields,
  };
}
