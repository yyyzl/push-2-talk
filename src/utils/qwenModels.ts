import catalogue from "../shared/qwen-models.json";
import { normalizeQwenAsrProfile, QWEN_ASR_PROFILES } from "../constants";
import type { AsrConfig } from "../types";

export type QwenMode = "http" | "realtime";
export const QWEN_MODELS = catalogue as Array<{
  id: string; label: string; mode: QwenMode; protocol: "audio3" | "qwen3" | "message";
}>;

export function selectedQwenModel(config: AsrConfig, mode: QwenMode): string {
  const profile = QWEN_ASR_PROFILES[normalizeQwenAsrProfile(config.qwen_profile)];
  return config.qwen_models?.[mode] ?? (mode === "http" ? profile.httpModel : profile.realtimeModel);
}

export function qwenModelOptions(config: AsrConfig, mode: QwenMode) {
  const options = QWEN_MODELS.filter(model => model.mode === mode)
    .map(model => ({ value: model.id, label: model.label, description: model.id }));
  const selected = selectedQwenModel(config, mode);
  if (!options.some(option => option.value === selected)) {
    options.unshift({ value: selected, label: `${selected || "空模型名"}（暂不支持，请重新选择）`, description: "原配置已保留" });
  }
  return options;
}

export function withQwenModel(config: AsrConfig, mode: QwenMode, id: string): AsrConfig {
  return { ...config, qwen_models: { ...config.qwen_models, [mode]: id } };
}
