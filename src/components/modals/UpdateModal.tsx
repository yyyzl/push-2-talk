import { Download, RefreshCw, X } from "lucide-react";

export type UpdateInfo = { version: string; notes?: string };
export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "ready";

export type UpdateModalProps = {
  open: boolean;
  updateInfo: UpdateInfo | null;
  updateStatus: UpdateStatus;
  downloadProgress: number;
  onDismiss: () => void;
  onDownloadAndInstall: () => void;
  onOpenChangelog: () => void;
};

export function UpdateModal({
  open,
  updateInfo,
  updateStatus,
  downloadProgress,
  onDismiss,
  onDownloadAndInstall,
  onOpenChangelog,
}: UpdateModalProps) {
  if (!open || !updateInfo) return null;

  const isDownloading = updateStatus === "downloading";

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
      <div className="bg-white rounded-2xl shadow-2xl w-full max-w-md mx-4 overflow-hidden animate-in zoom-in-95 duration-200">
        <div className="px-6 py-4 border-b border-slate-100 bg-gradient-to-r from-green-50 to-emerald-50">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-green-100 rounded-xl text-green-600">
                <Download size={20} />
              </div>
              <div>
                <h3 className="text-lg font-bold text-slate-900">发现新版本</h3>
                <p className="text-xs text-slate-500">v{updateInfo.version}</p>
              </div>
            </div>
            <button
              onClick={onDismiss}
              disabled={isDownloading}
              className="p-2 hover:bg-slate-200 rounded-lg text-slate-500 hover:text-slate-700 transition-colors disabled:opacity-50"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        <div className="p-6 space-y-4">
          {updateInfo.notes && (
            <div className="p-4 bg-slate-50 rounded-xl border border-slate-100">
              <h4 className="text-sm font-medium text-slate-700 mb-2">更新内容</h4>
              <p className="text-sm text-slate-600 whitespace-pre-wrap">{updateInfo.notes}</p>
            </div>
          )}

          <p className="text-xs text-slate-400">
            查看完整更新日志：
            <button
              onClick={onOpenChangelog}
              className="text-blue-500 hover:text-blue-600 hover:underline ml-1"
            >
              更新日志
            </button>
          </p>

          {isDownloading && (
            <div className="space-y-2">
              <div className="flex justify-between text-sm text-slate-600">
                <span>正在下载更新...</span>
                <span>{downloadProgress}%</span>
              </div>
              <div className="w-full h-2 bg-slate-200 rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500 transition-all duration-300"
                  style={{ width: `${downloadProgress}%` }}
                />
              </div>
            </div>
          )}

          <div className="flex gap-3">
            <button
              onClick={onDismiss}
              disabled={isDownloading}
              className="flex-1 px-4 py-2.5 text-sm font-medium text-slate-600 hover:text-slate-800 bg-slate-100 hover:bg-slate-200 rounded-xl transition-colors disabled:opacity-50"
            >
              稍后更新
            </button>
            <button
              onClick={onDownloadAndInstall}
              disabled={isDownloading}
              className="flex-1 px-4 py-2.5 text-sm font-medium text-white bg-green-500 hover:bg-green-600 rounded-xl shadow-lg shadow-green-500/20 transition-all disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {isDownloading ? (
                <>
                  <RefreshCw size={16} className="animate-spin" />
                  下载中...
                </>
              ) : (
                <>
                  <Download size={16} />
                  立即更新
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

