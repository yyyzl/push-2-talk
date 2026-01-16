import type { Dispatch, SetStateAction } from "react";
import { AlertCircle, MessageSquareQuote, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { LlmConfig, LlmPreset } from "../types";
import { ApiKeyInput } from "../components/common";

export type LlmPageProps = {
  llmConfig: LlmConfig;
  setLlmConfig: Dispatch<SetStateAction<LlmConfig>>;
  activePreset: LlmPreset;
  defaultPresets: LlmPreset[];
  handleAddPreset: () => void;
  handleDeletePreset: (id: string) => void;
  handleUpdateActivePreset: (key: keyof LlmPreset, value: string) => void;

  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;
  isRunning: boolean;
};

export function LlmPage({
  llmConfig,
  setLlmConfig,
  activePreset,
  defaultPresets,
  handleAddPreset,
  handleDeletePreset,
  handleUpdateActivePreset,
  showApiKey,
  setShowApiKey,
  isRunning,
}: LlmPageProps) {
  return (
    <div className="mx-auto max-w-5xl font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl overflow-hidden">
        <div className="grid grid-cols-12 min-h-[560px]">
          <aside className="col-span-4 bg-[var(--paper)] border-r border-[var(--stone)] flex flex-col">
            <div className="p-5 border-b border-[var(--stone)]">
              <div className="flex items-center gap-2 p-3 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-xs text-[var(--ink)]">
                <AlertCircle size={14} className="text-[var(--steel)]" />
                <span>Ctrl+Win 听写时使用</span>
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
                    onClick={() => setLlmConfig((prev) => ({ ...prev, active_preset_id: preset.id }))}
                    className={[
                      "group flex items-center justify-between p-3 rounded-2xl cursor-pointer transition-colors",
                      active ? "bg-white border border-[var(--stone)] shadow-sm" : "hover:bg-white/60",
                    ].join(" ")}
                  >
                    <div className="flex items-center gap-3 min-w-0">
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
                      <span className={["text-sm font-bold truncate", active ? "text-[var(--ink)]" : "text-stone-700"].join(" ")}>
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
                <label className="text-xs font-bold text-stone-500 uppercase tracking-widest">预设名称</label>
                <input
                  type="text"
                  value={activePreset?.name || ""}
                  disabled={isRunning}
                  onChange={(e) => handleUpdateActivePreset("name", e.target.value)}
                  className="w-full px-4 py-3 bg-white border border-[var(--stone)] rounded-2xl text-sm font-semibold focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-bold text-stone-500 uppercase tracking-widest">System Prompt</label>
                  <button
                    onClick={() => {
                      const original = defaultPresets.find((p) => p.id === activePreset.id);
                      if (original) handleUpdateActivePreset("system_prompt", original.system_prompt);
                    }}
                    disabled={isRunning}
                    className="text-xs font-bold text-[var(--steel)] hover:opacity-80 transition-opacity disabled:opacity-50"
                  >
                    <span className="inline-flex items-center gap-1">
                      <RotateCcw size={12} /> 恢复默认
                    </span>
                  </button>
                </div>
                <textarea
                  value={activePreset?.system_prompt || ""}
                  disabled={isRunning}
                  onChange={(e) => handleUpdateActivePreset("system_prompt", e.target.value)}
                  className="w-full min-h-[220px] p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl text-sm focus:outline-none focus:border-[var(--steel)] resize-none mono text-stone-700 leading-relaxed disabled:opacity-60"
                  placeholder="在这里定义 AI 的行为..."
                />
              </div>

              <div className="h-px bg-[var(--stone)]" />

              <div className="space-y-4">
                <h4 className="text-xs font-bold text-stone-400 uppercase tracking-widest">模型设置</h4>
                <div className="grid grid-cols-2 gap-4">
                  <div className="col-span-2 space-y-2">
                    <label className="text-xs font-bold text-stone-500">API Key</label>
                    <ApiKeyInput
                      value={llmConfig.api_key}
                      onChange={(value) => setLlmConfig({ ...llmConfig, api_key: value })}
                      show={showApiKey}
                      onToggleShow={() => setShowApiKey(!showApiKey)}
                      placeholder="sk-..."
                      inputClassName="bg-[var(--paper)] text-sm focus:ring-0 focus:border-[var(--steel)]"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-xs font-bold text-stone-500">模型名称</label>
                    <input
                      type="text"
                      value={llmConfig.model}
                      disabled={isRunning}
                      onChange={(e) => setLlmConfig({ ...llmConfig, model: e.target.value })}
                      className="w-full px-3 py-2.5 bg-[var(--paper)] border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
                      placeholder="glm-4-flash"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-xs font-bold text-stone-500">API 地址</label>
                    <input
                      type="text"
                      value={llmConfig.endpoint}
                      disabled={isRunning}
                      onChange={(e) => setLlmConfig({ ...llmConfig, endpoint: e.target.value })}
                      className="w-full px-3 py-2.5 bg-[var(--paper)] border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
                      placeholder="https://api..."
                    />
                  </div>
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
