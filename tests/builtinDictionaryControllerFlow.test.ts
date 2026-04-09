import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("loadConfig 应先加载内置词库 snapshot 再构建 runtime dictionary", async () => {
  const source = await readFile("src/hooks/useAppServiceController.ts", "utf8");
  assert.match(source, /fetchBuiltinDomains\(/);
  assert.match(source, /setBuiltinDomainsSnapshot\(/);
  assert.match(source, /buildRuntimeDictionary\(/);
});

test("前端 builtinDictionary 工具不应再读取打包内置 hotwords.txt", async () => {
  const source = await readFile("src/utils/builtinDictionary.ts", "utf8");
  assert.doesNotMatch(source, /hotwords\.txt/);
  assert.doesNotMatch(source, /loadEmbeddedHotwordsRaw/);
});

test("运行时词库应仅包含个人词库，内置领域通过 Omni 开关注入", async () => {
  const source = await readFile("src/hooks/useAppServiceController.ts", "utf8");
  assert.match(source, /const buildRuntimeDictionary = \(\s*dictionaryEntries: DictionaryEntry\[\]/);
  assert.match(source, /return entriesToWords\(dictionaryEntries\);/);
  assert.doesNotMatch(source, /getBuiltinWordsForDomains/);
});
