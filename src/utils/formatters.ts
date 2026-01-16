import type { HotkeyConfig, HotkeyKey } from '../types';
import { KEY_DISPLAY_NAMES } from '../constants';

// 格式化时间戳为 HH:MM:SS
export const formatTimestamp = (ts: number): string => {
  const d = new Date(ts);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`;
};

// 格式化录音时间为 M:SS
export const formatRecordingTime = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

// 格式化热键显示
export const formatHotkeyKeysDisplay = (keys: HotkeyKey[]): string => {
  return keys.map(k => KEY_DISPLAY_NAMES[k] || k).join(' + ');
};

export const formatHotkeyDisplay = (config: HotkeyConfig): string => {
  return formatHotkeyKeysDisplay(config.keys);
};

// 格式化毫秒为秒（保留2位小数）
export const formatMs = (ms: number): string => {
  return (ms / 1000).toFixed(2);
};

// 格式化毫秒为秒（保留1位小数）
export const formatMsShort = (ms: number): string => {
  return (ms / 1000).toFixed(1);
};

// 获取按键显示名称
export const getKeyDisplayName = (key: HotkeyKey): string => {
  return KEY_DISPLAY_NAMES[key] || key;
};
