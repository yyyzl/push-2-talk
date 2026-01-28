// src/App.tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { getVersion } from "@tauri-apps/api/app";
import {
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import type {
  AppStatus,
  AsrConfig,
  AssistantConfig,
  DualHotkeyConfig,
  LlmConfig,
  UsageStats,
} from "./types";
import type { AppPage } from "./pages/types";
import {
  DEFAULT_ASSISTANT_CONFIG,
  DEFAULT_DUAL_HOTKEY_CONFIG,
  DEFAULT_LLM_CONFIG,
} from "./constants";
import { loadUsageStats } from "./utils";
import { TopStatusBar } from "./components/layout/TopStatusBar";
import { Sidebar } from "./components/layout/Sidebar";
import { RightPanel } from "./components/layout/RightPanel";
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
import { AssistantPage } from "./pages/AssistantPage";
import { DictionaryPage } from "./pages/DictionaryPage";
import { HistoryPage } from "./pages/HistoryPage";
import { HotkeysPage } from "./pages/HotkeysPage";
import { PreferencesPage } from "./pages/PreferencesPage";
import { HelpPage } from "./pages/HelpPage";
import { ConfigSaveContext, type ConfigSyncStatus, type ConfigOverrides } from "./contexts/ConfigSaveContext";
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
    },
    selection: {
      active_provider: 'qwen',
      enable_fallback: false,
      fallback_provider: null,
    },
  });

  const [useRealtime, setUseRealtime] = useState(false);
  const [enablePostProcess, setEnablePostProcess] = useState(false);
  const [enableDictionaryEnhancement, setEnableDictionaryEnhancement] = useState(true);
  const [llmConfig, setLlmConfig] = useState<LlmConfig>(DEFAULT_LLM_CONFIG);
  const [status, setStatus] = useState<AppStatus>("idle");
  const [transcript, setTranscript] = useState("");
  const [originalTranscript, setOriginalTranscript] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recordingTime, setRecordingTime] = useState(0);
  const [asrTime, setAsrTime] = useState<number | null>(null);
  const [llmTime, setLlmTime] = useState<number | null>(null);
  const [totalTime, setTotalTime] = useState<number | null>(null);
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
  const {
    history,
    setHistory,
    copyToast,
    showToast,
    handleCopyText,
    handleClearHistory,
  } = useHistoryController();
  const [activePage, setActivePage] = useState<AppPage>("dashboard");
  const [showAsrApiKey, setShowAsrApiKey] = useState(false);
  const [showModelsApiKey, setShowModelsApiKey] = useState(false);
  const [showCloseDialog, setShowCloseDialog] = useState(false);
  const [rememberChoice, setRememberChoice] = useState(false);
  const [enableAutostart, setEnableAutostart] = useState(false);
  const [enableMuteOtherApps, setEnableMuteOtherApps] = useState(false);
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
    apiKey,
    fallbackApiKey,
    useRealtime,
    enablePostProcess,
    llmConfig,
    assistantConfig,
    asrConfig,
    enableMuteOtherApps,
    closeAction,
    dictionary,
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
  const skipNextAutoSaveRef = useRef(true);
  const autoSaveTimerRef = useRef<number | null>(null);
  const statusRef = useRef(status);
  useEffect(() => {
    statusRef.current = status;
  }, [status]);

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
  useTauriEventListeners({
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
  });

  // 取消 debounce timer 的回调，供即时保存使用
  const cancelAutoSaveDebounce = useCallback(() => {
    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
    skipNextAutoSaveRef.current = true;
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
    dictionary,
    setDictionary,
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
    onBeforeImmediateSave: cancelAutoSaveDebounce,
  });

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
    const init = async () => {
      try {
        await new Promise(resolve => setTimeout(resolve, 100));
        await loadConfig();
        hasLoadedConfigRef.current = true;
        skipNextAutoSaveRef.current = true;
        // 启动时自动检查更新（只执行一次）
        if (!hasCheckedUpdateOnStartup.current) {
          hasCheckedUpdateOnStartup.current = true;
          await checkForUpdates({ openModal: true, silentOnNoUpdate: true, silentOnError: true });
        }
      } catch (err) {
        console.error("初始化失败:", err);
        setError("应用初始化失败: " + String(err));
      }
    };
    init();
  }, []);
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

  // 热更新：配置变更时立即应用到运行时（无需等待 debounce）
  useEffect(() => {
    if (!hasLoadedConfigRef.current) return;
    if (status !== "running") return;

    void applyRuntimeConfig({
      enablePostProcess,
      enableDictionaryEnhancement,
      llmConfig,
      assistantConfig,
      enableMuteOtherApps,
      dictionary,
    });
  }, [enablePostProcess, enableDictionaryEnhancement, llmConfig, assistantConfig, enableMuteOtherApps, dictionary, status, applyRuntimeConfig]);

  // Auto-save config after changes (debounced).
  // While the service is running, this applies changes by restarting the backend.
  useEffect(() => {
    console.log("[App.tsx] 自动保存 useEffect 触发, theme=", theme, "hasLoaded=", hasLoadedConfigRef.current, "skip=", skipNextAutoSaveRef.current);
    if (!hasLoadedConfigRef.current) return;
    if (status === "recording" || status === "transcribing") return;
    if (skipNextAutoSaveRef.current) {
      skipNextAutoSaveRef.current = false;
      console.log("[App.tsx] 跳过本次自动保存 (skipNextAutoSaveRef)");
      return;
    }

    console.log("[App.tsx] 准备 debounce 保存配置, theme=", theme);

    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
    }

    autoSaveTimerRef.current = window.setTimeout(() => {
      // handleSaveConfig may normalize some state (e.g. dictionary trim) and/or trigger backend restarts.
      // Skip one follow-up auto-save to avoid save loops.
      skipNextAutoSaveRef.current = true;
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
    enableMuteOtherApps,
    closeAction,
    dualHotkeyConfig,
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

  const navigate = (page: AppPage) => setActivePage(page);

  const content = (() => {
    switch (activePage) {
      case "dashboard":
        return (
          <DashboardPage
            transcript={transcript}
            originalTranscript={originalTranscript}
            currentMode={currentMode}
            asrTime={asrTime}
            llmTime={llmTime}
            totalTime={totalTime}
            activePresetName={activePreset?.name || null}
            transcriptEndRef={transcriptEndRef}
            onCopyText={handleCopyText}
            history={history}
            onOpenHistory={() => navigate("history")}
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
            setTheme={async (newTheme) => {
              console.log("[App.tsx] setTheme 被调用, newTheme=", newTheme);
              setTheme(newTheme);
              await wrappedSaveImmediately({ theme: newTheme });
            }}
            enableAutostart={enableAutostart}
            onToggleAutostart={() => {
              void handleAutostartToggle();
            }}
            enableMuteOtherApps={enableMuteOtherApps}
            setEnableMuteOtherApps={setEnableMuteOtherApps}
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
