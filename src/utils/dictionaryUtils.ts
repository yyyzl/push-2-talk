/**
 * 词典工具函数
 *
 * 统一的词典 ID 生成和格式转换逻辑
 */

import type { DictionaryEntry } from "../types";

/**
 * 生成唯一的词典条目 ID
 *
 * 使用 crypto.randomUUID() 生成安全的唯一 ID
 * 如果浏览器不支持，则回退到时间戳 + 随机数
 */
export function generateDictionaryId(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID().substring(0, 12);
  }
  // 回退方案
  return `${Date.now()}-${Math.random().toString(36).substring(2, 8)}`;
}

/**
 * 解析词典字符串格式
 *
 * @param entry - "word" 或 "word|auto"
 * @returns DictionaryEntry
 */
export function parseEntry(entry: string): DictionaryEntry {
  const parts = entry.split("|");
  const now = Date.now();
  return {
    id: generateDictionaryId(),
    word: parts[0],
    source: parts[1] === "auto" ? "auto" : "manual",
    added_at: Math.floor(now / 1000),
    frequency: 0,
    last_used_at: null,
  };
}

/**
 * 从 DictionaryEntry[] 提取词汇字符串（用于 ASR API）
 */
export function entriesToWords(entries: DictionaryEntry[]): string[] {
  return entries.map((e) => e.word);
}

/**
 * 将 DictionaryEntry[] 转换为存储格式（保留 source 信息）
 *
 * - source = "manual" -> "word"
 * - source = "auto" -> "word|auto"
 */
export function entriesToStorageFormat(entries: DictionaryEntry[]): string[] {
  return entries.map((e) =>
    e.source === "auto" ? `${e.word}|auto` : e.word
  );
}

/**
 * 将旧格式 string[] 转换为 DictionaryEntry[]（向后兼容）
 */
export function wordsToEntries(words: string[]): DictionaryEntry[] {
  return words.map((word) => ({
    id: generateDictionaryId(),
    word,
    source: "manual" as const,
    added_at: Math.floor(Date.now() / 1000),
    frequency: 0,
    last_used_at: null,
  }));
}

/**
 * 创建新的词典条目
 *
 * @param word - 词汇
 * @param source - 来源 ("manual" | "auto")
 */
export function createDictionaryEntry(
  word: string,
  source: "manual" | "auto" = "manual"
): DictionaryEntry {
  return {
    id: generateDictionaryId(),
    word,
    source,
    added_at: Math.floor(Date.now() / 1000),
    frequency: 0,
    last_used_at: null,
  };
}
