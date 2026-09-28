import { useMemo, useState, useSyncExternalStore, type SetStateAction } from "react";
import type { AppConfig } from "../types";
import { ConfigStore } from "../state/configStore";
import { configValues, initialConfig } from "../state/appConfig";

export function useAppConfig() {
  const [store] = useState(() => new ConfigStore(initialConfig()));
  const view = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const setters = useMemo(() => {
    const bind = <K extends keyof AppConfig>(key: K) => (value: SetStateAction<AppConfig[K]>) => {
      store.edit(current => ({ ...current, [key]: typeof value === "function" ? (value as (previous: AppConfig[K]) => AppConfig[K])(current[key]) : value }), typeof value === "function" ? undefined : view.config);
    };
    return {
      setAsrConfig: bind("asr_config"), setUseRealtime: bind("use_realtime_asr"),
      setEnablePostProcess: bind("enable_llm_post_process"), setEnableDictionaryEnhancement: bind("enable_dictionary_enhancement"),
      setLlmConfig: bind("llm_config"), setAssistantConfig: bind("assistant_config"), setSearchConfig: bind("search_config"),
      setLearningConfig: bind("learning_config"), setTnlConfig: bind("tnl_config"), setDualHotkeyConfig: bind("dual_hotkey_config"),
      setEnableMuteOtherApps: bind("enable_mute_other_apps"), setTheme: bind("theme"), setCloseAction: bind("close_action"),
      setBuiltinDictionaryDomains: bind("builtin_dictionary_domains"),
    };
  }, [store, view.config]);
  return { store, view, ...configValues(view.config), ...setters };
}
