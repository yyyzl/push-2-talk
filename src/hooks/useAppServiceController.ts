import { ConfigStore } from "../state/configStore";
import { configValues } from "../state/appConfig";
import { desktop, type StartApp } from "../services/desktop";
import type React from "react";
import { useCallback } from "react";
import type { AppConfig, AppStatus, AsrConfig, AssistantConfig, DictionaryEntry, DisfluencyMode, LlmConfig } from "../types";
import type { ConfigOverrides } from "../contexts/ConfigSaveContext";
import { FALLBACK_ASR_PROVIDER } from "../constants";
import { isAsrConfigValid, normalizeAsrConfigWithFallback, getAsrProviderDisplayName } from "../utils";
import { parseEntry } from "../utils/dictionaryUtils";
import { fetchBuiltinDomains, setBuiltinDomainsSnapshot } from "../utils/builtinDictionary";
import { buildRuntimeDictionary } from "../utils/runtimeDictionary";
import { loadConfiguration, saveConfiguration } from "../state/configActions";

type ConfigFieldPatchPayload = {
  learningEnabled?: boolean;
  theme?: string;
  enableMuteOtherApps?: boolean;
  closeAction?: "close" | "minimize" | null;
  tnlConfig?: {
    disfluencyMode?: DisfluencyMode;
    enableContextHotwords?: boolean;
  };
};

export type UseAppServiceControllerParams = {
  configStore: ConfigStore<AppConfig>;
  setDictionary: React.Dispatch<React.SetStateAction<DictionaryEntry[]>>;
  recentHotwordEntries: string[];
  status: AppStatus;
  setStatus: React.Dispatch<React.SetStateAction<AppStatus>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  enableAutostart: boolean;
  setEnableAutostart: React.Dispatch<React.SetStateAction<boolean>>;
  rememberChoice: boolean;
  setRememberChoice: React.Dispatch<React.SetStateAction<boolean>>;
  setShowCloseDialog: React.Dispatch<React.SetStateAction<boolean>>;
  setShowSuccessToast: React.Dispatch<React.SetStateAction<boolean>>;
  showToast?: (message: string, durationMs?: number) => void;
  onBeforeImmediateSave?: () => void;
};

