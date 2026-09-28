import { Download, Power, RefreshCw, SlidersHorizontal, VolumeX, GraduationCap, Settings2, HelpCircle, Sparkles } from "lucide-react";
import { useState } from "react";
import { usePlatformStatus } from "../hooks/usePlatformStatus";
import { PlatformPermissions } from "../components/common/PlatformPermissions";
import type { AppStatus, DisfluencyMode, UpdateStatus, LearningConfig, SharedLlmConfig, TnlConfig } from "../types";
import { Toggle, ThemeSelector, LlmConnectionConfig, Tooltip } from "../components/common";
import { RedDot } from "../components/common/RedDot";
import { SettingsModal } from "../components/modals/SettingsModal";
import { normalizeLearningConfig } from "../constants";

export type PreferencesPageProps = {
  status: AppStatus;
  onStartService: () => Promise<void>;

  enableAutostart: boolean;
  onToggleAutostart: () => void;

  enableMuteOtherApps: boolean;
  onSetEnableMuteOtherApps: (next: boolean) => Promise<void>;

  theme: string;
  setTheme: (theme: string) => Promise<void>;

  updateStatus: UpdateStatus;
  updateInfo: { version: string; notes?: string } | null;
  currentVersion: string;
  onCheckUpdate: () => void;
  onDownloadAndInstall: () => void;

  sharedConfig: SharedLlmConfig;
  learningConfig: LearningConfig;
  tnlConfig: TnlConfig;
  setLearningConfig: (next: LearningConfig) => void;
  onSetLearningEnabled: (enabled: boolean) => Promise<void>;
  onSetDisfluencyMode: (mode: DisfluencyMode) => Promise<void>;
  onSetContextHotwords: (enabled: boolean) => Promise<void>;
  onNavigateToModels?: () => void;
};

const DISFLUENCY_MODE_OPTIONS: Array<{
  value: DisfluencyMode;
  label: string;
  summary: string;
}> = [
  { value: "off", label: "关闭", summary: "保留原始口语" },
  { value: "conservative", label: "保守", summary: "仅清理句首短停顿词，保留内容词和重复字" },
];

