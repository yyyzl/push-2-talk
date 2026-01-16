import { Minus, X, XCircle } from "lucide-react";

export type CloseConfirmDialogProps = {
  open: boolean;
  rememberChoice: boolean;
  onRememberChoiceChange: (next: boolean) => void;
  onDismiss: () => void;
  onResetRememberChoice: () => void;
  onCloseApp: () => void;
  onMinimizeToTray: () => void;
};

export function CloseConfirmDialog({
  open,
  rememberChoice,
  onRememberChoiceChange,
  onDismiss,
  onResetRememberChoice,
  onCloseApp,
  onMinimizeToTray,
}: CloseConfirmDialogProps) {
  if (!open) return null;

  return (

            <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
              <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-3xl shadow-2xl w-full max-w-md mx-4 overflow-hidden animate-in zoom-in-95 duration-200 font-sans">
                {/* Dialog Header */}
                <div className="px-6 py-4 border-b border-[var(--stone)] bg-[var(--paper)]">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-lg font-bold text-[var(--ink)]">关闭应用</h3>
                      <p className="text-xs text-[var(--stone-dark)]">选择关闭方式</p>
                    </div>
                    <button
                      onClick={() => {
                        onDismiss();
                        onResetRememberChoice();
                      }}
                      className="p-2 hover:bg-[var(--panel)] rounded-xl text-[var(--stone-dark)] hover:text-[var(--ink)] transition-colors"
                    >
                      <X size={18} />
                    </button>
                  </div>
                </div>
                {/* Dialog Body */}
                <div className="p-6 space-y-4">
                  <p className="text-sm text-stone-600">您希望如何处理应用窗口？</p>
                  <div className="space-y-3">
                    <button
                      onClick={onMinimizeToTray}
                      className="w-full p-4 bg-[rgba(106,155,204,0.10)] hover:bg-[rgba(106,155,204,0.14)] border border-[rgba(106,155,204,0.22)] rounded-2xl text-left transition-colors group"
                    >
                      <div className="flex items-center gap-3">
                        <div className="p-2 bg-[rgba(106,155,204,0.14)] rounded-xl text-[var(--steel)] transition-colors">
                          <Minus size={18} />
                        </div>
                        <div>
                          <div className="text-sm font-bold text-[var(--ink)]">最小化到系统托盘</div>
                          <div className="text-xs text-[var(--stone-dark)]">应用将在后台继续运行</div>
                        </div>
                      </div>
                    </button>
                    <button
                      onClick={onCloseApp}
                      className="w-full p-4 bg-[var(--panel)] hover:bg-[rgba(232,230,220,0.85)] border border-[var(--stone)] rounded-2xl text-left transition-colors group"
                    >
                      <div className="flex items-center gap-3">
                        <div className="p-2 bg-[var(--paper)] border border-[var(--stone)] rounded-xl text-[var(--stone-dark)] transition-colors">
                          <XCircle size={18} />
                        </div>
                        <div>
                          <div className="text-sm font-bold text-[var(--ink)]">完全退出</div>
                          <div className="text-xs text-[var(--stone-dark)]">关闭应用并停止所有服务</div>
                        </div>
                      </div>
                    </button>
                  </div>
                  <label className="flex items-center gap-3 p-3 bg-[var(--panel)] rounded-2xl cursor-pointer hover:bg-[rgba(232,230,220,0.85)] transition-colors">
                    <input
                      type="checkbox"
                      checked={rememberChoice}
                      onChange={(e) => onRememberChoiceChange(e.target.checked)}
                      className="w-4 h-4 rounded border-[var(--stone)] text-[var(--crail)] focus:ring-[rgba(106,155,204,0.20)]"
                    />
                    <span className="text-sm text-stone-600">记住我的选择，下次不再询问</span>
                  </label>
                </div>
              </div>
            </div>

  );
}
