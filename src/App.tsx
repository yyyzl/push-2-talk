// src/App.tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { getVersion } from "@tauri-apps/api/app";
import {
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import type {
  AppConfig,
  AppStatus,
  AsrConfig,
  AssistantConfig,
  DualHotkeyConfig,
  LearningConfig,
  LlmConfig,
  UsageStats,
} from "./types";
import type { AppPage } from "./pages/types";
import {
  DEFAULT_ASSISTANT_CONFIG,
  DEFAULT_DUAL_HOTKEY_CONFIG,
  DEFAULT_LEARNING_CONFIG,
  DEFAULT_LLM_CONFIG,
} from "./constants";
import { loadUsageStats } from "./utils";
import { TopStatusBar } from "./components/layout/TopStatusBar";
import { Sidebar } from "./components/layout/Sidebar";
import { RightPanel } from "./components/layout/RightPanel";
import { resolveGlobalNotice } from "./utils/globalNotice";
import { CloseConfirmDialog } from "./components/modals/CloseConfirmDialog";
import { UpdateModal } from "./components/modals/UpdateModal";
import { useDictionary } from "./hooks/useDictionary";
import { useHotkeyRecording } from "./hooks/useHotkeyRecording";
import { useHistoryController } from "./hooks/useHistoryController";
import { useTauriEventListeners } from "./hooks/useTauriEventListeners";
import { useAppServiceController } from "./hooks/useAppServiceController";
import { useLlmPresets } from "./hooks/useLlmPresets";
import { useUpdater } from "./hooks/useUpdater";
import { DashboardPage } from "./pages/DashboardPage";
import { AsrPage } from "./pages/AsrPage";
import { ModelsPage } from "./pages/ModelsPage";
import { LlmPage } from "./pages/LlmPage";
import { clearPresetOverridesForProvider } from "./utils/presetOverride";
import { AssistantPage } from "./pages/AssistantPage";
import { DictionaryPage } from "./pages/DictionaryPage";
import { HistoryPage } from "./pages/HistoryPage";
import { HotkeysPage } from "./pages/HotkeysPage";
import { PreferencesPage } from "./pages/PreferencesPage";
import { HelpPage } from "./pages/HelpPage";
import { ConfigSaveContext, type ConfigSyncStatus, type ConfigOverrides } from "./contexts/ConfigSaveContext";
import {
  createConfigSyncWindowController,
  scheduleSyncWindowRelease,
  type ConfigSyncWindowSnapshot,
} from "./utils/configSyncWindow";

/** 哨兵值：外部配置更新时设置，applyRuntimeConfig effect 据此跳过并重置基准 */
const EXTERNAL_UPDATE_SENTINEL = "__EXTERNAL_CONFIG_UPDATE__";

function App() {
  const [currentVersion, setCurrentVersion] = useState(() =>
    localStorage.getItem('app_version') || ''
  );
  const [apiKey, setApiKey] = useState("");
  const [fallbackApiKey, setFallbackApiKey] = useState("");

  const [asrConfig, setAsrConfig] = useState<AsrConfig>({
    credentials: {
      qwen_api_key: '',
      sensevoice_api_key: '',
      doubao_app_id: '',
      doubao_access_token: '',
      doubao_ime_device_id: '',
      doubao_ime_token: '',
      doubao_ime_cdid: '',
    },
    selection: {
      active_provider: 'doubao_ime',
      enable_fallback: false,
      fallback_provider: null,
    },
    language_mode: 'auto',
  });

  const [useRealtime, setUseRealtime] = useState(false);
  const [enablePostProcess, setEnablePostProcess] = useState(false);
  const [enableDictionaryEnhancement, setEnableDictionaryEnhancement] = useState(false);
  const [learningConfig, setLearningConfig] = useState<LearningConfig>(DEFAULT_LEARNING_CONFIG);
  const [llmConfig, setLlmConfig] = useState<LlmConfig>(DEFAULT_LLM_CONFIG);
  const [status, setStatus] = useState<AppStatus>("idle");
  const [transcript, setTranscript] = useState("");
  const [originalTranscript, setOriginalTranscript] = useState<string | null>(null);
  const [selectedText, setSelectedText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recordingTime, setRecordingTime] = useState(0);
  const [asrTime, setAsrTime] = useState<number | null>(null);
  const [llmTime, setLlmTime] = useState<number | null>(null);
  const [totalTime, setTotalTime] = useState<number | null>(null);
  const [activePresetName, setActivePresetName] = useState<string | null>(null);
  const [showSuccessToast, setShowSuccessToast] = useState(false);
  const {
    dictionary,
    setDictionary,
    newWord,
    setNewWord,
    duplicateHint,
    setDuplicateHint,
    editingIndex,
    editingValue,
    setEditingValue,
    handleAddWord,
    handleDeleteWord,
    handleStartEdit,
    handleSaveEdit,
    handleCancelEdit,
    handleBatchDelete,
  } = useDictionary();
  const [builtinDictionaryDomains, setBuiltinDictionaryDomains] = useState<string[]>([]);
  const [builtinDictionaryVersion, setBuiltinDictionaryVersion] = useState(0);
  const {
    history,
    setHistory,
    copyToast,
    showToast,
    handleCopyText,
    handleClearHistory,
  } = useHistoryController();
  const [activePage, setActivePage] = useState<AppPage>("dashboard");
  // R8.2 (v4): cross-page focus state — set by ModelsPage callback, consumed by LlmPage useEffect
  // v4 simplification: no "action" field — model selector is inline so just scrolling+activating is enough
  const [pendingPresetFocus, setPendingPresetFocus] = useState<{ presetId: string } | null>(null);
  const navigateToPreset = useCallback((presetId: string) => {
    setPendingPresetFocus({ presetId });
    setActivePage("llm");
  }, []);
  const [showAsrApiKey, setShowAsrApiKey] = useState(false);
  const [showModelsApiKey, setShowModelsApiKey] = useState(false);
  const [showCloseDialog, setShowCloseDialog] = useState(false);
  const [rememberChoice, setRememberChoice] = useState(false);
  const [enableAutostart, setEnableAutostart] = useState(false);
  const [enableMuteOtherApps, setEnableMuteOtherApps] = useState(false);
  const [enableAutoEnter, setEnableAutoEnter] = useState(false);
  const [theme, setTheme] = useState("light");
  const [closeAction, setCloseAction] = useState<"close" | "minimize" | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const {
    updateStatus,
    updateInfo,
    downloadProgress,
    showUpdateModal,
    dismissUpdateModal,
    checkForUpdates,
    downloadAndInstall,
  } = useUpdater({
    onToast: showToast,
    onError: (message) => setError(message),
  });
  // hotkeyConfig 已迁移到 dualHotkeyConfig，不再单独使用
  const [dualHotkeyConfig, setDualHotkeyConfig] = useState<DualHotkeyConfig>(DEFAULT_DUAL_HOTKEY_CONFIG);
  const [assistantConfig, setAssistantConfig] = useState<AssistantConfig>(DEFAULT_ASSISTANT_CONFIG);

  // 创建 ref 用于在 useHotkeyRecording 中访问 wrappedSaveImmediately
  const saveImmediatelyRef = useRef<((overrides?: ConfigOverrides) => Promise<void>) | null>(null);

  const {
    isRecordingHotkey,
    setIsRecordingHotkey,
    recordingMode,
    setRecordingMode,
    recordingKeys,
    hotkeyError,
    resetHotkeyToDefault,
  } = useHotkeyRecording({
    dualHotkeyConfig,
    setDualHotkeyConfig,
    onSaveConfig: async (overrides) => {
      if (saveImmediatelyRef.current) {
        await saveImmediatelyRef.current(overrides);
      }
    },
  });
  const [currentMode, setCurrentMode] = useState<string | null>(null); // 当前转录模式: "normal" | "smartcommand"
  const transcriptEndRef = useRef<HTMLDivElement>(null);
  const hasCheckedUpdateOnStartup = useRef(false);
  const hasLoadedConfigRef = useRef(false);
  const autoSaveTimerRef = useRef<number | null>(null);
  const configSyncWindowControllerRef = useRef(createConfigSyncWindowController());
  const [syncWindowSnapshot, setSyncWindowSnapshot] = useState<ConfigSyncWindowSnapshot>(
    () => configSyncWindowControllerRef.current.snapshot(),
  );

  const syncWindowSnapshotRef = useRef(syncWindowSnapshot);
  useEffect(() => {
    syncWindowSnapshotRef.current = syncWindowSnapshot;
  }, [syncWindowSnapshot]);

  const updateSyncWindowSnapshot = useCallback(() => {
    const nextSnapshot = configSyncWindowControllerRef.current.snapshot();
    const prevSnapshot = syncWindowSnapshotRef.current;

    if (
      prevSnapshot.isSuppressed === nextSnapshot.isSuppressed
      && prevSnapshot.source === nextSnapshot.source
      && prevSnapshot.isExternalSyncing === nextSnapshot.isExternalSyncing
    ) {
      return;
    }

    syncWindowSnapshotRef.current = nextSnapshot;
    setSyncWindowSnapshot(nextSnapshot);
  }, []);
  const releaseConfigSyncWindow = useCallback((token: number) => {
    scheduleSyncWindowRelease({
      token,
      complete: (releasedToken) => {
        configSyncWindowControllerRef.current.complete(releasedToken);
        updateSyncWindowSnapshot();
      },
    });
  }, [updateSyncWindowSnapshot]);

  const handleExternalConfigUpdated = useCallback((_config: AppConfig) => {
    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
    const syncToken = configSyncWindowControllerRef.current.begin("external_config_updated");
    updateSyncWindowSnapshot();
    // 标记 applyRuntimeConfig 基准需要重置，防止外部配置触发冗余的后端热更新
    // （后端已经通过 restart_service_with_config 处理过了）
    lastAppliedConfigHashRef.current = EXTERNAL_UPDATE_SENTINEL;
    releaseConfigSyncWindow(syncToken);
  }, [releaseConfigSyncWindow, updateSyncWindowSnapshot]);
  const statusRef = useRef(status);
  useEffect(() => {
    statusRef.current = status;
  }, [status]);
  const dictionaryRef = useRef(dictionary);
  useEffect(() => {
    dictionaryRef.current = dictionary;
  }, [dictionary]);
  const applyRuntimeConfigRef = useRef<((updates: {
    enablePostProcess?: boolean;
    enableDictionaryEnhancement?: boolean;
    llmConfig?: LlmConfig;
    assistantConfig?: AssistantConfig;
    enableMuteOtherApps?: boolean;
    dictionary?: typeof dictionary;
  }) => Promise<boolean>) | null>(null);
  const handleBuiltinDictionaryUpdated = useCallback(() => {
    setBuiltinDictionaryVersion((prev) => prev + 1);
    if (statusRef.current !== "running") return;
    const applyRuntime = applyRuntimeConfigRef.current;
    if (!applyRuntime) return;
    void applyRuntime({ dictionary: dictionaryRef.current });
  }, []);

  const [usageStats, setUsageStats] = useState<UsageStats>({
    totalRecordingMs: 0,
    totalRecordingCount: 0,
    totalRecognizedChars: 0,
  });
  const {
    activePreset,
    handleAddPreset,
    handleDeletePreset,
    handleUpdateActivePreset,
  } = useLlmPresets({ llmConfig, setLlmConfig });
  const llmConfigRef = useRef(llmConfig);
  useEffect(() => {
    llmConfigRef.current = llmConfig;
  }, [llmConfig]);
  const enablePostProcessRef = useRef(enablePostProcess);
  useEffect(() => {
    enablePostProcessRef.current = enablePostProcess;
  }, [enablePostProcess]);
  const enableDictionaryEnhancementRef = useRef(enableDictionaryEnhancement);
  useEffect(() => {
    enableDictionaryEnhancementRef.current = enableDictionaryEnhancement;
  }, [enableDictionaryEnhancement]);
  const handlePolishingFailed = useCallback((errorMessage: string) => {
    const shortMsg = errorMessage.length > 50
      ? errorMessage.slice(0, 50) + "..."
      : errorMessage;
    showToast(`润色失败：${shortMsg}，已显示原文`);
  }, [showToast]);
  useTauriEventListeners({
    llmConfigRef,
    enablePostProcessRef,
    enableDictionaryEnhancementRef,
    setActivePresetName,
    setStatus,
    setError,
    setTranscript,
    setOriginalTranscript,
    setSelectedText,
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
    setLearningConfig,
    setEnableMuteOtherApps,
    setEnableAutoEnter,
    setTheme,
    setCloseAction,
    setDictionary,
    setDualHotkeyConfig,
    setBuiltinDictionaryDomains,
    onExternalConfigUpdated: handleExternalConfigUpdated,
    onBuiltinDictionaryUpdated: handleBuiltinDictionaryUpdated,
    setHistory,
    setUsageStats,
    onPolishingFailed: handlePolishingFailed,
  });

  // 取消 debounce timer 的回调，供即时保存使用
  const cancelAutoSaveDebounce = useCallback(() => {
    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
    // timer 已清除，用户后续操作正常触发自动保存
  }, []);

  // 全局配置保存状态管理
  const [syncStatus, setSyncStatus] = useState<ConfigSyncStatus>("idle");
  const syncTimeoutRef = useRef<number | null>(null);

  // 清理 syncStatus timeout
  useEffect(() => {
    return () => {
      if (syncTimeoutRef.current) {
        window.clearTimeout(syncTimeoutRef.current);
      }
    };
  }, []);

  const {
    loadConfig,
    handleSaveConfig,
    immediatelySaveConfig,
    handleAutostartToggle,
    handleCloseAction,
    applyRuntimeConfig,
    patchConfigFields,
  } = useAppServiceController({
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
    setEnableAutoEnter,
    theme,
    setTheme,
    closeAction,
    setCloseAction,
    rememberChoice,
    setRememberChoice,
    setShowCloseDialog,
    setShowSuccessToast,
    showToast,
    onBeforeImmediateSave: cancelAutoSaveDebounce,
  });
  useEffect(() => {
    applyRuntimeConfigRef.current = applyRuntimeConfig;
  }, [applyRuntimeConfig]);

  // 包装 immediatelySaveConfig，添加状态管理
  const wrappedSaveImmediately = useCallback(async (overrides?: ConfigOverrides) => {
    // 清理之前的 timeout
    if (syncTimeoutRef.current) {
      window.clearTimeout(syncTimeoutRef.current);
      syncTimeoutRef.current = null;
    }

    setSyncStatus("syncing");

    try {
      await immediatelySaveConfig(overrides);
      setSyncStatus("success");

      // 1.5s 后回到 idle
      syncTimeoutRef.current = window.setTimeout(() => {
        setSyncStatus("idle");
      }, 1500);
    } catch (err) {
      setSyncStatus("error");

      // 2s 后回到 idle
      syncTimeoutRef.current = window.setTimeout(() => {
        setSyncStatus("idle");
      }, 2000);

      throw err; // 重新抛出以便调用方处理
    }
  }, [immediatelySaveConfig]);

  const saveFieldPatchWithStatus = useCallback(async (patch: {
    learningEnabled?: boolean;
    theme?: string;
    enableMuteOtherApps?: boolean;
    enableAutoEnter?: boolean;
    closeAction?: "close" | "minimize" | null;
  }) => {
    cancelAutoSaveDebounce();
    const syncToken = configSyncWindowControllerRef.current.begin("external_config_updated");
    updateSyncWindowSnapshot();

    if (syncTimeoutRef.current) {
      window.clearTimeout(syncTimeoutRef.current);
      syncTimeoutRef.current = null;
    }

    const previousTheme = theme;
    const previousEnableMuteOtherApps = enableMuteOtherApps;
    const previousEnableAutoEnter = enableAutoEnter;
    const previousLearningConfig = learningConfig;
    const previousCloseAction = closeAction;

    if (typeof patch.theme === "string") {
      setTheme(patch.theme);
    }
    if (typeof patch.enableMuteOtherApps === "boolean") {
      setEnableMuteOtherApps(patch.enableMuteOtherApps);
    }
    if (typeof patch.enableAutoEnter === "boolean") {
      setEnableAutoEnter(patch.enableAutoEnter);
    }
    if (typeof patch.learningEnabled === "boolean") {
      const nextLearningEnabled = patch.learningEnabled;
      setLearningConfig((prev) => ({ ...prev, enabled: nextLearningEnabled }));
    }
    if (patch.closeAction !== undefined) {
      setCloseAction(patch.closeAction);
    }

    setSyncStatus("syncing");

    try {
      await patchConfigFields(patch);
      setSyncStatus("success");
      syncTimeoutRef.current = window.setTimeout(() => {
        setSyncStatus("idle");
      }, 1500);
    } catch (err) {
      if (typeof patch.theme === "string") {
        setTheme(previousTheme);
      }
      if (typeof patch.enableMuteOtherApps === "boolean") {
        setEnableMuteOtherApps(previousEnableMuteOtherApps);
      }
      if (typeof patch.enableAutoEnter === "boolean") {
        setEnableAutoEnter(previousEnableAutoEnter);
      }
      if (typeof patch.learningEnabled === "boolean") {
        setLearningConfig(previousLearningConfig);
      }
      if (patch.closeAction !== undefined) {
        setCloseAction(previousCloseAction);
      }

      setSyncStatus("error");
      syncTimeoutRef.current = window.setTimeout(() => {
        setSyncStatus("idle");
      }, 2000);
      throw err;
    } finally {
      releaseConfigSyncWindow(syncToken);
    }
  }, [
    theme,
    enableMuteOtherApps,
    enableAutoEnter,
    learningConfig,
    closeAction,
    patchConfigFields,
    setTheme,
    setEnableMuteOtherApps,
    setEnableAutoEnter,
    setLearningConfig,
    setCloseAction,
    cancelAutoSaveDebounce,
    releaseConfigSyncWindow,
    updateSyncWindowSnapshot,
  ]);

  // 更新 ref 以便 useHotkeyRecording 可以访问
  useEffect(() => {
    saveImmediatelyRef.current = wrappedSaveImmediately;
  }, [wrappedSaveImmediately]);

  const handleSaveConfigRef = useRef(handleSaveConfig);
  useEffect(() => {
    handleSaveConfigRef.current = handleSaveConfig;
  }, [handleSaveConfig]);
  useEffect(() => {
    // 双栏模式下容器高度动态变化，scrollIntoView 会导致页面级滚动，跳过
    if (transcriptEndRef.current && !originalTranscript) {
      transcriptEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [transcript, originalTranscript]);
  useEffect(() => {
    if (hasLoadedConfigRef.current) return;
    hasLoadedConfigRef.current = true;

    const init = async () => {
      try {
        await new Promise(resolve => setTimeout(resolve, 100));
        const syncToken = configSyncWindowControllerRef.current.begin("initial_load");
        updateSyncWindowSnapshot();
        try {
          await loadConfig();
        } finally {
          releaseConfigSyncWindow(syncToken);
        }
        // 启动时自动检查更新（只执行一次）
        if (!hasCheckedUpdateOnStartup.current) {
          hasCheckedUpdateOnStartup.current = true;
          await checkForUpdates({ openModal: true, silentOnNoUpdate: true, silentOnError: true });
        }
      } catch (err) {
        hasLoadedConfigRef.current = false;
        console.error("初始化失败:", err);
        setError("应用初始化失败: " + String(err));
      }
    };
    init();
  }, [checkForUpdates, loadConfig, releaseConfigSyncWindow]);
  useEffect(() => {
    getVersion().then(v => {
      setCurrentVersion(v);
      localStorage.setItem('app_version', v);
    }).catch(() => { });

    // 从 Tauri 后端加载统计数据
    loadUsageStats().then(stats => {
      setUsageStats(stats);
    }).catch(error => {
      console.error('加载统计数据失败:', error);
    });
  }, []);
  useEffect(() => {
    let interval: ReturnType<typeof setInterval>;
    if (status === "recording") {
      setRecordingTime(0);
      interval = setInterval(() => {
        setRecordingTime(prev => prev + 1);
      }, 1000);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [status]);

  useEffect(() => {
    if (status !== "recording" && status !== "transcribing") return;
    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
  }, [status]);

  // 热更新：配置变更时在 running 状态下立即应用
  // 使用 hash 去重，避免配置未变时重复调用后端
  // 注意：null 表示未初始化（首次进入 running 时设置基准，不触发 apply）
  const lastAppliedConfigHashRef = useRef<string | null>(null);

  useEffect(() => {
    if (!hasLoadedConfigRef.current) return;
    if (status !== "running") return;

    const configHash = JSON.stringify({
      enablePostProcess,
      enableDictionaryEnhancement,
      llmConfig,
      assistantConfig,
      enableMuteOtherApps,
      dictionary,
      builtinDictionaryDomains,
    });

    // 首次进入 running 时初始化基准（不触发 apply，因为后端启动时已加载配置）
    if (lastAppliedConfigHashRef.current === null) {
      lastAppliedConfigHashRef.current = configHash;
      return;
    }

    // 外部配置更新（托盘切换等）：后端已处理，仅重置基准，跳过冗余 apply
    if (lastAppliedConfigHashRef.current === EXTERNAL_UPDATE_SENTINEL) {
      lastAppliedConfigHashRef.current = configHash;
      return;
    }

    // 配置未变，跳过
    if (configHash === lastAppliedConfigHashRef.current) return;

    // 配置变了，应用后再更新基准（确保成功后才更新，失败时允许重试）
    // 注意：builtinDictionaryDomains 在 hash 中但不传给 applyRuntimeConfig
    // 因为它已在 useAppServiceController 内部通过闭包捕获
    void applyRuntimeConfig({
      enablePostProcess,
      enableDictionaryEnhancement,
      llmConfig,
      assistantConfig,
      enableMuteOtherApps,
      dictionary,
    }).then((success) => {
      if (success) {
        // 成功后才更新基准，确保下次相同配置不会重复触发
        lastAppliedConfigHashRef.current = configHash;
      }
      // 失败时不更新基准，下次相同配置会重试
    });
  }, [status, enablePostProcess, enableDictionaryEnhancement, llmConfig, assistantConfig, enableMuteOtherApps, dictionary, builtinDictionaryDomains, applyRuntimeConfig]);

  // Auto-save config after changes (debounced).
  // While the service is running, this applies changes by restarting the backend.
  useEffect(() => {
    console.log(
      "[App.tsx] 自动保存 useEffect 触发, theme=",
      theme,
      "hasLoaded=",
      hasLoadedConfigRef.current,
      "syncSuppressed=",
      configSyncWindowControllerRef.current.isSuppressed(),
      "syncSource=",
      configSyncWindowControllerRef.current.currentSource(),
    );
    if (!hasLoadedConfigRef.current) return;
    if (status === "recording" || status === "transcribing") return;

    if (configSyncWindowControllerRef.current.isSuppressed()) {
      console.log(
        "[App.tsx] 同步窗口中，跳过自动保存, source=",
        configSyncWindowControllerRef.current.currentSource(),
      );
      return;
    }

    console.log("[App.tsx] 准备 debounce 保存配置, theme=", theme);

    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
    }

    autoSaveTimerRef.current = window.setTimeout(() => {
      if (statusRef.current === "recording" || statusRef.current === "transcribing") return;
      console.log("[App.tsx] debounce 到期，执行 handleSaveConfig");
      void handleSaveConfigRef.current();
    }, 900);

    return () => {
      if (autoSaveTimerRef.current) window.clearTimeout(autoSaveTimerRef.current);
    };
  }, [
    asrConfig,
    useRealtime,
    enablePostProcess,
    enableDictionaryEnhancement,
    llmConfig,
    assistantConfig,
    dictionary,
    builtinDictionaryDomains,
    enableMuteOtherApps,
    closeAction,
    dualHotkeyConfig,
    learningConfig,
    theme,
  ]);

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };
  const isRecording = status === "recording";
  const isTranscribing = status === "transcribing";
  const isPolishing = status === "polishing";
  const isAssistantProcessing = status === "assistant_processing";
  const isConfigLocked = isRecording || isTranscribing || isPolishing || isAssistantProcessing;
  const globalNotice = resolveGlobalNotice({
    syncWindowSource: syncWindowSnapshot.source,
    syncStatus,
    updateStatus,
    updateDownloadProgress: downloadProgress,
  });

  const navigate = (page: AppPage) => setActivePage(page);

  const content = (() => {
    switch (activePage) {
      case "dashboard":
        return (
          <DashboardPage
            transcript={transcript}
            originalTranscript={originalTranscript}
            selectedText={selectedText}
            currentMode={currentMode}
            asrTime={asrTime}
            llmTime={llmTime}
            totalTime={totalTime}
            activePresetName={activePresetName}
            transcriptEndRef={transcriptEndRef}
            onCopyText={handleCopyText}
            history={history}
            onOpenHistory={() => navigate("history")}
            enablePostProcess={enablePostProcess}
            enableDictionaryEnhancement={enableDictionaryEnhancement}
          />
        );
      case "asr":
        return (
          <AsrPage
            asrConfig={asrConfig}
            setAsrConfig={setAsrConfig}
            showApiKey={showAsrApiKey}
            setShowApiKey={setShowAsrApiKey}
            isRunning={isConfigLocked}
          />
        );
      case "models":
        return (
          <ModelsPage
            sharedConfig={llmConfig.shared}
            setSharedConfig={(newShared) => {
              if (typeof newShared === 'function') {
                setLlmConfig((prev) => ({ ...prev, shared: newShared(prev.shared) }));
              } else {
                setLlmConfig((prev) => ({ ...prev, shared: newShared }));
              }
            }}
            presets={llmConfig.presets}
            onClearPresetOverridesForProvider={(providerId) => {
              // R5.3 (方案 A): 删除 provider 时同步清空所有引用此 provider 的 preset 覆盖
              setLlmConfig((prev) => ({
                ...prev,
                presets: clearPresetOverridesForProvider(prev.presets, providerId),
              }));
            }}
            onNavigateToPreset={navigateToPreset}
            showApiKey={showModelsApiKey}
            setShowApiKey={setShowModelsApiKey}
            isRunning={isConfigLocked}
          />
        );
      case "llm":
        return (
          <LlmPage
            llmConfig={llmConfig}
            setLlmConfig={setLlmConfig}
            activePreset={activePreset}
            handleAddPreset={handleAddPreset}
            handleDeletePreset={handleDeletePreset}
            handleUpdateActivePreset={handleUpdateActivePreset}
            onNavigateToModels={() => setActivePage("models")}
            pendingFocus={pendingPresetFocus}
            onFocusConsumed={() => setPendingPresetFocus(null)}
            isRunning={isConfigLocked}
          />
        );
      case "assistant":
        return (
          <AssistantPage
            assistantConfig={assistantConfig}
            setAssistantConfig={setAssistantConfig}
            sharedConfig={llmConfig.shared}
            onNavigateToModels={() => setActivePage("models")}
            isRunning={isConfigLocked}
          />
        );
      case "dictionary":
        return (
          <DictionaryPage
            dictionary={dictionary}
            newWord={newWord}
            setNewWord={setNewWord}
            duplicateHint={duplicateHint}
            setDuplicateHint={setDuplicateHint}
            editingIndex={editingIndex}
            editingValue={editingValue}
            setEditingValue={setEditingValue}
            handleAddWord={handleAddWord}
            handleDeleteWord={handleDeleteWord}
            handleStartEdit={handleStartEdit}
            handleSaveEdit={handleSaveEdit}
            handleCancelEdit={handleCancelEdit}
            handleBatchDelete={handleBatchDelete}
            builtinDictionaryDomains={builtinDictionaryDomains}
            setBuiltinDictionaryDomains={setBuiltinDictionaryDomains}
            builtinDictionaryVersion={builtinDictionaryVersion}
            isRunning={isConfigLocked}
          />
        );
      case "history":
        return (
          <HistoryPage history={history} onCopyText={handleCopyText} onClear={handleClearHistory} />
        );
      case "hotkeys":
        return (
          <HotkeysPage
            status={status}
            isRecordingHotkey={isRecordingHotkey}
            setIsRecordingHotkey={setIsRecordingHotkey}
            recordingMode={recordingMode}
            setRecordingMode={setRecordingMode}
            recordingKeys={recordingKeys}
            hotkeyError={hotkeyError}
            dualHotkeyConfig={dualHotkeyConfig}
            resetHotkeyToDefault={resetHotkeyToDefault}
          />
        );
      case "preferences":
        return (
          <PreferencesPage
            status={status}
            theme={theme}
            learningConfig={learningConfig}
            setLearningConfig={setLearningConfig}
            setTheme={async (newTheme) => {
              console.log("[App.tsx] setTheme 被调用, newTheme=", newTheme);
              await saveFieldPatchWithStatus({ theme: newTheme });
            }}
            enableAutostart={enableAutostart}
            onToggleAutostart={() => {
              void handleAutostartToggle();
            }}
            enableMuteOtherApps={enableMuteOtherApps}
            onSetEnableMuteOtherApps={async (next) => {
              await saveFieldPatchWithStatus({ enableMuteOtherApps: next });
            }}
            enableAutoEnter={enableAutoEnter}
            onSetEnableAutoEnter={async (next) => {
              await saveFieldPatchWithStatus({ enableAutoEnter: next });
            }}
            updateStatus={updateStatus}
            updateInfo={updateInfo}
            currentVersion={currentVersion}
            onCheckUpdate={() => {
              void checkForUpdates({ openModal: false });
            }}
            onDownloadAndInstall={() => {
              void downloadAndInstall();
            }}
            sharedConfig={llmConfig.shared}
            onSetLearningEnabled={async (enabled) => {
              await saveFieldPatchWithStatus({ learningEnabled: enabled });
            }}
            onNavigateToModels={() => setActivePage("models")}
          />
        );
      case "help":
        return <HelpPage />;
      default:
        return null;
    }
  })();

  return (
    <ConfigSaveContext.Provider
      value={{
        saveImmediately: wrappedSaveImmediately,
        syncStatus,
        isSaving: syncStatus === "syncing",
        isExternalSyncing: syncWindowSnapshot.isExternalSyncing,
        syncWindowSource: syncWindowSnapshot.source,
      }}
    >
      <div className="h-screen w-full bg-[var(--paper)] text-[var(--ink)] font-serif flex">
        <Sidebar
          collapsed={sidebarCollapsed}
          onToggleCollapsed={() => setSidebarCollapsed((v) => !v)}
          activePage={activePage}
          onNavigate={navigate}
          updateStatus={updateStatus}
        />

        <div className="flex-1 min-w-0 flex flex-col h-screen overflow-hidden">
          <TopStatusBar
            status={status}
            recordingTime={recordingTime}
            formatTime={formatTime}
            usageStats={usageStats}
            globalNotice={globalNotice}
          />

          <div className="flex-1 min-h-0 flex overflow-hidden">
            <main className="flex-1 min-w-0 min-h-0 overflow-y-auto custom-scroll p-6">
              {error && (
                <div className="mx-auto max-w-3xl mb-6 flex items-center gap-3 p-4 bg-red-50 border border-red-100 rounded-2xl text-red-700 text-sm font-semibold">
                  <AlertCircle size={18} />
                  <span>{error}</span>
                </div>
              )}

              {content}
            </main>

            {activePage === "dashboard" && (
              <RightPanel
                asrConfig={asrConfig}
                setAsrConfig={setAsrConfig}
                useRealtime={useRealtime}
                setUseRealtime={setUseRealtime}
                enablePostProcess={enablePostProcess}
                setEnablePostProcess={setEnablePostProcess}
                enableDictionaryEnhancement={enableDictionaryEnhancement}
                setEnableDictionaryEnhancement={setEnableDictionaryEnhancement}
                llmConfig={llmConfig}
                setLlmConfig={setLlmConfig}
                dualHotkeyConfig={dualHotkeyConfig}
                dictionary={dictionary}
                newWord={newWord}
                setNewWord={setNewWord}
                onAddWord={handleAddWord}
                onNavigate={navigate}
                isRunning={isConfigLocked}
              />
            )}
          </div>
        </div>

        <div
          className={`fixed top-6 left-1/2 -translate-x-1/2 pointer-events-none transition-all duration-500 z-50 ${showSuccessToast ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-4"
            }`}
        >
          <div className="bg-white/90 backdrop-blur text-emerald-700 px-4 py-2 rounded-full shadow-xl border border-emerald-100 flex items-center gap-2 text-sm font-bold">
            <CheckCircle2 size={16} className="fill-emerald-100" />
            <span>配置已保存成功</span>
          </div>
        </div>
        {/* Close Confirmation Dialog */}
        <CloseConfirmDialog
          open={showCloseDialog}
          rememberChoice={rememberChoice}
          onRememberChoiceChange={setRememberChoice}
          onDismiss={() => setShowCloseDialog(false)}
          onResetRememberChoice={() => setRememberChoice(false)}
          onCloseApp={() => { void handleCloseAction("close"); }}
          onMinimizeToTray={() => { void handleCloseAction("minimize"); }}
        />

        {/* Update Modal */}
        <UpdateModal
          open={showUpdateModal}
          updateInfo={updateInfo}
          updateStatus={updateStatus}
          downloadProgress={downloadProgress}
          onDismiss={dismissUpdateModal}
          onDownloadAndInstall={() => { void downloadAndInstall(); }}
        />

        {/* Global Toast */}
        {copyToast && (
          <div className="fixed bottom-8 left-1/2 -translate-x-1/2 z-[100] bg-slate-900 text-white px-4 py-2 rounded-full text-sm font-medium shadow-lg animate-in fade-in zoom-in duration-200">
            {copyToast}
          </div>
        )}
      </div>
    </ConfigSaveContext.Provider>
  );
}
export default App;
