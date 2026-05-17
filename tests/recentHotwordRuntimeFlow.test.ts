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
  const saveGatewayStart = source.indexOf("const saveConfigThroughGateway = useCallback");
  const saveGatewayEnd = source.indexOf("const patchConfigFields", saveGatewayStart);
  assert.ok(saveGatewayStart >= 0 && saveGatewayEnd > saveGatewayStart, "应找到保存网关代码块");
  const saveConfigBlock = source.slice(saveGatewayStart, saveGatewayEnd);
  const payloadBlock = saveConfigBlock.match(
    /const saveConfigPayload[\s\S]*?theme:\s*resolved\.theme,\s*\};/,
  );

  assert.match(source, /recentHotwordEntries:\s*string\[\]/);
  assert.match(source, /buildRuntimeDictionary\([\s\S]*recentHotwordEntries/);
  assert.match(source, /for \(const entry of recentHotwordEntries\)/);
  assert.ok(payloadBlock, "应找到 save_config payload 构造块");
  assert.doesNotMatch(payloadBlock[0], /dictionary:/);
  assert.match(saveConfigBlock, /overrides\.dictionaryEntries\s*!==\s*undefined/);
  assert.match(saveConfigBlock, /overrides\.storageDictionary\s*!==\s*undefined/);
  assert.match(
    saveConfigBlock,
    /if\s*\(shouldPersistDictionary\)\s*\{[\s\S]*saveConfigPayload\.dictionary\s*=\s*resolved\.storageDictionary/,
  );
  assert.doesNotMatch(saveConfigBlock, /dictionary:\s*resolved\.runtimeDictionary/);
});
