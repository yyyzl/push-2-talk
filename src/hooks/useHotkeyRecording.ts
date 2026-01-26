import type React from "react";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AsrConfig,
  AssistantConfig,
  DictionaryEntry,
  DualHotkeyConfig,
  HotkeyKey,
  HotkeyRecordingMode,
  LlmConfig,
} from "../types";
import { isModifierKey, mapDomKeyToHotkeyKey } from "../utils";

export type UseHotkeyRecordingParams = {
  apiKey: string;
  fallbackApiKey: string;
  useRealtime: boolean;
  enablePostProcess: boolean;
  llmConfig: LlmConfig;
  assistantConfig: AssistantConfig;
  asrConfig: AsrConfig;
  enableMuteOtherApps: boolean;
  closeAction: "close" | "minimize" | null;
  dictionary: DictionaryEntry[];

  dualHotkeyConfig: DualHotkeyConfig;
  setDualHotkeyConfig: React.Dispatch<React.SetStateAction<DualHotkeyConfig>>;

  /** 保存配置的回调（用于即时保存并重启服务）
   * @param overrides - 可选的配置覆盖，用于传入最新的状态值
   */
  onSaveConfig?: (overrides?: { dualHotkeyConfig?: DualHotkeyConfig }) => Promise<void>;
};

export type UseHotkeyRecordingResult = {
  isRecordingHotkey: boolean;
  setIsRecordingHotkey: React.Dispatch<React.SetStateAction<boolean>>;
  recordingMode: HotkeyRecordingMode;
  setRecordingMode: React.Dispatch<React.SetStateAction<HotkeyRecordingMode>>;
  recordingKeys: HotkeyKey[];
  hotkeyError: string | null;
  resetHotkeyToDefault: (mode: "dictation" | "assistant" | "release") => void;
};

