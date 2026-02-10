import type { HotkeyKey, AsrConfig } from '../types';

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
  const provider = config.selection.active_provider;

  if (provider === 'qwen') {
    return config.credentials.qwen_api_key.trim() !== '';
  } else if (provider === 'doubao') {
    return config.credentials.doubao_app_id.trim() !== '' &&
           config.credentials.doubao_access_token.trim() !== '';
  } else if (provider === 'doubao_ime') {
    // 豆包输入法 ASR 无需用户配置，首次使用时自动注册
    return true;
  } else if (provider === 'siliconflow') {
    return config.credentials.sensevoice_api_key.trim() !== '';
  }

  return false;
};
