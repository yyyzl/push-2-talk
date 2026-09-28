import { Select } from "../common/Select";
import { AlertTriangle, ExternalLink } from "lucide-react";
import type { LlmConfig, LlmPreset } from "../../types";
import {
  buildModelSelectOptions,
  computeOverrideCommit,
  resolvePresetEffectiveModel,
} from "../../utils/presetOverride";

export type PresetModelSelectProps = {
  preset: LlmPreset;
  llmConfig: LlmConfig;
  disabled?: boolean;
  onCommit: (providerId: string | undefined, model: string | undefined) => void;
  onNavigateToModels?: () => void;
};

/**
 * Inline model selector embedded in the LlmPage preset editor panel.
 *
 * UX (v4):
 * - One dropdown with "跟随润色默认（<provider> · <model>）" as the first option
 *   followed by the full provider list.
 * - Below the dropdown: an optional model ID textbox that appears whenever a
 *   specific provider is selected (always-visible, not collapsed under "advanced").
 * - For dangling references (manual config edits), the deleted provider id is
 *   shown as a disabled "(已删除) <id>" option so the user can see the broken
 *   state and pick a replacement explicitly.
 *
 * No transactional popover — edits commit immediately to global llmConfig
 * via `onCommit`. The whole "preset override" mental model is collapsed into
 * a regular form field; the user just edits the preset like any other property.
 */
export function PresetModelSelect({
  preset,
  llmConfig,
  disabled,
  onCommit,
  onNavigateToModels,
}: PresetModelSelectProps) {
  const options = buildModelSelectOptions(preset, llmConfig.shared.providers, llmConfig);
  const effective = resolvePresetEffectiveModel(preset, llmConfig);

  // Selected value: either the preset's provider_id, "" for inherit, or the dangling id
  const selectedValue = preset.provider_id ?? "";
  const isInherit = !preset.provider_id;
  const isDangling = effective.kind === "dangling";

  // Model ID input is shown when a specific (live) provider is selected
  const showModelInput =
    !isInherit &&
    !isDangling &&
    llmConfig.shared.providers.some((p) => p.id === preset.provider_id);

  const selectedProviderDefaultModel = showModelInput
    ? llmConfig.shared.providers.find((p) => p.id === preset.provider_id)?.default_model ?? ""
    : "";

  const handleSelectChange = (newValue: string) => {
    const providerId = newValue === "" ? null : newValue;
    const { providerId: pid, model } = computeOverrideCommit(providerId, preset.model ?? "");
    onCommit(pid, model);
  };

  const handleModelChange = (newModel: string) => {
    if (!preset.provider_id) return; // invariant: shouldn't happen when input is visible
    const { providerId, model } = computeOverrideCommit(preset.provider_id, newModel);
    onCommit(providerId, model);
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <label htmlFor="preset-model-provider" className="text-xs font-bold text-stone-500 uppercase tracking-widest">
          LLM 模型
        </label>
        {onNavigateToModels && (
          <button
            type="button"
            onClick={onNavigateToModels}
            className="text-[11px] text-stone-500 hover:text-stone-800 flex items-center gap-1 font-bold transition-colors"
            title="去「LLM 模型配置」管理 Provider 与默认绑定"
          >
            <ExternalLink size={11} />
            管理 Provider
          </button>
        )}
      </div>

      <Select id="preset-model-provider" value={selectedValue} disabled={disabled}
        onChange={handleSelectChange} options={options} />

      {isDangling && (
        <div className="flex items-start gap-2 p-3 bg-red-50 border border-red-200 rounded-xl text-xs text-red-700">
          <AlertTriangle size={14} className="shrink-0 mt-0.5" />
          <div>
            此预设原本指向的 Provider <code className="font-mono">{effective.danglingProviderId}</code> 已被删除。
            <br />
            当前实际使用 <span className="font-bold">{effective.fallbackModel}</span>（润色默认）。
            请在上方下拉中重新选择，或选「跟随润色默认」清除引用。
          </div>
        </div>
      )}

      {showModelInput && (
        <div className="space-y-1.5">
          <label className="text-[11px] text-stone-500 font-bold">
            模型 ID（可选，留空使用 Provider 默认 <code className="font-mono">{selectedProviderDefaultModel || "未设置"}</code>）
          </label>
          <input
            type="text"
            value={preset.model ?? ""}
            disabled={disabled}
            placeholder={selectedProviderDefaultModel || "例如 gpt-4o-mini"}
            onChange={(e) => handleModelChange(e.target.value.trimEnd())}
            className="w-full px-4 py-2.5 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl text-sm font-mono focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
          />
        </div>
      )}
    </div>
  );
}
