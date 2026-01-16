import { Download, RefreshCw, SlidersHorizontal, VolumeX } from "lucide-react";
import type { UpdateStatus } from "../types";
import { Toggle } from "../components/common";

export type PreferencesPageProps = {
  status: "idle" | "running" | "recording" | "transcribing";

  enableAutostart: boolean;
  onToggleAutostart: () => void;

  enableMuteOtherApps: boolean;
  setEnableMuteOtherApps: (next: boolean) => void;

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
          <div>
            <div className="text-sm font-bold text-[var(--ink)]">开机自启动</div>
            <div className="text-[11px] text-stone-400 font-semibold">系统启动后自动运行</div>
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
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
