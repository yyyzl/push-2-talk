import type { PermissionStatus } from "../../types";

type PermissionGuideModalProps = {
  open: boolean;
  status: PermissionStatus | null;
  onDismiss: () => void;
  onRefresh: () => void;
};

type PermissionItem = {
  key: keyof PermissionStatus;
  name: string;
  path: string;
};

const PERMISSION_ITEMS: PermissionItem[] = [
  {
    key: "microphone",
    name: "麦克风",
    path: "系统设置 -> 隐私与安全性 -> 麦克风",
  },
  {
    key: "input_monitoring",
    name: "输入监控",
    path: "系统设置 -> 隐私与安全性 -> 输入监控",
  },
  {
    key: "accessibility",
    name: "辅助功能",
    path: "系统设置 -> 隐私与安全性 -> 辅助功能",
  },
];

export function PermissionGuideModal({
  open,
  status,
  onDismiss,
  onRefresh,
}: PermissionGuideModalProps) {
  if (!open || !status) return null;

  const pendingItems = PERMISSION_ITEMS.filter((item) => !status[item.key]);
  if (pendingItems.length === 0) return null;

  return (
    <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
      <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-3xl shadow-2xl w-full max-w-xl mx-4 overflow-hidden animate-in zoom-in-95 duration-200 font-sans">
        <div className="px-6 py-4 border-b border-[var(--stone)]">
          <h3 className="text-lg font-bold text-[var(--ink)]">macOS 权限引导</h3>
          <p className="text-xs text-[var(--stone-dark)]">
            以下权限未授权时，热键、录音或文本注入会失效。
          </p>
        </div>

        <div className="p-6 space-y-3">
          {pendingItems.map((item) => (
            <div
              key={item.key}
              className="p-3 border border-[var(--stone)] rounded-2xl bg-white"
            >
              <div className="text-sm font-bold text-[var(--ink)]">{item.name}</div>
              <div className="text-xs text-stone-500 mt-1">{item.path}</div>
            </div>
          ))}
        </div>

        <div className="px-6 pb-6 flex items-center gap-3">
          <button
            onClick={onDismiss}
            className="flex-1 px-4 py-2.5 text-sm font-bold text-[var(--stone-dark)] bg-[var(--panel)] hover:bg-[rgba(232,230,220,0.85)] border border-[var(--stone)] rounded-2xl transition-colors"
          >
            稍后处理
          </button>
          <button
            onClick={onRefresh}
            className="flex-1 px-4 py-2.5 text-sm font-bold text-white bg-[var(--sage)] hover:opacity-90 rounded-2xl transition-all"
          >
            已授权，重新检测
          </button>
        </div>
      </div>
    </div>
  );
}
