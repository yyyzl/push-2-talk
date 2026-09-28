import type { ConfigOverrides } from "../contexts/ConfigSaveContext";
import type { desktop } from "../services/desktop";
import type { AppConfig } from "../types";
import { normalizeAsrConfigWithFallback } from "../utils/hotkey";
import { entriesToStorageFormat, parseEntry } from "../utils/dictionaryUtils";
import { ConfigStore } from "./configStore";

type ConfigurationPort = Pick<typeof desktop, "getConfig" | "updateConfig" | "getDictionary" | "saveDictionary">;

/** Loading a snapshot is never an edit. Repair only the unusable ASR selection, if needed. */
export async function loadConfiguration(store: ConfigStore<AppConfig>, port: ConfigurationPort) {
  store.receive(await port.getConfig());
  let config = store.getSnapshot().config;
  let dictionarySource = config.dictionary;
  try { dictionarySource = await port.getDictionary(); }
  catch { /* Keep the existing JSON projection when SQLite cannot be read. */ }
  const normalized = normalizeAsrConfigWithFallback(config.asr_config);
  if (normalized.didFallback) {
    store.receive(await port.updateConfig({ asr_config: { selection: normalized.config.selection } }));
    config = store.getSnapshot().config;
  }
  return {
    config,
    dictionary: dictionarySource.filter(entry => typeof entry === "string" && entry.trim()).map(parseEntry),
    didFallback: normalized.didFallback,
  };
}

export async function saveConfiguration(
  store: ConfigStore<AppConfig>, port: ConfigurationPort,
  overrides: ConfigOverrides = {}, observed: AppConfig = store.getSnapshot().config,
): Promise<AppConfig> {
  const keys = {
    useRealtime: "use_realtime_asr", enablePostProcess: "enable_llm_post_process",
    enableDictionaryEnhancement: "enable_dictionary_enhancement", llmConfig: "llm_config",
    assistantConfig: "assistant_config", searchConfig: "search_config", asrConfig: "asr_config",
    dualHotkeyConfig: "dual_hotkey_config", learningConfig: "learning_config",
    enableMuteOtherApps: "enable_mute_other_apps", builtinDictionaryDomains: "builtin_dictionary_domains",
    theme: "theme",
  } as const;
  store.edit(current => {
    const next = { ...current };
    for (const [input, field] of Object.entries(keys)) {
      const value = overrides[input as keyof typeof keys];
      if (value !== undefined) Object.assign(next, { [field]: value });
    }
    return next;
  }, observed);
  await store.flush(port.updateConfig);
  if (overrides.dictionaryEntries !== undefined) {
    await port.saveDictionary(entriesToStorageFormat(overrides.dictionaryEntries));
    store.receive(await port.getConfig());
  }
  return store.getSnapshot().config;
}
