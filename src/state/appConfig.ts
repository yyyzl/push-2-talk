import type { AppConfig } from "../types";
import { DEFAULT_ASSISTANT_CONFIG, DEFAULT_DUAL_HOTKEY_CONFIG, DEFAULT_LEARNING_CONFIG, DEFAULT_LLM_CONFIG, DEFAULT_QWEN_ASR_PROFILE, DEFAULT_SEARCH_CONFIG, DEFAULT_TNL_CONFIG, normalizeLearningConfig, normalizeTnlConfig } from "../constants";
import { normalizeLoadedAssistant, normalizeLoadedLlm } from "../utils/loadedConfig";
import { normalizeBuiltinDictionaryDomains } from "../utils/builtinDictionary";

export function initialConfig(): AppConfig {
  return structuredClone({
    dashscope_api_key: "", siliconflow_api_key: "",
    asr_config: { credentials: { qwen_api_key: "", sensevoice_api_key: "", doubao_app_id: "", doubao_access_token: "", doubao_ime_device_id: "", doubao_ime_token: "", doubao_ime_cdid: "" }, selection: { active_provider: "doubao_ime", enable_fallback: false, fallback_provider: null }, qwen_profile: DEFAULT_QWEN_ASR_PROFILE, language_mode: "auto" },
    use_realtime_asr: false, enable_llm_post_process: false, enable_dictionary_enhancement: false,
    llm_config: DEFAULT_LLM_CONFIG, assistant_config: DEFAULT_ASSISTANT_CONFIG, search_config: DEFAULT_SEARCH_CONFIG,
    learning_config: DEFAULT_LEARNING_CONFIG, tnl_config: DEFAULT_TNL_CONFIG, close_action: null,
    hotkey_config: DEFAULT_DUAL_HOTKEY_CONFIG.dictation, dual_hotkey_config: DEFAULT_DUAL_HOTKEY_CONFIG,
    enable_mute_other_apps: false, dictionary: [], builtin_dictionary_domains: [], theme: "light",
  });
}

/** Display defaults only. This function never marks a setting dirty or writes it back. */
export function normalizeConfig(config: AppConfig): AppConfig {
  return {
    ...config,
    llm_config: normalizeLoadedLlm(config.llm_config),
    assistant_config: normalizeLoadedAssistant(config.assistant_config),
    search_config: config.search_config ?? DEFAULT_SEARCH_CONFIG,
    learning_config: normalizeLearningConfig(config.learning_config ?? DEFAULT_LEARNING_CONFIG),
    tnl_config: normalizeTnlConfig(config.tnl_config),
    close_action: config.close_action ?? null,
    dual_hotkey_config: config.dual_hotkey_config ?? DEFAULT_DUAL_HOTKEY_CONFIG,
    builtin_dictionary_domains: normalizeBuiltinDictionaryDomains(config.builtin_dictionary_domains ?? []),
  };
}

export function configValues(config: AppConfig) {
  return {
    apiKey: config.asr_config.credentials.qwen_api_key,
    fallbackApiKey: config.asr_config.credentials.sensevoice_api_key,
    asrConfig: config.asr_config, useRealtime: config.use_realtime_asr,
    enablePostProcess: config.enable_llm_post_process, enableDictionaryEnhancement: config.enable_dictionary_enhancement,
    llmConfig: config.llm_config, assistantConfig: config.assistant_config, searchConfig: config.search_config,
    learningConfig: config.learning_config, tnlConfig: config.tnl_config, dualHotkeyConfig: config.dual_hotkey_config,
    enableMuteOtherApps: config.enable_mute_other_apps, theme: config.theme, closeAction: config.close_action,
    builtinDictionaryDomains: config.builtin_dictionary_domains,
  };
}
