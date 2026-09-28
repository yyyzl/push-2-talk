export type BuiltinDictionaryDomain = {
  name: string;
  words: string[];
};

type ImportMetaWithGlob = ImportMeta & {
  glob?: (
    pattern: string,
    options?: {
      eager?: boolean;
      as?: "raw";
      import?: string;
      query?: string;
    },
  ) => Record<string, unknown>;
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

function loadEmbeddedHotwordsRaw(): string {
  const glob = (import.meta as ImportMetaWithGlob).glob;
  if (typeof glob !== "function") {
    return "";
  }

  // Vite 环境：同步读取打包内置 hotwords，避免 fetch 返回前出现空窗期
  const modules = glob("../../hotwords.txt", { eager: true, as: "raw" });
  const first = Object.values(modules)[0];
  return typeof first === "string" ? first : "";
}

function replaceSnapshot(domains: BuiltinDictionaryDomain[]): void {
  BUILTIN_DICTIONARY_DOMAINS.splice(
    0,
    BUILTIN_DICTIONARY_DOMAINS.length,
    ...sanitizeDomains(domains),
  );
  builtinDictionaryMap = createDomainMap(BUILTIN_DICTIONARY_DOMAINS);
}

const embeddedDomains = parseHotwords(loadEmbeddedHotwordsRaw());

// 向后兼容导出：保持同一个数组引用，内部通过 splice 更新内容
export const BUILTIN_DICTIONARY_DOMAINS: BuiltinDictionaryDomain[] = [...embeddedDomains];

let builtinDictionaryMap = createDomainMap(BUILTIN_DICTIONARY_DOMAINS);

export function setBuiltinDomainsSnapshot(domains: BuiltinDictionaryDomain[]): void {
  replaceSnapshot(domains);
}

export async function fetchBuiltinDomains(): Promise<BuiltinDictionaryDomain[]> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const raw = await invoke<string>("get_builtin_domains_raw");
    const parsed = parseHotwords(raw);
    return parsed.length > 0 ? parsed : [...BUILTIN_DICTIONARY_DOMAINS];
  } catch (error) {
    console.warn("获取内置词库失败，回退本地快照:", error);
    return [...BUILTIN_DICTIONARY_DOMAINS];
  }
}

export function normalizeBuiltinDictionaryDomains(
  domains: string[],
  limit: number = BUILTIN_DICTIONARY_LIMIT,
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

function formatBuiltinRuntimeEntry(word: string): string | null {
  const trimmed = word.trim();
  if (!trimmed) return null;
  return `${trimmed}|domain|domain_term`;
}

export function getBuiltinRuntimeEntriesForDomains(domains: string[]): string[] {
  return getBuiltinWordsForDomains(domains)
    .map(formatBuiltinRuntimeEntry)
    .filter((entry): entry is string => Boolean(entry));
}
