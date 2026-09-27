import type { DictionaryCategory, HistoryRecord } from "../types";
import { inferDictionaryCategory } from "./dictionaryUtils";

export const RECENT_HOTWORD_WINDOW_MS = 24 * 60 * 60 * 1000;
export const DEFAULT_RECENT_HOTWORD_LIMIT = 20;

type TokenSpan = {
  text: string;
  start: number;
  end: number;
};

const ASCII_TOKEN_PATTERN = /[A-Za-z][A-Za-z0-9+#._-]*/g;
const TITLECASE_STOPWORDS = new Set([
  "A",
  "An",
  "And",
  "Are",
  "But",
  "For",
  "From",
  "Have",
  "Here",
  "Into",
  "Next",
  "Please",
  "That",
  "The",
  "Then",
  "There",
  "These",
  "This",
  "Today",
  "Use",
  "Using",
  "When",
  "Where",
  "With",
  "Would",
]);

export function buildRecentHotwordEntries(
  history: HistoryRecord[],
  now: number = Date.now(),
  limit: number = DEFAULT_RECENT_HOTWORD_LIMIT,
): string[] {
  const minTimestamp = now - RECENT_HOTWORD_WINDOW_MS;
  const seen = new Set<string>();
  const entries: string[] = [];

  for (const record of history) {
    if (entries.length >= limit) break;
    if (!record.success || record.timestamp < minTimestamp) continue;

    const text = (record.polishedText || record.originalText || "").trim();
    if (!text) continue;

    for (const candidate of extractRecentHotwordCandidates(text)) {
      const key = candidate.toLocaleLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      entries.push(formatRecentHotwordEntry(candidate));
      if (entries.length >= limit) break;
    }
  }

  return entries;
}

export function extractRecentHotwordCandidates(text: string): string[] {
  const tokens = tokenizeAscii(text);
  const candidates: string[] = [];

  for (let index = 0; index < tokens.length; index += 1) {
    const phrase = longestJoinablePhrase(tokens, text, index);
    if (phrase) {
      candidates.push(phrase.text);
      index += phrase.tokenCount - 1;
      continue;
    }

    const token = tokens[index].text;
    if (isTechnicalSingleToken(token)) {
      candidates.push(token);
    }
  }

  return dedupeStable(candidates);
}

function formatRecentHotwordEntry(word: string): string {
  const category: DictionaryCategory = inferDictionaryCategory(word);
  return `${word}|recent|${category}`;
}

function tokenizeAscii(text: string): TokenSpan[] {
  return Array.from(text.matchAll(ASCII_TOKEN_PATTERN), (match) => ({
    text: match[0],
    start: match.index ?? 0,
    end: (match.index ?? 0) + match[0].length,
  }));
}

function longestJoinablePhrase(
  tokens: TokenSpan[],
  sourceText: string,
  startIndex: number,
): { text: string; tokenCount: number } | null {
  if (!isPhraseToken(tokens[startIndex].text)) return null;

  let best: { text: string; tokenCount: number } | null = null;
  const parts = [tokens[startIndex].text];

  for (let index = startIndex + 1; index < Math.min(tokens.length, startIndex + 4); index += 1) {
    const separator = sourceText.slice(tokens[index - 1].end, tokens[index].start);
    if (!/^\s{1,3}$/.test(separator) || !isPhraseToken(tokens[index].text)) break;

    parts.push(tokens[index].text);
    if (parts.length >= 2) {
      best = {
        text: parts.join(" "),
        tokenCount: parts.length,
      };
    }
  }

  return best;
}

function isPhraseToken(token: string): boolean {
  return isTitleCaseToken(token) || isTechnicalSingleToken(token);
}

function isTechnicalSingleToken(token: string): boolean {
  if (token.length < 3) return false;
  if (/[0-9+#._-]/.test(token)) return true;
  if (/[a-z][A-Z]/.test(token)) return true;
  if (/^[A-Z]{2,8}$/.test(token)) return true;
  return isTitleCaseToken(token) && token.length >= 5;
}

function isTitleCaseToken(token: string): boolean {
  return /^[A-Z][A-Za-z0-9]{2,}$/.test(token) && !TITLECASE_STOPWORDS.has(token);
}

function dedupeStable(values: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const value of values) {
    const key = value.toLocaleLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(value);
  }

  return result;
}
