import type { HistoryRecord } from '../types';
import { HISTORY_KEY, MAX_HISTORY } from '../constants';

export const loadHistory = (): HistoryRecord[] => {
  try {
    const data = localStorage.getItem(HISTORY_KEY);
    return data ? JSON.parse(data) : [];
  } catch {
    return [];
  }
};

export const saveHistory = (records: HistoryRecord[]): void => {
  localStorage.setItem(HISTORY_KEY, JSON.stringify(records.slice(0, MAX_HISTORY)));
};

export const addHistoryRecord = (
  records: HistoryRecord[],
  record: HistoryRecord
): HistoryRecord[] => {
  const updated = [record, ...records].slice(0, MAX_HISTORY);
  saveHistory(updated);
  return updated;
};

export const clearHistory = (): void => {
  localStorage.setItem(HISTORY_KEY, JSON.stringify([]));
};
