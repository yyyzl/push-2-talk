import type React from "react";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DualHotkeyConfig, HotkeyKey, HotkeyRecordingMode } from "../types";
import { DEFAULT_DUAL_HOTKEY_CONFIG } from "../constants";
import { isMacos } from "./usePlatform";
import { isModifierKey, mapDomKeyToHotkeyKey } from "../utils";

export type UseHotkeyRecordingParams = {
  dualHotkeyConfig: DualHotkeyConfig;
  setDualHotkeyConfig: React.Dispatch<React.SetStateAction<DualHotkeyConfig>>;
  onSaveConfig: (overrides?: { dualHotkeyConfig?: DualHotkeyConfig }) => Promise<void>;
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
  dualHotkeyConfig,
  setDualHotkeyConfig,
  onSaveConfig,
}: UseHotkeyRecordingParams): UseHotkeyRecordingResult {
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [recordingMode, setRecordingMode] = useState<HotkeyRecordingMode>("dictation");
  const [recordingKeys, setRecordingKeys] = useState<HotkeyKey[]>([]);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const wasHotkeyServiceActiveRef = useRef<boolean | null>(null);

  // 用 ref 持有回调和配置，避免它们的引用变化导致 keydown/keyup useEffect 重建
  // （重建会清空 pressedKeysSet，导致录制失败）
  const onSaveConfigRef = useRef(onSaveConfig);
  useEffect(() => { onSaveConfigRef.current = onSaveConfig; }, [onSaveConfig]);
  const dualHotkeyConfigRef = useRef(dualHotkeyConfig);
  useEffect(() => { dualHotkeyConfigRef.current = dualHotkeyConfig; }, [dualHotkeyConfig]);

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
      e.stopPropagation();
      if (!hasRecordedKeys || pressedKeysSet.size === 0) return;

      const keysArray = Array.from(pressedKeysSet);
      const hasModifier = keysArray.some((key) => isModifierKey(key));
      const isFunctionKey = keysArray.every((key) => /^f([1-9]|1[0-2])$/.test(key));
      const modifierHint = isMacos ? "Ctrl/Option/Shift/Cmd" : "Ctrl/Alt/Shift/Win";

      if (!(hasModifier || isFunctionKey)) {
        setHotkeyError(`必须包含修饰键 ${modifierHint} 或功能键 F1-F12`);
        window.setTimeout(() => setHotkeyError(null), 3000);
        setIsRecordingHotkey(false);
        setRecordingKeys([]);
        return;
      }

      const currentConfig = dualHotkeyConfigRef.current;
      const nextDualHotkeyConfig: DualHotkeyConfig = { ...currentConfig };
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

      void onSaveConfigRef.current({ dualHotkeyConfig: nextDualHotkeyConfig }).catch(() => {
        setHotkeyError("保存热键配置失败");
        window.setTimeout(() => setHotkeyError(null), 3000);
      });

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
    setDualHotkeyConfig,
  ]);

  const resetHotkeyToDefault = (mode: "dictation" | "assistant" | "release") => {
    const defaultDictationKeys = [...DEFAULT_DUAL_HOTKEY_CONFIG.dictation.keys] as HotkeyKey[];
    const defaultAssistantKeys = [...DEFAULT_DUAL_HOTKEY_CONFIG.assistant.keys] as HotkeyKey[];
    const defaultReleaseKeys = [
      ...(DEFAULT_DUAL_HOTKEY_CONFIG.dictation.release_mode_keys || (["f2"] as HotkeyKey[])),
    ] as HotkeyKey[];

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

      void onSaveConfigRef.current({ dualHotkeyConfig: next }).catch(() => {
        setHotkeyError("保存热键配置失败");
        window.setTimeout(() => setHotkeyError(null), 3000);
      });

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
