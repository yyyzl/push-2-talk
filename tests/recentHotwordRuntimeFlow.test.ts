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
