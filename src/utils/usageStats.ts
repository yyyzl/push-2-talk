import type { UsageStats } from '../types';
import { USAGE_STATS_KEY } from '../constants';

export const DEFAULT_USAGE_STATS: UsageStats = {
  totalRecordingMs: 0,
  totalRecordingCount: 0,
  totalRecognizedChars: 0,
};

export const loadUsageStats = (): UsageStats => {
  try {
    const raw = localStorage.getItem(USAGE_STATS_KEY);
    if (!raw) return DEFAULT_USAGE_STATS;

    const parsed = JSON.parse(raw) as Partial<UsageStats> | null;
    return {
      totalRecordingMs: typeof parsed?.totalRecordingMs === 'number' ? parsed.totalRecordingMs : 0,
      totalRecordingCount: typeof parsed?.totalRecordingCount === 'number' ? parsed.totalRecordingCount : 0,
      totalRecognizedChars: typeof parsed?.totalRecognizedChars === 'number' ? parsed.totalRecognizedChars : 0,
    };
  } catch {
    return DEFAULT_USAGE_STATS;
  }
};

export const saveUsageStats = (stats: UsageStats): void => {
  localStorage.setItem(USAGE_STATS_KEY, JSON.stringify(stats));
};

