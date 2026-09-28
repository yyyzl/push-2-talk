import { DEFAULT_ASSISTANT_CONFIG, DEFAULT_LLM_CONFIG } from "../constants";
import type { AssistantConfig, LlmConfig } from "../types";

export function normalizeLoadedLlm(config: LlmConfig | null | undefined): LlmConfig {
  const loaded = config || DEFAULT_LLM_CONFIG;
  if (!loaded.presets) {
    return { ...loaded, presets: DEFAULT_LLM_CONFIG.presets, active_preset_id: DEFAULT_LLM_CONFIG.active_preset_id };
  }
  if (loaded.presets.length === 0) return loaded;
  return { ...loaded, active_preset_id: loaded.presets.some(p => p.id === loaded.active_preset_id) ? loaded.active_preset_id : loaded.presets[0].id };
}

export function normalizeLoadedAssistant(config: AssistantConfig | null | undefined): AssistantConfig {
  return {
    ...DEFAULT_ASSISTANT_CONFIG,
    ...config,
    qa_system_prompt: config?.qa_system_prompt ?? DEFAULT_ASSISTANT_CONFIG.qa_system_prompt,
    text_processing_system_prompt: config?.text_processing_system_prompt ?? DEFAULT_ASSISTANT_CONFIG.text_processing_system_prompt,
  };
}