export function PreferencesPage({
  status,
  onStartService,
  enableAutostart,
  onToggleAutostart,
  enableMuteOtherApps,
  onSetEnableMuteOtherApps,
  theme,
  setTheme,
  updateStatus,
  updateInfo,
  currentVersion,
  onCheckUpdate,
  onDownloadAndInstall,
  sharedConfig,
  learningConfig,
  tnlConfig,
  setLearningConfig,
  onSetLearningEnabled,
  onSetDisfluencyMode,
  onSetContextHotwords,
  onNavigateToModels,
}: PreferencesPageProps) {
  const platformState = usePlatformStatus();
  const [startingService, setStartingService] = useState(false);
  const [savingContext, setSavingContext] = useState(false);
  const [contextError, setContextError] = useState<string | null>(null);
  const startService = async () => {
    setStartingService(true);
    try {
      await onStartService();
    } finally {
      setStartingService(false);
    }
  };
  const muteSupported = platformState.platform?.other_app_mute === true;
  const canInstallUpdate = updateStatus === "available" || updateStatus === "downloading";

  // 自动学习配置状态
  const learningEnabled = learningConfig.enabled;
  const disfluencyMode = tnlConfig.disfluency_mode === "aggressive" ? "conservative" : tnlConfig.disfluency_mode;
  const disfluencySummary =
    DISFLUENCY_MODE_OPTIONS.find((option) => option.value === disfluencyMode)?.summary
    ?? DISFLUENCY_MODE_OPTIONS[1].summary;
  const [learningConfigModalOpen, setLearningConfigModalOpen] = useState(false);

  // 切换自动学习开关
  const handleToggleLearning = async () => {
    const newValue = !learningEnabled;
    const previousLearningConfig = learningConfig;
    const updatedLearningConfig = normalizeLearningConfig({
      ...learningConfig,
      enabled: newValue,
    });
    setLearningConfig(updatedLearningConfig);

    try {
      await onSetLearningEnabled(newValue);
    } catch (error) {
      console.error("保存自动学习配置失败:", error);
      setLearningConfig(previousLearningConfig); // 回滚
    }
  };

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <SlidersHorizontal size={14} />
          <span>偏好设置</span>
        </div>

        <PlatformPermissions {...platformState} onRequest={platformState.request} onRefresh={platformState.refresh}
          serviceIdle={status === "idle"} startingService={startingService} onStartService={startService} />
        <div className="flex items-center justify-between gap-4 p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                disfluencyMode !== "off"
                  ? "bg-[rgba(120,140,93,0.12)] text-[var(--sage)]"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <Sparkles size={16} />
            </div>
            <div>
              <div className="flex items-center gap-1.5">
                <div className="text-sm font-bold text-[var(--ink)]">口语流畅化</div>
                <Tooltip content="保守模式只清理带停顿的句首“嗯、呃”，保留内容词和重复字；旧强力设置也按此规则处理。">
                  <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
                </Tooltip>
              </div>
              <div className="text-[11px] text-stone-400 font-semibold">{disfluencySummary}</div>
            </div>
          </div>

          <div className="grid grid-cols-2 overflow-hidden rounded-xl border border-[var(--stone)] bg-white">
            {DISFLUENCY_MODE_OPTIONS.map((option) => {
              const selected = disfluencyMode === option.value;
              return (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => {
                    if (selected) return;
                    void onSetDisfluencyMode(option.value);
                  }}
                  disabled={status === "recording" || status === "transcribing"}
                  className={[
                    "h-9 min-w-[3.5rem] px-3 text-xs font-bold transition-colors",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                    selected
                      ? "bg-[var(--ink)] text-white"
                      : "text-stone-500 hover:bg-[var(--paper)] hover:text-[var(--ink)]",
                  ].join(" ")}
                >
                  {option.label}
                </button>
              );
            })}
          </div>
        </div>

        <div className="space-y-2 border-t border-[var(--stone)] pt-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <div id="context-hotword-label" className="text-sm font-semibold text-[var(--ink)]">上下文热词（实验性）</div>
              <p id="context-hotword-hint" className="mt-1 text-xs text-stone-600 leading-relaxed">从当前输入窗口和近 24 小时历史提取技术词，随录音作为热词发送给所选识别服务。默认关闭。</p>
              <p className="mt-1 text-xs text-stone-600">停止服务后调整，下次启动生效。</p>
            </div>
            <Toggle
              aria-labelledby="context-hotword-label"
              aria-describedby="context-hotword-hint"
              checked={tnlConfig.enable_context_hotwords}
              disabled={status !== "idle" || savingContext}
              size="sm"
              className="mt-1 shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--steel)]"
              onCheckedChange={async (enabled) => {
                setSavingContext(true);
                setContextError(null);
                try {
                  await onSetContextHotwords(enabled);
                } catch {
                  setContextError("保存失败，已恢复原设置，请重试。");
                } finally {
                  setSavingContext(false);
                }
              }}
            />
          </div>
          {contextError && <p className="text-sm text-red-700" role="alert">{contextError}</p>}
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                enableAutostart
                  ? "bg-[rgba(34,197,94,0.12)] text-green-500"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <Power size={16} />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--ink)]">开机自启动</div>
              <div className="text-[11px] text-stone-400 font-semibold">系统启动后自动运行</div>
            </div>
          </div>
          <Toggle checked={enableAutostart} onCheckedChange={() => onToggleAutostart()} size="sm" variant="green" />
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                enableMuteOtherApps
                  ? "bg-[rgba(217,119,87,0.12)] text-[var(--crail)]"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <VolumeX size={16} />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--ink)]">录音时静音其他应用</div>
              <div className="text-[11px] text-stone-400 font-semibold">
                {!muteSupported ? "当前平台暂不支持此功能" : enableMuteOtherApps ? "录音期间自动静音" : "不干预音频"}
              </div>
            </div>
          </div>
          <Toggle
            checked={muteSupported && enableMuteOtherApps}
            onCheckedChange={(next) => {
              void onSetEnableMuteOtherApps(next);
            }}
            disabled={!muteSupported || status === "recording" || status === "transcribing"}
            size="sm"
            variant="orange"
          />
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                learningEnabled
                  ? "bg-[rgba(120,140,93,0.12)] text-[var(--sage)]"
                  : "bg-white border border-[var(--stone)] text-stone-500",
              ].join(" ")}
            >
              <GraduationCap size={16} />
            </div>
            <div>
              <div className="flex items-center gap-1.5">
                <div className="text-sm font-bold text-[var(--ink)]">自动词库学习</div>
                <Tooltip content="AI 自动识别语音中的专业术语、人名和地名，学习后会自动添加到个人词库中，提高后续识别准确率。">
                  <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
                </Tooltip>
              </div>
              <div className="text-[11px] text-stone-400 font-semibold">
                {learningEnabled ? "AI 自动识别专业术语" : "手动管理词库"}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-3">
            {learningEnabled && (
              <button
                onClick={() => setLearningConfigModalOpen(true)}
                className="p-2 rounded-xl text-stone-400 hover:bg-white hover:text-[var(--ink)] hover:shadow-sm border border-transparent hover:border-[var(--stone)] transition-all"
                title="配置自动学习"
              >
                <Settings2 size={18} />
              </button>
            )}
            <div className="h-6 w-px bg-[var(--stone)] mx-1" />
            <Toggle
              checked={learningEnabled}
              onCheckedChange={handleToggleLearning}
              disabled={status === "recording" || status === "transcribing"}
              size="sm"
              variant="green"
            />
          </div>
        </div>

        <SettingsModal
          open={learningConfigModalOpen}
          onDismiss={() => setLearningConfigModalOpen(false)}
          title="自动词库学习配置"
        >
          <div className="space-y-4">
            <div className="p-4 bg-[rgba(120,140,93,0.08)] border border-[rgba(120,140,93,0.15)] rounded-2xl">
              <p className="text-sm text-[var(--ink)] leading-relaxed">
                开启此功能后，AI 将自动分析您的语音输入，识别并提取专业术语、人名和地名，自动添加到您的个人词库中，提高后续识别的准确率。
              </p>
            </div>

            <div className="space-y-2">
              <h4 className="text-xs font-bold text-stone-500 uppercase tracking-widest">LLM 连接配置</h4>
              <LlmConnectionConfig
                sharedConfig={sharedConfig}
                featureName="learning"
                onNavigateToModels={() => {
                  setLearningConfigModalOpen(false);
                  onNavigateToModels?.();
                }}
              />
            </div>
          </div>
        </SettingsModal>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div className="flex items-center gap-3">
            <div
              className={[
                "p-2 rounded-xl",
                theme === "light"
                  ? "bg-[rgba(217,119,87,0.12)] text-[var(--crail)]"
                  : "bg-stone-800 text-stone-200",
              ].join(" ")}
            >
              <div className="w-4 h-4 rounded-full border-2 border-current" />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--ink)]">悬浮窗风格</div>
              <div className="text-[11px] text-stone-400 font-semibold">
                选择录音指示器外观
              </div>
            </div>
          </div>
          <ThemeSelector
            value={theme}
            onChange={(newTheme) => {
              console.log("[PreferencesPage] 切换主题:", newTheme);
              setTheme(newTheme);
            }}
            disabled={status === "recording" || status === "transcribing"}
          />
        </div>

        <div className="flex items-center justify-between p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl">
          <div>
            <div className="text-sm font-bold text-[var(--ink)]">检查更新</div>
            <div className="text-[11px] text-stone-400 font-semibold">
              {updateStatus === "available" && updateInfo
                ? `发现新版本 v${updateInfo.version}`
                : `当前版本 v${currentVersion}`}
            </div>
          </div>
          <div className="flex items-center gap-2">
            {canInstallUpdate && (
              <button
                onClick={onDownloadAndInstall}
                disabled={updateStatus === "downloading"}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                {updateStatus === "downloading" ? <RefreshCw size={14} className="animate-spin" /> : <Download size={14} />}
                更新
              </button>
            )}
            <button
              onClick={onCheckUpdate}
              disabled={updateStatus === "checking" || updateStatus === "downloading"}
              className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
            >
              {updateStatus === "checking" ? <RefreshCw size={14} className="animate-spin" /> : <RefreshCw size={14} />}
              检查
              {updateStatus === "available" && <RedDot size="md" />}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
