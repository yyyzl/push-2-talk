import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  DICTIONARY_CATEGORY_OPTIONS,
  createDictionaryEntry,
  entriesToRuntimeFormat,
  entriesToStorageFormat,
  entriesToWords,
  inferDictionaryCategory,
  parseEntry,
} from "../src/utils/dictionaryUtils";
import type { DictionaryEntry } from "../src/types";

const entry = (
  word: string,
  source: DictionaryEntry["source"],
  category: DictionaryEntry["category"],
): DictionaryEntry => ({
  id: `${word}-${source}-${category}`,
  word,
  source,
  category,
  added_at: 0,
  frequency: 0,
  last_used_at: null,
});

test("词库 category metadata 应兼容旧格式并保留新格式", () => {
  assert.equal(parseEntry("rust|auto").source, "auto");
  assert.equal(parseEntry("rust|auto").category, "generic");
  assert.equal(parseEntry("Claude Code|auto|product").category, "product");
  assert.equal(parseEntry("useState|manual|code_symbol").category, "code_symbol");

  const entries = [
    entry("Claude Code", "auto", "product"),
    entry("团队约定", "manual", "phrase"),
    entry("rust", "manual", "generic"),
  ];

  assert.deepEqual(entriesToStorageFormat(entries), [
    "Claude Code|auto|product",
    "团队约定|manual|phrase",
    "rust",
  ]);
  assert.deepEqual(entriesToRuntimeFormat(entries), [
    "Claude Code|auto|product",
    "团队约定|manual|phrase",
    "rust",
  ]);
  assert.deepEqual(entriesToWords(entries), ["Claude Code", "团队约定", "rust"]);
});

test("词库 category 推断应覆盖 email/url/code/中文短语/兜底", () => {
  assert.equal(inferDictionaryCategory("me@example.com"), "email");
  assert.equal(inferDictionaryCategory("https://example.com/docs"), "url");
  assert.equal(inferDictionaryCategory("useState"), "code_symbol");
  assert.equal(inferDictionaryCategory("async_await"), "code_symbol");
  assert.equal(inferDictionaryCategory("团队约定"), "phrase");
  assert.equal(inferDictionaryCategory("rust"), "generic");

  const created = createDictionaryEntry("www.example.com");
  assert.equal(created.category, "url");

  assert.ok(DICTIONARY_CATEGORY_OPTIONS.some((option) => option.value === "product"));
  assert.ok(DICTIONARY_CATEGORY_OPTIONS.some((option) => option.value === "code_symbol"));
});

test("DictionaryPage 和 useDictionary 应暴露手动修改 category 的保存路径", async () => {
  const pageSource = await readFile("src/pages/DictionaryPage.tsx", "utf8");
  const hookSource = await readFile("src/hooks/useDictionary.ts", "utf8");
  const backendSource = await readFile("src-tauri/src/lib.rs", "utf8");

  assert.match(pageSource, /DICTIONARY_CATEGORY_OPTIONS/);
  assert.match(pageSource, /handleUpdateCategory:\s*\(word:\s*string,\s*category:\s*DictionaryCategory\)/);
  assert.match(pageSource, /<select[\s\S]*value=\{entry\.category\}/);

  assert.match(hookSource, /handleUpdateCategory/);
  assert.match(
    hookSource,
    /invoke\("add_learned_word",\s*\{[\s\S]*word:\s*entry\.word,[\s\S]*source:\s*entry\.source,[\s\S]*category/,
  );

  assert.match(backendSource, /upsert_entry_with_category/);
  assert.match(backendSource, /category\.as_deref\(\)/);
});
