import type { DictionaryEntry } from "../types";
import { entriesToRuntimeFormat } from "./dictionaryUtils";
import { getBuiltinRuntimeEntriesForDomains } from "./builtinDictionary";

export const buildRuntimeDictionary = (
  dictionaryEntries: DictionaryEntry[],
  builtinDomains: string[],
  recentHotwordEntries: string[] = [],
): string[] => {
  const userEntries = entriesToRuntimeFormat(dictionaryEntries);
  const builtinEntries = getBuiltinRuntimeEntriesForDomains(builtinDomains);
  if (recentHotwordEntries.length === 0 && builtinEntries.length === 0) return userEntries;

  const merged = new Set<string>();
  const result: string[] = [];

  for (const entry of userEntries) {
    const word = runtimeEntryWord(entry);
    const key = word.toLocaleLowerCase();
    if (!word || merged.has(key)) continue;
    merged.add(key);
    result.push(entry);
  }

  for (const entry of recentHotwordEntries) {
    const word = runtimeEntryWord(entry);
    const key = word.toLocaleLowerCase();
    if (!word || merged.has(key)) continue;
    merged.add(key);
    result.push(entry);
  }

  for (const entry of builtinEntries) {
    const word = runtimeEntryWord(entry);
    const key = word.toLocaleLowerCase();
    if (!word || merged.has(key)) continue;
    merged.add(key);
    result.push(entry);
  }

  return result;
};

function runtimeEntryWord(entry: string): string {
  return (entry.split("|")[0] || "").trim();
}
