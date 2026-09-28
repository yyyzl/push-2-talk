/** Typed IPC boundary for configuration and application service lifecycle. */
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, AsrConfig, AssistantConfig, DualHotkeyConfig, LlmConfig, SearchConfig } from "../types";
import type { ConfigSnapshot, DeepPatch } from "../state/configStore";
import { normalizeConfig } from "../state/appConfig";

export type StartApp = {
  apiKey: string; fallbackApiKey: string; useRealtime: boolean; enablePostProcess: boolean;
  enableDictionaryEnhancement: boolean; llmConfig: LlmConfig; assistantConfig: AssistantConfig;
  smartCommandConfig: null; searchConfig?: SearchConfig; asrConfig: AsrConfig | null;
  dualHotkeyConfig: DualHotkeyConfig; enableMuteOtherApps: boolean; dictionary: string[]; theme: string;
};
export type RuntimeConfig = {
  enablePostProcess?: boolean; enableDictionaryEnhancement?: boolean; llmConfig?: LlmConfig;
  assistantConfig?: AssistantConfig; enableMuteOtherApps?: boolean; dictionary?: string[];
};
export type ConfigPatch = DeepPatch<Omit<AppConfig, "dictionary" | "dashscope_api_key" | "siliconflow_api_key" | "hotkey_config">>;
const normalized = (snapshot: ConfigSnapshot<AppConfig>): ConfigSnapshot<AppConfig> => ({ ...snapshot, config: normalizeConfig(snapshot.config) });
export function createDesktop(call: typeof invoke = invoke) {
  let serviceQueue: Promise<unknown> = Promise.resolve();
  function lifecycle<T>(work: () => Promise<T>): Promise<T> {
    const next = serviceQueue.then(work, work);
    serviceQueue = next.catch(() => undefined);
    return next;
  }
  return {
    getConfig: async () => normalized(await call<ConfigSnapshot<AppConfig>>("get_config_snapshot")),
    updateConfig: async (patch: ConfigPatch) => normalized(await call<ConfigSnapshot<AppConfig>>("update_config", { patch })),
    // Dictionary CRUD owns SQLite. This legacy command is limited to explicit bulk dictionary imports.
    saveDictionary: (dictionary: string[]) => call<string>("save_config", { apiKey: "", fallbackApiKey: "", dictionary }),
    getDictionary: () => call<string[]>("get_dictionary_entries"),
    start: (payload: StartApp) => lifecycle(() => call<string>("start_app", payload)),
    stop: () => lifecycle(() => call<string>("stop_app")),
    updateRuntime: (payload: RuntimeConfig) => call<string>("update_runtime_config", payload),
    getAutostart: () => call<boolean>("get_autostart"),
    setAutostart: (enabled: boolean) => call<string>("set_autostart", { enabled }),
    cancelTranscription: () => call<string>("cancel_transcription"),
    quit: () => lifecycle(() => call<void>("quit_app")),
    hide: () => call<string>("hide_to_tray"),
  };
}

export const desktop = createDesktop();
