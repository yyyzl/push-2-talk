import { Download, Power, RefreshCw, SlidersHorizontal, VolumeX } from "lucide-react";
import type { AppStatus, UpdateStatus } from "../types";
import { Toggle } from "../components/common";
import { RedDot } from "../components/common/RedDot";

export type PreferencesPageProps = {
  status: AppStatus;

  enableAutostart: boolean;
  onToggleAutostart: () => void;

  enableMuteOtherApps: boolean;
  setEnableMuteOtherApps: (next: boolean) => void;

  theme: string;
  setTheme: (theme: string) => void;

  updateStatus: UpdateStatus;
  updateInfo: { version: string; notes?: string } | null;
  currentVersion: string;
  onCheckUpdate: () => void;
  onDownloadAndInstall: () => void;
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
}: PreferencesPageProps) {
  const canInstallUpdate = updateStatus === "available" || updateStatus === "downloading";

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
                {theme === "light" ? "红朱 (Light)" : "墨色 (Dark)"}
              </div>
            </div>
          </div>
          <Toggle
            checked={theme === "light"}
            onCheckedChange={(checked) => setTheme(checked ? "light" : "dark")}
            size="sm"
            variant="orange"
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
