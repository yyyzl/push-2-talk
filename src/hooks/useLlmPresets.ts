import type React from "react";
import { useCallback, useMemo } from "react";
import { nanoid } from "nanoid";
import type { LlmConfig, LlmPreset } from "../types";

export type UseLlmPresetsParams = {
  llmConfig: LlmConfig;
  setLlmConfig: React.Dispatch<React.SetStateAction<LlmConfig>>;
};

export type UseLlmPresetsResult = {
  activePreset: LlmPreset;
  handleAddPreset: () => void;
  handleDeletePreset: (id: string) => void;
  handleUpdateActivePreset: (key: keyof LlmPreset, value: string) => void;
};

export function useLlmPresets({
  llmConfig,
  setLlmConfig,
}: UseLlmPresetsParams): UseLlmPresetsResult {
  const activePreset = useMemo(() => {
    return (
      llmConfig.presets.find((p) => p.id === llmConfig.active_preset_id) ||
      llmConfig.presets[0] || { id: "", name: "", system_prompt: "" }
    );
  }, [llmConfig.active_preset_id, llmConfig.presets]);

  const handleAddPreset = useCallback(() => {
    const newPreset: LlmPreset = {
      id: nanoid(8),
      name: "新预设",
      system_prompt: "",
    };

    setLlmConfig((prev) => ({
      ...prev,
      presets: [...prev.presets, newPreset],
      active_preset_id: newPreset.id,
    }));
  }, [setLlmConfig]);

  const handleDeletePreset = useCallback(
    (id: string) => {
      setLlmConfig((prev) => {
        if (prev.presets.length <= 1) return prev;

        const newPresets = prev.presets.filter((p) => p.id !== id);
        const newActiveId =
          prev.active_preset_id === id ? newPresets[0]?.id || "" : prev.active_preset_id;

        return {
          ...prev,
          presets: newPresets,
          active_preset_id: newActiveId,
        };
      });
    },
    [setLlmConfig],
  );

  const handleUpdateActivePreset = useCallback(
    (key: keyof LlmPreset, value: string) => {
      setLlmConfig((prev) => ({
        ...prev,
        presets: prev.presets.map((p) =>
          p.id === prev.active_preset_id ? { ...p, [key]: value } : p,
        ),
      }));
    },
    [setLlmConfig],
  );

  return {
    activePreset,
    handleAddPreset,
    handleDeletePreset,
    handleUpdateActivePreset,
  };
}

