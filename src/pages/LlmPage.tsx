import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { AlertCircle, MessageSquareQuote, Plus, Trash2 } from "lucide-react";
import type { LlmConfig, LlmPreset } from "../types";
import { PresetModelSelect } from "../components/llm/PresetModelSelect";

export type LlmPageProps = {
  llmConfig: LlmConfig;
  setLlmConfig: Dispatch<SetStateAction<LlmConfig>>;
  activePreset: LlmPreset;
  handleAddPreset: () => void;
  handleDeletePreset: (id: string) => void;
  handleUpdateActivePreset: (key: keyof LlmPreset, value: string) => void;
  onNavigateToModels?: () => void;
  /** R8.2 (v4 simplified): scroll to this preset and select it as active */
  pendingFocus?: { presetId: string } | null;
  /** R8.2: must be called once consumed so the parent can clear pending state */
  onFocusConsumed?: () => void;
  isRunning: boolean;
};

export function LlmPage({
  llmConfig,
  setLlmConfig,
  activePreset,
  handleAddPreset,
  handleDeletePreset,
  handleUpdateActivePreset,
  onNavigateToModels,
  pendingFocus,
  onFocusConsumed,
  isRunning,
}: LlmPageProps) {
  const presetRowRefs = useRef<Record<string, HTMLDivElement | null>>({});
  // 跳转后临时高亮目标 preset row（视觉提示位置，不切 active 也不改配置）
  const [highlightedPresetId, setHighlightedPresetId] = useState<string | null>(null);

  // R8.2 (v4.1): consume pendingFocus — scroll + visual highlight only
  // 不切 active_preset_id（切了会触发配置保存提示，对"跳转浏览"语义错位）
  useEffect(() => {
    if (!pendingFocus) return;
    const node = presetRowRefs.current[pendingFocus.presetId];
    if (node) {
      node.scrollIntoView({ behavior: "smooth", block: "center" });
      setHighlightedPresetId(pendingFocus.presetId);
    }
    onFocusConsumed?.();
  }, [pendingFocus, onFocusConsumed]);

  // 高亮自动消失（2 秒后）
  useEffect(() => {
    if (!highlightedPresetId) return;
    const timer = setTimeout(() => setHighlightedPresetId(null), 2000);
    return () => clearTimeout(timer);
  }, [highlightedPresetId]);

  // v4: commit per-preset model directly (no popover, no transactional state)
  const commitPresetModel = (
    presetId: string,
    providerId: string | undefined,
    model: string | undefined,
  ) => {
    setLlmConfig((prev) => ({
      ...prev,
      presets: prev.presets.map((p) =>
        p.id === presetId ? { ...p, provider_id: providerId, model } : p,
      ),
    }));
  };

  return (
    <div className="mx-auto max-w-5xl font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl overflow-hidden">
        <div className="grid grid-cols-12 min-h-[560px]">
          <aside className="col-span-4 bg-[var(--paper)] border-r border-[var(--stone)] flex flex-col">
            <div className="p-5 border-b border-[var(--stone)]">
              <div className="flex items-center gap-2 p-3 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-xs text-[var(--ink)]">
                <AlertCircle size={14} className="text-[var(--steel)]" />
                <span>听写时使用</span>
              </div>
              <button
                onClick={handleAddPreset}
                disabled={isRunning}
                className="w-full mt-4 py-2.5 bg-white border border-[var(--stone)] rounded-xl text-sm text-stone-600 font-bold hover:border-[rgba(176,174,165,0.75)] hover:text-[var(--steel)] transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
              >
                <Plus size={14} /> 新增预设
              </button>
            </div>

            <div className="flex-1 overflow-y-auto custom-scroll p-2 space-y-1">
              {llmConfig.presets.map((preset) => {
                const active = llmConfig.active_preset_id === preset.id;
                return (
                  <div
                    key={preset.id}
                    ref={(node) => {
                      presetRowRefs.current[preset.id] = node;
                    }}
                    onClick={() =>
                      setLlmConfig((prev) => ({ ...prev, active_preset_id: preset.id }))
                    }
                    className={[
                      "group flex items-center justify-between p-3 rounded-2xl cursor-pointer transition-all gap-2",
                      active
                        ? "bg-white border border-[var(--stone)] shadow-sm"
                        : "hover:bg-white/60",
                      highlightedPresetId === preset.id
                        ? "ring-2 ring-[rgba(217,119,87,0.45)] ring-offset-2 ring-offset-[var(--paper)]"
                        : "",
                    ].join(" ")}
                  >
                    <div className="flex items-center gap-3 min-w-0 flex-1">
                      <div
                        className={[
                          "p-2 rounded-xl shrink-0",
                          active
                            ? "bg-[rgba(217,119,87,0.12)] text-[var(--crail)]"
                            : "bg-white border border-[var(--stone)] text-stone-500",
                        ].join(" ")}
                      >
                        <MessageSquareQuote size={14} />
                      </div>
                      <span
                        className={[
                          "text-sm font-bold truncate",
                          active ? "text-[var(--ink)]" : "text-stone-700",
                        ].join(" ")}
                      >
                        {preset.name}
                      </span>
                    </div>

                    {llmConfig.presets.length > 1 && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDeletePreset(preset.id);
                        }}
                        disabled={isRunning}
                        className={[
                          "p-2 rounded-xl text-stone-400 hover:bg-red-50 hover:text-red-600 transition-colors",
                          "opacity-0 group-hover:opacity-100",
                          active ? "opacity-100" : "",
                          isRunning ? "opacity-50 cursor-not-allowed" : "",
                        ].join(" ")}
                        title="删除预设"
                      >
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </aside>

          <section className="col-span-8 flex flex-col">
            <div className="flex-1 overflow-y-auto custom-scroll p-6 space-y-6">
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500 uppercase tracking-widest">
                  预设名称
                </label>
                <input
                  type="text"
                  value={activePreset?.name || ""}
                  disabled={isRunning}
                  onChange={(e) => handleUpdateActivePreset("name", e.target.value)}
                  className="w-full px-4 py-3 bg-white border border-[var(--stone)] rounded-2xl text-sm font-semibold focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
                />
              </div>

              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500 uppercase tracking-widest">
                  System Prompt
                </label>
                <textarea
                  value={activePreset?.system_prompt || ""}
                  disabled={isRunning}
                  onChange={(e) => handleUpdateActivePreset("system_prompt", e.target.value)}
                  className="w-full min-h-[220px] p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl text-sm focus:outline-none focus:border-[var(--steel)] resize-none mono text-stone-700 leading-relaxed disabled:opacity-60"
                  placeholder="在这里定义 AI 的行为..."
                />
              </div>

              <div className="h-px bg-[var(--stone)]" />

              {activePreset && (
                <PresetModelSelect
                  preset={activePreset}
                  llmConfig={llmConfig}
                  disabled={isRunning}
                  onCommit={(providerId, model) =>
                    commitPresetModel(activePreset.id, providerId, model)
                  }
                  onNavigateToModels={onNavigateToModels}
                />
              )}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
