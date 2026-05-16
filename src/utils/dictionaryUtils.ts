/**
 * 词典工具函数
 *
 * 统一的词典 ID 生成和格式转换逻辑
 */

import type { DictionaryCategory, DictionaryEntry } from "../types";

export const DICTIONARY_CATEGORY_OPTIONS: Array<{
  value: DictionaryCategory;
  label: string;
}> = [
  { value: "person", label: "人名" },
  { value: "product", label: "产品" },
  { value: "tool", label: "工具" },
  { value: "phrase", label: "短语" },
  { value: "email", label: "邮箱" },
  { value: "url", label: "链接" },
  { value: "code_symbol", label: "代码" },
  { value: "domain_term", label: "术语" },
  { value: "generic", label: "通用" },
];

const DICTIONARY_CATEGORY_VALUES = new Set<DictionaryCategory>(
  DICTIONARY_CATEGORY_OPTIONS.map((option) => option.value),
);

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
 * @param entry - "word"、"word|auto" 或 "word|source|category"
 * @returns DictionaryEntry
 */
function normalizeDictionaryCategory(
  category: string | null | undefined,
): DictionaryCategory | null {
  const normalized = category?.trim();
  if (!normalized) return null;

  if (DICTIONARY_CATEGORY_VALUES.has(normalized as DictionaryCategory)) {
    return normalized as DictionaryCategory;
  }

  if (normalized === "proper_noun") return "product";
  if (normalized === "term") return "domain_term";
  if (normalized === "frequent") return "generic";

  return null;
}

function normalizeDictionarySource(source: string | null | undefined): DictionaryEntry["source"] {
  return source === "auto" ? "auto" : "manual";
}

export function inferDictionaryCategory(word: string): DictionaryCategory {
  const trimmed = word.trim();
  if (!trimmed) return "generic";

  if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmed)) {
    return "email";
  }

  if (/^(https?:\/\/|www\.)\S+/i.test(trimmed)) {
    return "url";
  }

  if (
    /^[\x00-\x7F]+$/.test(trimmed) &&
    (/[a-z][A-Z]/.test(trimmed) ||
      /[_/\\]/.test(trimmed) ||
      /[A-Za-z0-9]+-[A-Za-z0-9]+/.test(trimmed))
  ) {
    return "code_symbol";
  }

  const chineseCharCount = Array.from(trimmed).filter((char) =>
    /[\u4e00-\u9fff]/.test(char)
  ).length;
  if (chineseCharCount >= 2 && !/\s/.test(trimmed)) {
    return "phrase";
  }

  return "generic";
}

export function normalizeDictionaryEntry(
  entry: Partial<DictionaryEntry> & { word?: string },
): DictionaryEntry {
  const word = (entry.word || "").trim();
  const source = normalizeDictionarySource(entry.source);
  const category = normalizeDictionaryCategory(entry.category) ?? inferDictionaryCategory(word);
  const now = Date.now();
  return {
    id: entry.id || generateDictionaryId(),
    word,
    source,
    category,
    added_at: entry.added_at ?? Math.floor(now / 1000),
    frequency: entry.frequency ?? 0,
    last_used_at: entry.last_used_at ?? null,
  };
}

export function parseEntry(entry: string): DictionaryEntry {
  const parts = entry.split("|");
  const word = (parts[0] || "").trim();
  const source = normalizeDictionarySource(parts[1]);
  const category = normalizeDictionaryCategory(parts[2]) ?? inferDictionaryCategory(word);

  return normalizeDictionaryEntry({ word, source, category });
}

/**
 * 从 DictionaryEntry[] 提取词汇字符串（用于 ASR API）
 */
export function entriesToWords(entries: DictionaryEntry[]): string[] {
  return entries.map((e) => e.word);
}

/**
 * 将 DictionaryEntry[] 转换为存储格式（保留 source/category 信息）
 *
 * - source = "manual" -> "word"
 * - source = "auto" -> "word|auto"
 * - category 非 generic -> "word|source|category"
 */
export function entriesToStorageFormat(entries: DictionaryEntry[]): string[] {
  return entries
    .map((entry) => {
      const word = entry.word.trim();
      const source = normalizeDictionarySource(entry.source);
      const category =
        normalizeDictionaryCategory(entry.category) ?? inferDictionaryCategory(word);

      if (category === "generic") {
        return source === "auto" ? `${word}|auto` : word;
      }

      return `${word}|${source}|${category}`;
    })
    .filter(Boolean);
}

/**
 * 将 DictionaryEntry[] 转换为运行时格式。
 *
 * 运行时格式保留 source/category，供后端 HotwordCompiler 计算来源权重；
 * TNL/LLM 入口负责提纯为纯词，避免 metadata 泄漏到实际文本处理。
 */
export function entriesToRuntimeFormat(entries: DictionaryEntry[]): string[] {
  return entriesToStorageFormat(entries);
}

/**
 * 将旧格式 string[] 转换为 DictionaryEntry[]（向后兼容）
 */
export function wordsToEntries(words: string[]): DictionaryEntry[] {
  return words.map(parseEntry);
}

/**
 * 创建新的词典条目
 *
 * @param word - 词汇
 * @param source - 来源 ("manual" | "auto")
 */
export function createDictionaryEntry(
  word: string,
  source: "manual" | "auto" = "manual",
  category?: DictionaryCategory,
): DictionaryEntry {
  return normalizeDictionaryEntry({
    word,
    source,
    category: category ?? inferDictionaryCategory(word),
  });
}
