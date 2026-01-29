import hotwordsRaw from "../../hotwords.txt?raw";

export type BuiltinDictionaryDomain = {
  name: string;
  words: string[];
};

export const BUILTIN_DICTIONARY_LIMIT = 5;

const HOTWORDS_LINE_RE = /^\s*【(.+?)】:\[(.*)\]\s*$/;

function parseHotwords(raw: string): BuiltinDictionaryDomain[] {
  return raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const match = HOTWORDS_LINE_RE.exec(line);
      if (!match) return null;
      const name = match[1].trim();
      const words = match[2]
        .split(",")
        .map((word) => word.trim())
        .filter(Boolean);
      const uniqueWords = Array.from(new Set(words));
      return { name, words: uniqueWords };
    })
    .filter((domain): domain is BuiltinDictionaryDomain => Boolean(domain));
}

export const BUILTIN_DICTIONARY_DOMAINS = parseHotwords(hotwordsRaw);

const builtinDictionaryMap = new Map(
  BUILTIN_DICTIONARY_DOMAINS.map((domain) => [domain.name, domain])
);

export function normalizeBuiltinDictionaryDomains(
  domains: string[],
  limit: number = BUILTIN_DICTIONARY_LIMIT
): string[] {
  const normalized: string[] = [];
  const seen = new Set<string>();

  for (const domain of domains) {
    const trimmed = domain.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    if (!builtinDictionaryMap.has(trimmed)) continue;
    normalized.push(trimmed);
    seen.add(trimmed);
    if (normalized.length >= limit) break;
  }

  return normalized;
}

export function getBuiltinWordsForDomains(domains: string[]): string[] {
  const words: string[] = [];
  const seen = new Set<string>();

  for (const domain of domains) {
    const entry = builtinDictionaryMap.get(domain);
    if (!entry) continue;
    for (const word of entry.words) {
      if (seen.has(word)) continue;
      seen.add(word);
      words.push(word);
    }
  }

  return words;
}
