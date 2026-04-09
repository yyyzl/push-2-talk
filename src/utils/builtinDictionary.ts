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

      if (!name || words.length === 0) return null;
      return { name, words: Array.from(new Set(words)) };
    })
    .filter((domain): domain is BuiltinDictionaryDomain => Boolean(domain));
}

function sanitizeDomains(domains: BuiltinDictionaryDomain[]): BuiltinDictionaryDomain[] {
  return domains
    .map((domain) => ({
      name: domain.name.trim(),
      words: Array.from(
        new Set(
          domain.words
            .map((word) => word.trim())
            .filter(Boolean),
        ),
      ),
    }))
    .filter((domain) => domain.name && domain.words.length > 0);
}

function createDomainMap(domains: BuiltinDictionaryDomain[]): Map<string, BuiltinDictionaryDomain> {
  return new Map(domains.map((domain) => [domain.name, domain]));
}

function replaceSnapshot(domains: BuiltinDictionaryDomain[]): void {
  BUILTIN_DICTIONARY_DOMAINS.splice(
    0,
    BUILTIN_DICTIONARY_DOMAINS.length,
    ...sanitizeDomains(domains),
  );
  builtinDictionaryMap = createDomainMap(BUILTIN_DICTIONARY_DOMAINS);
}

// 向后兼容导出：保持同一个数组引用，内部通过 splice 更新内容
export const BUILTIN_DICTIONARY_DOMAINS: BuiltinDictionaryDomain[] = [];

let builtinDictionaryMap = createDomainMap(BUILTIN_DICTIONARY_DOMAINS);

export function setBuiltinDomainsSnapshot(domains: BuiltinDictionaryDomain[]): void {
  replaceSnapshot(domains);
}

export async function fetchBuiltinDomains(): Promise<BuiltinDictionaryDomain[]> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const raw = await invoke<string>("get_builtin_domains_raw");
    if (!raw.trim()) {
      return [];
    }
    return parseHotwords(raw);
  } catch (error) {
    console.warn("获取远端内置词库失败，继续使用当前快照:", error);
    return [...BUILTIN_DICTIONARY_DOMAINS];
  }
}

export function normalizeBuiltinDictionaryDomains(
  domains: string[],
  limit: number = BUILTIN_DICTIONARY_LIMIT,
): string[] {
  const normalized: string[] = [];
  const seen = new Set<string>();
  const shouldFilterBySnapshot = builtinDictionaryMap.size > 0;

  for (const domain of domains) {
    const trimmed = domain.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    if (shouldFilterBySnapshot && !builtinDictionaryMap.has(trimmed)) continue;
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
