import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("loadConfig 应先加载内置词库 snapshot 再构建 runtime dictionary", async () => {
  const source = await readFile("src/hooks/useAppServiceController.ts", "utf8");
  assert.match(source, /fetchBuiltinDomains\(/);
  assert.match(source, /setBuiltinDomainsSnapshot\(/);
  assert.match(source, /buildRuntimeDictionary\(/);
  assert.match(source, /entriesToRuntimeFormat\(/);
  assert.match(source, /getBuiltinRuntimeEntriesForDomains\(/);
});
