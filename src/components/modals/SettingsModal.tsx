import { X, Sparkles } from "lucide-react";
import { ReactNode } from "react";

export type SettingsModalProps = {
    open: boolean;
    onDismiss: () => void;
    title: string;
    children: ReactNode;
};

export function SettingsModal({
    open,
    onDismiss,
    title,
    children,
}: SettingsModalProps) {
    if (!open) return null;

    return (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
            <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-3xl shadow-2xl w-full max-w-2xl mx-4 overflow-hidden animate-in zoom-in-95 duration-200 font-sans flex flex-col max-h-[85vh]">
                {/* Header */}
                <div className="px-6 py-4 border-b border-[var(--stone)] bg-[rgba(120,140,93,0.08)] shrink-0">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-[rgba(120,140,93,0.14)] rounded-xl text-[var(--sage)]">
                                <Sparkles size={20} />
                            </div>
                            <h3 className="text-lg font-bold text-[var(--ink)]">{title}</h3>
                        </div>
                        <button
                            onClick={onDismiss}
                            className="p-2 hover:bg-[var(--panel)] rounded-xl text-[var(--stone-dark)] hover:text-[var(--ink)] transition-colors"
                        >
                            <X size={18} />
                        </button>
                    </div>
                </div>

                {/* Body */}
                <div className="p-6 overflow-y-auto custom-scroll">
                    {children}
                </div>
            </div>
        </div>
    );
}
