import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("App 应把历史记录转换为 recent hotwords 并参与运行时刷新 hash", async () => {
  const source = await readFile("src/App.tsx", "utf8");

  assert.match(source, /buildRecentHotwordEntries/);
  assert.match(source, /recentHotwordEntries\s*=\s*useMemo\(/);
  assert.match(source, /buildRecentHotwordEntries\(history\)/);
  assert.match(source, /const configHash = JSON\.stringify\(\{[\s\S]*recentHotwordEntries,[\s\S]*builtinDictionaryDomains,/);
  assert.match(source, /\[status,[\s\S]*recentHotwordEntries,[\s\S]*applyRuntimeConfig\]/);
});

test("useAppServiceController 应把 recent hotwords 合并进 runtime dictionary 且不写入持久化 dictionary", async () => {
  const source = await readFile("src/hooks/useAppServiceController.ts", "utf8");
  const saveConfigBlock = source.match(/await invoke<string>\("save_config", \{[\s\S]*?\n      \}\);/);

  assert.match(source, /recentHotwordEntries:\s*string\[\]/);
  assert.match(source, /buildRuntimeDictionary\([\s\S]*recentHotwordEntries/);
  assert.match(source, /for \(const entry of recentHotwordEntries\)/);
  assert.ok(saveConfigBlock, "应找到 save_config 调用块");
  assert.match(saveConfigBlock[0], /dictionary:\s*resolved\.storageDictionary/);
  assert.doesNotMatch(saveConfigBlock[0], /dictionary:\s*resolved\.runtimeDictionary/);
});
