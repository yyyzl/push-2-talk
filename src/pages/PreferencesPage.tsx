import { Download, Power, RefreshCw, SlidersHorizontal, VolumeX, GraduationCap, Settings2, HelpCircle } from "lucide-react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppStatus, UpdateStatus, LearningConfig, SharedLlmConfig } from "../types";
import { Toggle, ThemeSelector, LlmConnectionConfig, Tooltip } from "../components/common";
import { RedDot } from "../components/common/RedDot";
import { SettingsModal } from "../components/modals/SettingsModal";

export type PreferencesPageProps = {
  status: AppStatus;

  enableAutostart: boolean;
  onToggleAutostart: () => void;

  enableMuteOtherApps: boolean;
  setEnableMuteOtherApps: (next: boolean) => void;

  theme: string;
  setTheme: (theme: string) => Promise<void>;

  updateStatus: UpdateStatus;
  updateInfo: { version: string; notes?: string } | null;
  currentVersion: string;
  onCheckUpdate: () => void;
  onDownloadAndInstall: () => void;

  sharedConfig: SharedLlmConfig;
  onNavigateToModels?: () => void;
};

export function PreferencesPage({
  status,
  enableAutostart,
  onToggleAutostart,
  enableMuteOtherApps,
  setEnableMuteOtherApps,
  theme,
  setTheme,
  updateStatus,
  updateInfo,
  currentVersion,
  onCheckUpdate,
  onDownloadAndInstall,
  sharedConfig,
  onNavigateToModels,
}: PreferencesPageProps) {
  const canInstallUpdate = updateStatus === "available" || updateStatus === "downloading";

  // 自动学习配置状态
  const [learningEnabled, setLearningEnabled] = useState(false);
  const [_learningConfig, setLearningConfig] = useState<LearningConfig | null>(null);
  const [isLoadingLearning, setIsLoadingLearning] = useState(true);
  const [learningConfigModalOpen, setLearningConfigModalOpen] = useState(false);

  // 加载自动学习配置
  useEffect(() => {
    const loadLearningConfig = async () => {
      try {
        const config = await invoke<{ learning_config: LearningConfig }>("load_config");
        const lc = config.learning_config;
        setLearningEnabled(lc?.enabled ?? false);
        setLearningConfig(lc);
      } catch (error) {
        console.error("加载自动学习配置失败:", error);
      } finally {
        setIsLoadingLearning(false);
      }
    };
    loadLearningConfig();
  }, []);

  // 切换自动学习开关
  const handleToggleLearning = async () => {
    const newValue = !learningEnabled;
    setLearningEnabled(newValue);

    try {
      const config = await invoke<any>("load_config");
      const updatedLearningConfig = {
        ...config.learning_config,
        enabled: newValue,
      };

      await invoke("save_config", {
        apiKey: config.dashscope_api_key || config.asr_config?.credentials?.qwen_api_key || "",
        fallbackApiKey: config.siliconflow_api_key || config.asr_config?.credentials?.sensevoice_api_key || "",
        learningConfig: updatedLearningConfig,
      });

      setLearningConfig(updatedLearningConfig);
    } catch (error) {
      console.error("保存自动学习配置失败:", error);
      setLearningEnabled(!newValue); // 回滚
    }
  };

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <SlidersHorizontal size={14} />
          <span>偏好设置</span>
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
                {enableMuteOtherApps ? "录音期间自动静音" : "不干预音频"}
              </div>
            </div>
          </div>
          <Toggle
            checked={enableMuteOtherApps}
            onCheckedChange={setEnableMuteOtherApps}
            disabled={status === "recording" || status === "transcribing"}
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
              disabled={isLoadingLearning || status === "recording" || status === "transcribing"}
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
                : updateStatus === "checking"
                  ? "正在连接服务器..."
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
                <Download size={14} />
                {updateStatus === "downloading" ? "下载中..." : "更新"}
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
