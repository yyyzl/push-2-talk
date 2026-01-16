// src/App.tsx
import { useState, useEffect, useRef } from "react";
import { getVersion } from "@tauri-apps/api/app";
import {
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import type {
  AsrCache,
  AsrConfig,
  AssistantConfig,
  DualHotkeyConfig,
  LlmConfig,
  UsageStats,
} from "./types";
import type { AppPage } from "./pages/types";
import {
  ASR_CACHE_STORAGE_KEY,
  DEFAULT_ASSISTANT_CONFIG,
  DEFAULT_ASR_CACHE,
  DEFAULT_DUAL_HOTKEY_CONFIG,
  DEFAULT_LLM_CONFIG,
  DEFAULT_PRESETS,
  DEFAULT_SMART_COMMAND_CONFIG,
} from "./constants";
import { loadUsageStats } from "./utils";
import { TopStatusBar } from "./components/layout/TopStatusBar";
import { Sidebar } from "./components/layout/Sidebar";
import { RightPanel } from "./components/layout/RightPanel";
import { CloseConfirmDialog } from "./components/modals/CloseConfirmDialog";
import { useDictionary } from "./hooks/useDictionary";
import { useHotkeyRecording } from "./hooks/useHotkeyRecording";
import { useHistoryController } from "./hooks/useHistoryController";
import { useTauriEventListeners } from "./hooks/useTauriEventListeners";
import { useAppServiceController } from "./hooks/useAppServiceController";
import { useLlmPresets } from "./hooks/useLlmPresets";
import { useUpdater } from "./hooks/useUpdater";
import { DashboardPage } from "./pages/DashboardPage";
import { AsrPage } from "./pages/AsrPage";
import { LlmPage } from "./pages/LlmPage";
import { AssistantPage } from "./pages/AssistantPage";
import { DictionaryPage } from "./pages/DictionaryPage";
import { HistoryPage } from "./pages/HistoryPage";
import { HotkeysPage } from "./pages/HotkeysPage";
import { PreferencesPage } from "./pages/PreferencesPage";
function App() {
  const [currentVersion, setCurrentVersion] = useState(() =>
    localStorage.getItem('app_version') || ''
  );
  const [apiKey, setApiKey] = useState("");
  const [fallbackApiKey, setFallbackApiKey] = useState("");
  // Cache for different ASR providers to prevent data loss when switching
  const CACHE_STORAGE_KEY = ASR_CACHE_STORAGE_KEY;
  const [asrCache, setAsrCache] = useState<AsrCache>(() => {
    // Initialize from localStorage to persist across sessions
    try {
      const saved = localStorage.getItem(CACHE_STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        return {
          ...DEFAULT_ASR_CACHE,
          ...parsed,
          active_provider: parsed.active_provider || 'qwen',
        };
      }
    } catch (e) {
      console.error("Failed to load ASR cache:", e);
    }
    return DEFAULT_ASR_CACHE;
  });
  const [asrConfig, setAsrConfig] = useState<AsrConfig>(() => {
    const provider = asrCache.active_provider;
    return {
      primary:
        provider === 'qwen'
          ? { provider: 'qwen', api_key: asrCache.qwen.api_key }
          : provider === 'doubao'
            ? { provider: 'doubao', api_key: '', app_id: asrCache.doubao.app_id, access_token: asrCache.doubao.access_token }
            : { provider: 'siliconflow', api_key: asrCache.siliconflow.api_key },
      fallback: null,
      enable_fallback: false,
    };
  });
  const [useRealtime, setUseRealtime] = useState(true);
  const [enablePostProcess, setEnablePostProcess] = useState(false);
  const [llmConfig, setLlmConfig] = useState<LlmConfig>(DEFAULT_LLM_CONFIG);
  // smartCommandConfig 保留用于向后兼容（加载旧配置时不会报错），使用常量替代 useState
  const smartCommandConfig = DEFAULT_SMART_COMMAND_CONFIG;
  const [status, setStatus] = useState<"idle" | "running" | "recording" | "transcribing">("idle");
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
  const [showApiKey, setShowApiKey] = useState(false);
  const [showCloseDialog, setShowCloseDialog] = useState(false);
  const [rememberChoice, setRememberChoice] = useState(false);
  const [enableAutostart, setEnableAutostart] = useState(false);
  const [enableMuteOtherApps, setEnableMuteOtherApps] = useState(false);
  const [closeAction, setCloseAction] = useState<"close" | "minimize" | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const {
    updateStatus,
    updateInfo,
    checkForUpdates,
    downloadAndInstall,
  } = useUpdater({
    onToast: showToast,
    onError: (message) => setError(message),
  });
  // hotkeyConfig 已迁移到 dualHotkeyConfig，不再单独使用
  const [dualHotkeyConfig, setDualHotkeyConfig] = useState<DualHotkeyConfig>(DEFAULT_DUAL_HOTKEY_CONFIG);
  const [assistantConfig, setAssistantConfig] = useState<AssistantConfig>(DEFAULT_ASSISTANT_CONFIG);
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
    smartCommandConfig,
    assistantConfig,
    asrConfig,
    enableMuteOtherApps,
    dualHotkeyConfig,
    setDualHotkeyConfig,
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

  const [usageStats, setUsageStats] = useState<UsageStats>(() => loadUsageStats());
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
  useTauriEventListeners({
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
  });

  const {
    loadConfig,
    handleSaveConfig,
    handleAutostartToggle,
    handleStartStop,
    handleCancelTranscription,
    handleCloseAction,
  } = useAppServiceController({
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
  });
  const handleSaveConfigRef = useRef(handleSaveConfig);
  useEffect(() => {
    handleSaveConfigRef.current = handleSaveConfig;
  }, [handleSaveConfig]);
  // Auto-save asrCache to localStorage whenever it changes
  useEffect(() => {
    localStorage.setItem(CACHE_STORAGE_KEY, JSON.stringify(asrCache));
  }, [asrCache]);
  useEffect(() => {
    if (transcriptEndRef.current) {
      transcriptEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [transcript]);
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
          await checkForUpdates({ openModal: false, silentOnNoUpdate: true, silentOnError: true });
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
    }).catch(() => {});
  }, []);
  useEffect(() => {
    let interval: number;
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

  // Auto-save config after changes (debounced).
  // While the service is running, this applies changes by restarting the backend.
  useEffect(() => {
    if (!hasLoadedConfigRef.current) return;
    if (status === "recording" || status === "transcribing") return;
    if (skipNextAutoSaveRef.current) {
      skipNextAutoSaveRef.current = false;
      return;
    }

    if (autoSaveTimerRef.current) {
      window.clearTimeout(autoSaveTimerRef.current);
    }

    autoSaveTimerRef.current = window.setTimeout(() => {
      // handleSaveConfig may normalize some state (e.g. dictionary trim) and/or trigger backend restarts.
      // Skip one follow-up auto-save to avoid save loops.
      skipNextAutoSaveRef.current = true;
      if (statusRef.current === "recording" || statusRef.current === "transcribing") return;
      void handleSaveConfigRef.current();
    }, 900);

    return () => {
      if (autoSaveTimerRef.current) window.clearTimeout(autoSaveTimerRef.current);
    };
  }, [
    asrConfig,
    useRealtime,
    enablePostProcess,
    llmConfig,
    assistantConfig,
    dictionary,
    enableMuteOtherApps,
    closeAction,
    dualHotkeyConfig,
  ]);

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };
  const isRecording = status === "recording";
  const isTranscribing = status === "transcribing";
  const isConfigLocked = isRecording || isTranscribing;

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
            asrCache={asrCache}
            setAsrCache={setAsrCache}
            asrConfig={asrConfig}
            setAsrConfig={setAsrConfig}
            showApiKey={showApiKey}
            setShowApiKey={setShowApiKey}
            isRunning={isConfigLocked}
          />
        );
      case "llm":
        return (
          <LlmPage
            llmConfig={llmConfig}
            setLlmConfig={setLlmConfig}
            activePreset={activePreset}
            defaultPresets={DEFAULT_PRESETS}
            handleAddPreset={handleAddPreset}
            handleDeletePreset={handleDeletePreset}
            handleUpdateActivePreset={handleUpdateActivePreset}
            showApiKey={showApiKey}
            setShowApiKey={setShowApiKey}
            isRunning={isConfigLocked}
          />
        );
      case "assistant":
        return (
          <AssistantPage
            assistantConfig={assistantConfig}
            setAssistantConfig={setAssistantConfig}
            showApiKey={showApiKey}
            setShowApiKey={setShowApiKey}
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
          />
        );
      default:
        return null;
    }
  })();

  return (
    <div className="h-screen w-full bg-[var(--paper)] text-[var(--ink)] font-serif flex">
      <Sidebar
        collapsed={sidebarCollapsed}
        onToggleCollapsed={() => setSidebarCollapsed((v) => !v)}
        activePage={activePage}
        onNavigate={navigate}
      />

      <div className="flex-1 min-w-0 flex flex-col h-screen overflow-hidden">
        <TopStatusBar
          status={status}
          updateStatus={updateStatus}
          recordingTime={recordingTime}
          formatTime={formatTime}
          usageStats={usageStats}
          onOpenDictionary={() => navigate("dictionary")}
          onOpenSettings={() => navigate("preferences")}
          onOpenHistory={() => navigate("history")}
          onCancelTranscription={handleCancelTranscription}
          onStartService={() => {
            void handleStartStop();
          }}
          startServiceDisabled={isRecording || isTranscribing}
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
              asrCache={asrCache}
              setAsrCache={setAsrCache}
              asrConfig={asrConfig}
              setAsrConfig={setAsrConfig}
              useRealtime={useRealtime}
              setUseRealtime={setUseRealtime}
              enablePostProcess={enablePostProcess}
              setEnablePostProcess={setEnablePostProcess}
              llmConfig={llmConfig}
              setLlmConfig={setLlmConfig}
              dualHotkeyConfig={dualHotkeyConfig}
              dictionary={dictionary}
              newWord={newWord}
              setNewWord={setNewWord}
              onAddWord={handleAddWord}
              isRunning={isConfigLocked}
              onNavigate={navigate}
            />
          )}
        </div>
      </div>

      <div
        className={`fixed top-6 left-1/2 -translate-x-1/2 pointer-events-none transition-all duration-500 z-50 ${
          showSuccessToast ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-4"
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

      {/* Global Toast */}
      {copyToast && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 z-[100] bg-slate-900 text-white px-4 py-2 rounded-full text-sm font-medium shadow-lg animate-in fade-in zoom-in duration-200">
          {copyToast}
        </div>
      )}
    </div>
  );
}
export default App;