export function useHotkeyRecording({
  apiKey,
  fallbackApiKey,
  useRealtime,
  enablePostProcess,
  llmConfig,
  assistantConfig,
  asrConfig,
  enableMuteOtherApps,
  closeAction,
  dictionary,
  dualHotkeyConfig,
  setDualHotkeyConfig,
  onSaveConfig,
}: UseHotkeyRecordingParams): UseHotkeyRecordingResult {
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [recordingMode, setRecordingMode] = useState<HotkeyRecordingMode>("dictation");
  const [recordingKeys, setRecordingKeys] = useState<HotkeyKey[]>([]);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const wasHotkeyServiceActiveRef = useRef<boolean | null>(null);

  useEffect(() => {
    if (!isRecordingHotkey) return;

    let cancelled = false;

    void (async () => {
      try {
        const wasActive = await invoke<boolean>("get_hotkey_service_active");
        if (cancelled) return;
        wasHotkeyServiceActiveRef.current = wasActive;
        if (wasActive) {
          await invoke("set_hotkey_service_active", { active: false });
        }
      } catch {
        // ignore
      }
    })();

    return () => {
      cancelled = true;
      if (wasHotkeyServiceActiveRef.current) {
        void invoke("set_hotkey_service_active", { active: true }).catch(() => {
          // ignore
        });
      }
      wasHotkeyServiceActiveRef.current = null;
    };
  }, [isRecordingHotkey]);

  useEffect(() => {
    if (!isRecordingHotkey) {
      setRecordingKeys([]);
      return;
    }

    const pressedKeysSet = new Set<HotkeyKey>();
    let hasRecordedKeys = false;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const key = mapDomKeyToHotkeyKey(e);
      if (key && !pressedKeysSet.has(key)) {
        pressedKeysSet.add(key);
        hasRecordedKeys = true;
        setRecordingKeys(Array.from(pressedKeysSet));
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (!hasRecordedKeys || pressedKeysSet.size === 0) return;

      const keysArray = Array.from(pressedKeysSet);
      const hasModifier = keysArray.some((k) => isModifierKey(k));
      const isFunctionKey = keysArray.every((k) => /^f([1-9]|1[0-2])$/.test(k));

      if (!(hasModifier || isFunctionKey)) {
        setHotkeyError("必须包含修饰键(Ctrl/Alt/Shift/Win) 或 功能键(F1-F12)");
        window.setTimeout(() => setHotkeyError(null), 3000);
        setIsRecordingHotkey(false);
        setRecordingKeys([]);
        return;
      }

      const nextDualHotkeyConfig: DualHotkeyConfig = { ...dualHotkeyConfig };
      if (recordingMode === "dictation") {
        nextDualHotkeyConfig.dictation = {
          ...nextDualHotkeyConfig.dictation,
          keys: keysArray,
        };
      } else if (recordingMode === "release") {
        nextDualHotkeyConfig.dictation = {
          ...nextDualHotkeyConfig.dictation,
          release_mode_keys: keysArray,
        };
      } else {
        nextDualHotkeyConfig.assistant = {
          ...nextDualHotkeyConfig.assistant,
          keys: keysArray,
        };
      }

      setDualHotkeyConfig(nextDualHotkeyConfig);
      setHotkeyError(null);

      // 使用即时保存回调（如果提供）或回退到直接保存
      if (onSaveConfig) {
        void onSaveConfig({ dualHotkeyConfig: nextDualHotkeyConfig }).catch(() => {
          setHotkeyError("保存热键配置失败");
          window.setTimeout(() => setHotkeyError(null), 3000);
        });
      } else {
        void invoke<string>("save_config", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig: null,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig: nextDualHotkeyConfig,
          enableMuteOtherApps,
          closeAction,
          dictionary,
        }).catch(() => {
          setHotkeyError("保存热键配置失败");
          window.setTimeout(() => setHotkeyError(null), 3000);
        });
      }

      setIsRecordingHotkey(false);
      setRecordingKeys([]);
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
    };
  }, [
    isRecordingHotkey,
    recordingMode,
    apiKey,
    fallbackApiKey,
    useRealtime,
    enablePostProcess,
    llmConfig,
    assistantConfig,
    asrConfig,
    enableMuteOtherApps,
    closeAction,
    dictionary,
    dualHotkeyConfig,
    setDualHotkeyConfig,
  ]);

  const resetHotkeyToDefault = (mode: "dictation" | "assistant" | "release") => {
    const defaultDictationKeys = ["control_left", "meta_left"] as HotkeyKey[];
    const defaultAssistantKeys = ["alt_left", "space"] as HotkeyKey[];
    const defaultReleaseKeys = ["f2"] as HotkeyKey[];

    setDualHotkeyConfig((prev) => {
      let next: DualHotkeyConfig;
      if (mode === "assistant") {
        next = {
          ...prev,
          assistant: {
            ...prev.assistant,
            keys: defaultAssistantKeys,
          },
        };
      } else if (mode === "release") {
        next = {
          ...prev,
          dictation: {
            ...prev.dictation,
            release_mode_keys: defaultReleaseKeys,
          },
        };
      } else {
        next = {
          ...prev,
          dictation: {
            ...prev.dictation,
            keys: defaultDictationKeys,
          },
        };
      }

      // 使用即时保存回调（如果提供）或回退到直接保存
      if (onSaveConfig) {
        void onSaveConfig({ dualHotkeyConfig: next }).catch(() => {
          setHotkeyError("保存热键配置失败");
          window.setTimeout(() => setHotkeyError(null), 3000);
        });
      } else {
        void invoke<string>("save_config", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig: null,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig: next,
          enableMuteOtherApps,
          closeAction,
          dictionary,
        }).catch(() => {
          setHotkeyError("保存热键配置失败");
          window.setTimeout(() => setHotkeyError(null), 3000);
        });
      }

      return next;
    });
  };

  return {
    isRecordingHotkey,
    setIsRecordingHotkey,
    recordingMode,
    setRecordingMode,
    recordingKeys,
    hotkeyError,
    resetHotkeyToDefault,
  };
}
