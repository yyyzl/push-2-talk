import type { HotkeyKey, AsrConfig, AsrProvider } from '../types';
import { ASR_PROVIDERS } from '../constants';

// 将 DOM KeyboardEvent 映射为 HotkeyKey
export const mapDomKeyToHotkeyKey = (e: KeyboardEvent): HotkeyKey | null => {
  const { key, code, location } = e;

  // 修饰键（带位置）
  if (key === 'Control') return location === 1 ? 'control_left' : 'control_right';
  if (key === 'Shift') return location === 1 ? 'shift_left' : 'shift_right';
  if (key === 'Alt') return location === 1 ? 'alt_left' : 'alt_right';
  if (key === 'Meta') return location === 1 ? 'meta_left' : 'meta_right';

  // 特殊键
  if (key === ' ') return 'space';
  if (key === 'Tab') return 'tab';
  if (key === 'Escape') return 'escape';
  if (key === 'CapsLock') return 'caps_lock';

  // 功能键
  if (/^F([1-9]|1[0-2])$/.test(key)) {
    return `f${key.slice(1).toLowerCase()}` as HotkeyKey;
  }

  // 字母键
  if (/^Key[A-Z]$/.test(code)) {
    return `key_${code.slice(3).toLowerCase()}` as HotkeyKey;
  }

  // 数字键 (Top Row)
  if (/^Digit[0-9]$/.test(code)) {
    return `num_${code.slice(5)}` as HotkeyKey;
  }

  // 小键盘数字键 (Numpad)
  if (/^Numpad[0-9]$/.test(code)) {
    return `num_${code.slice(6)}` as HotkeyKey;
  }

  // 方向键
  if (key === 'ArrowUp') return 'up';
  if (key === 'ArrowDown') return 'down';
  if (key === 'ArrowLeft') return 'left';
  if (key === 'ArrowRight') return 'right';

  // 编辑键
  if (key === 'Enter') return 'return';
  if (key === 'Backspace') return 'backspace';
  if (key === 'Delete') return 'delete';
  if (key === 'Insert') return 'insert';
  if (key === 'Home') return 'home';
  if (key === 'End') return 'end';
  if (key === 'PageUp') return 'page_up';
  if (key === 'PageDown') return 'page_down';

  return null;
};

// 判断是否是修饰键
export const isModifierKey = (key: HotkeyKey): boolean => {
  return [
    'control_left', 'control_right',
    'shift_left', 'shift_right',
    'alt_left', 'alt_right',
    'meta_left', 'meta_right'
  ].includes(key);
};

// 判断是否是功能键 (F1-F12)
export const isFunctionKey = (key: HotkeyKey): boolean => {
  return /^f([1-9]|1[0-2])$/.test(key);
};

// 验证热键组合是否有效（必须包含修饰键或为纯功能键）
export const isValidHotkeyCombo = (keys: HotkeyKey[]): boolean => {
  const hasModifier = keys.some(k => isModifierKey(k));
  const allFunctionKeys = keys.every(k => isFunctionKey(k));
  return hasModifier || allFunctionKeys;
};

// 验证 ASR 配置是否有效
export const isAsrConfigValid = (config: AsrConfig): boolean => {
  return (config.omni?.api_key ?? '').trim() !== '' && (config.omni?.endpoint ?? '').trim() !== '';
};

/**
 * 当 ASR 配置无效时，尝试回退到 FALLBACK_ASR_PROVIDER。
 * 返回 { config, didFallback } — didFallback 为 true 表示发生了回退。
 */
export const normalizeAsrConfigWithFallback = (
  config: AsrConfig,
): { config: AsrConfig; didFallback: boolean } => {
  const omniConfig: AsrConfig = {
    ...config,
    selection: {
      ...config.selection,
      active_provider: "omni",
      enable_fallback: false,
      fallback_provider: null,
    },
  };

  if (isAsrConfigValid(omniConfig)) {
    return {
      config: omniConfig,
      didFallback: config.selection.active_provider !== "omni",
    };
  }

  if (isAsrConfigValid(config)) {
    return { config, didFallback: false };
  }

  return { config: omniConfig, didFallback: config.selection.active_provider !== "omni" };
};

/** 获取 ASR Provider 的中文显示名称 */
export const getAsrProviderDisplayName = (provider: AsrProvider): string => {
  return ASR_PROVIDERS[provider]?.name ?? provider;
};
