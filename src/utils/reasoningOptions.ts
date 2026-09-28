import type { AssistantConfig, LlmConfig, ReasoningEffort, SharedLlmConfig } from "../types";

export type ReasoningContext =
  | { kind: "polishing"; config: LlmConfig }
  | { kind: "assistant"; config: AssistantConfig; shared: SharedLlmConfig; text_processing: boolean };

export type ReasoningOptions = {
  model: string;
  efforts: ReasoningEffort[];
  legacy_hint: string | null;
};

export const REASONING_LABELS: Record<ReasoningEffort, string> = {
  default: "默认（沿用已有配置）", none: "关闭", auto: "开启（使用模型默认强度）",
  low: "低", medium: "中", high: "高", xhigh: "极高",
};

export function reasoningSelectOptions(value: ReasoningEffort | undefined, capabilities?: ReasoningOptions) {
  const efforts = capabilities?.efforts ?? ["default"];
  const options = efforts.map((effort) => ({ value: effort, label: REASONING_LABELS[effort], disabled: false }));
  if (value && !efforts.includes(value)) {
    options.push({ value, label: `已保存：${REASONING_LABELS[value]}（旧设置）`, disabled: true });
  }
  return options;
}