export function useAppServiceController({ configStore, setDictionary,
  recentHotwordEntries, status, setStatus, setError, enableAutostart, setEnableAutostart,
  rememberChoice, setRememberChoice, setShowCloseDialog, setShowSuccessToast, showToast,
  onBeforeImmediateSave }: UseAppServiceControllerParams) {
  const observedConfig = configStore.getSnapshot().config;
  const { asrConfig, builtinDictionaryDomains } = configValues(observedConfig);
  const setAsrConfig = useCallback((value: AsrConfig) => configStore.edit(current => ({ ...current, asr_config: value }), observedConfig), [configStore, observedConfig]);

  const flashSuccessToast = useCallback(() => {
    setShowSuccessToast(true);
    window.setTimeout(() => setShowSuccessToast(false), 3000);
  }, [setShowSuccessToast]);

  const startApp = useCallback(async (payload: StartApp) => { await desktop.start(payload); }, []);
  const stopApp = useCallback(async () => { await desktop.stop(); }, []);

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
        await desktop.updateRuntime({
          enablePostProcess: updates.enablePostProcess,
          enableDictionaryEnhancement: updates.enableDictionaryEnhancement,
          llmConfig: updates.llmConfig,
          assistantConfig: updates.assistantConfig,
          enableMuteOtherApps: updates.enableMuteOtherApps,
          dictionary: updates.dictionary
            ? buildRuntimeDictionary(
              updates.dictionary,
              builtinDictionaryDomains,
              recentHotwordEntries,
            )
            : undefined,
        });
        return true;
      } catch (err) {
        console.error("热更新配置失败:", err);
        return false;
      }
    },
    [builtinDictionaryDomains, recentHotwordEntries, status],
  );

  const saveConfigThroughGateway = useCallback(async (overrides: ConfigOverrides = {}) => {
    const saved = await saveConfiguration(configStore, desktop, overrides, observedConfig);
    const dictionaryEntries = saved.dictionary.map(parseEntry);
    return {
      ...configValues(saved), dictionaryEntries,
      runtimeDictionary: buildRuntimeDictionary(dictionaryEntries, saved.builtin_dictionary_domains, recentHotwordEntries)
    };
  }, [configStore, observedConfig, recentHotwordEntries]);

  const patchConfigFields = useCallback(async (patch: ConfigFieldPatchPayload) => {
    configStore.edit(current => ({
      ...current,
      ...(patch.theme !== undefined ? { theme: patch.theme } : {}),
      ...(patch.enableMuteOtherApps !== undefined ? { enable_mute_other_apps: patch.enableMuteOtherApps } : {}),
      ...(patch.closeAction !== undefined ? { close_action: patch.closeAction } : {}),
      learning_config: patch.learningEnabled === undefined ? current.learning_config : { ...current.learning_config, enabled: patch.learningEnabled },
      tnl_config: {
        ...current.tnl_config,
        ...(patch.tnlConfig?.disfluencyMode !== undefined ? { disfluency_mode: patch.tnlConfig.disfluencyMode } : {}),
        ...(patch.tnlConfig?.enableContextHotwords !== undefined ? { enable_context_hotwords: patch.tnlConfig.enableContextHotwords } : {}),
      },
    }));
    await configStore.flush(desktop.updateConfig);
  }, [configStore]);

  const loadConfig = useCallback(async () => {
    try {
      try { setBuiltinDomainsSnapshot(await fetchBuiltinDomains()); }
      catch (error) { console.warn("预加载内置词库失败，继续使用当前快照:", error); }
      const { config, dictionary: loadedDictionary, didFallback } = await loadConfiguration(configStore, desktop);
      setDictionary(loadedDictionary);
      try { setEnableAutostart(await desktop.getAutostart()); }
      catch (error) { console.error("获取开机自启状态失败:", error); }
      if (didFallback) {
        showToast?.(`ASR Key 缺失，已自动切换至${getAsrProviderDisplayName(FALLBACK_ASR_PROVIDER)}`, 2600);
      }
      if (isAsrConfigValid(config.asr_config)) {
        try {
          await startApp({
            ...configValues(config), smartCommandConfig: null,
            dictionary: buildRuntimeDictionary(loadedDictionary, config.builtin_dictionary_domains,
              config.tnl_config.enable_context_hotwords ? recentHotwordEntries : []),
          });
          // Startup can refresh provider credentials before the event listener finishes attaching.
          configStore.receive(await desktop.getConfig());
          setStatus("running"); setError(null);
        } catch (error) { setStatus("idle"); setError(String(error)); }
      }
    } catch (error) {
      setError(String(error)); throw error;
    }
  }, [configStore, setDictionary, setEnableAutostart, showToast, recentHotwordEntries, startApp, setStatus, setError]);

  const handleSaveConfig = useCallback(async () => {
    try {
      const resolved = await saveConfigThroughGateway();

      if (status === "running") {
        // The backend validates the requested model before restarting its running service.
        await startApp({
          apiKey: resolved.apiKey,
          fallbackApiKey: resolved.fallbackApiKey,
          useRealtime: resolved.useRealtime,
          enablePostProcess: resolved.enablePostProcess,
          enableDictionaryEnhancement: resolved.enableDictionaryEnhancement,
          llmConfig: resolved.llmConfig,
          smartCommandConfig: null,
          assistantConfig: resolved.assistantConfig,
          searchConfig: resolved.searchConfig,
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
      throw err;
    }
  }, [
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
  const immediatelySaveConfig = useCallback(async (overrides?: ConfigOverrides) => {
    // 先取消 debounce timer
    onBeforeImmediateSave?.();

    try {
      const resolved = await saveConfigThroughGateway(overrides);

      if (overrides?.dictionaryEntries) setDictionary(resolved.dictionaryEntries);

      if (status === "running") {
        // The backend validates the requested model before restarting its running service.
        await startApp({
          apiKey: resolved.apiKey,
          fallbackApiKey: resolved.fallbackApiKey,
          useRealtime: resolved.useRealtime,
          enablePostProcess: resolved.enablePostProcess,
          enableDictionaryEnhancement: resolved.enableDictionaryEnhancement,
          llmConfig: resolved.llmConfig,
          smartCommandConfig: null,
          assistantConfig: resolved.assistantConfig,
          searchConfig: resolved.searchConfig,
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
    setError,
    saveConfigThroughGateway,
    startApp,
    stopApp,
  ]);

  const handleAutostartToggle = useCallback(async () => {
    try {
      const newValue = !enableAutostart;
      await desktop.setAutostart(newValue);
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
          searchConfig: resolved.searchConfig,
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
      await desktop.cancelTranscription();
    } catch (err) {
      setError(String(err));
    }
  }, [setError]);

  const handleCloseAction = useCallback(
    async (action: "close" | "minimize") => {
      if (rememberChoice) {
        try {
          await patchConfigFields({ closeAction: action });
        } catch (err) {
          console.error("保存关闭配置失败:", err);
        }
      }

      setShowCloseDialog(false);
      setRememberChoice(false);

      if (action === "close") {
        await desktop.quit();
      } else {
        await desktop.hide();
      }
    },
    [
      rememberChoice,
      patchConfigFields,
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
