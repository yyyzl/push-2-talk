/**
 * ConfigSaveContext - 全局配置保存上下文
 *
 * 提供即时保存配置并重启服务的能力，供所有需要立即生效的配置使用。
 *
 * 使用场景：
 * - ASR 引擎切换
 * - 实时/HTTP 模式切换
 * - 热键配置修改
 * - 词库修改（realtime 模式）
 * - 静音其他应用开关
 * - Fallback 配置修改
 */

import { createContext, useContext } from "react";
import type { AsrConfig, AssistantConfig, DictionaryEntry, DualHotkeyConfig, LlmConfig } from "../types";

export type ConfigSyncStatus = "idle" | "syncing" | "success" | "error";

/**
 * 配置覆盖参数（用于解决 React setState 异步问题）
 */
export type ConfigOverrides = {
  useRealtime?: boolean;
  enablePostProcess?: boolean;
  enableDictionaryEnhancement?: boolean;
  llmConfig?: LlmConfig;
  assistantConfig?: AssistantConfig;
  asrConfig?: AsrConfig;
  dualHotkeyConfig?: DualHotkeyConfig;
  enableMuteOtherApps?: boolean;
  dictionary?: DictionaryEntry[];
  theme?: string;
};

export type ConfigSaveContextValue = {
  /** 立即保存配置并重启服务（绕过 debounce）
   * @param overrides - 可选的配置覆盖，用于传入最新的状态值
   */
  saveImmediately: (overrides?: ConfigOverrides) => Promise<void>;
  /** 当前同步状态 */
  syncStatus: ConfigSyncStatus;
  /** 是否正在保存（便捷属性） */
  isSaving: boolean;
};

export const ConfigSaveContext = createContext<ConfigSaveContextValue | null>(null);

/**
 * 使用配置保存上下文
 *
 * @throws 如果在 ConfigSaveContext.Provider 外部使用
 */
export function useConfigSave(): ConfigSaveContextValue {
  const context = useContext(ConfigSaveContext);

  if (!context) {
    throw new Error(
      "useConfigSave must be used within ConfigSaveContext.Provider"
    );
  }

  return context;
}
