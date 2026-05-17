import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

function commandBlock(source: string, name: string): string {
  const start = source.indexOf(`async fn ${name}`);
  assert.notEqual(start, -1, `missing command ${name}`);

  const nextCommand = source.indexOf("#[tauri::command]", start + 1);
  return source.slice(start, nextCommand === -1 ? undefined : nextCommand);
}

function functionBlock(source: string, name: string): string {
  const start = source.indexOf(`fn ${name}`);
  assert.notEqual(start, -1, `missing function ${name}`);

  const braceStart = source.indexOf("{", start);
  assert.notEqual(braceStart, -1, `missing function body ${name}`);

  let depth = 0;
  for (let index = braceStart; index < source.length; index += 1) {
    const char = source[index];
    if (char === "{") depth += 1;
    if (char === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, index + 1);
    }
  }

  throw new Error(`unterminated function ${name}`);
}

test("词库管理命令应以 user_terms sidecar 为主要持久化源", async () => {
  const backendSource = await readFile("src-tauri/src/lib.rs", "utf8");

  const addBlock = commandBlock(backendSource, "add_learned_word");
  assert.match(addBlock, /upsert_user_term_sidecar_entry_and_snapshot_config\(/);
  assert.doesNotMatch(addBlock, /upsert_entry_with_inferred_category\(\s*&mut config\.dictionary/);

  const getBlock = commandBlock(backendSource, "get_dictionary_entries");
  assert.match(getBlock, /dictionary_entries_from_user_terms_or_config\(/);

  const deleteBlock = commandBlock(backendSource, "delete_dictionary_entries");
  assert.match(deleteBlock, /delete_user_term_sidecar_entries_and_snapshot_config\(/);
  assert.doesNotMatch(deleteBlock, /remove_entries\(\s*&mut config\.dictionary/);
});

test("普通配置保存不应把 AppConfig.dictionary 快照默认同步回 sidecar", async () => {
  const backendSource = await readFile("src-tauri/src/lib.rs", "utf8");

  const saveHelperBlock = functionBlock(backendSource, "save_persisted_config_without_emit");
  assert.match(saveHelperBlock, /config\.save\(\)/);
  assert.doesNotMatch(saveHelperBlock, /sync_user_terms_sidecar_from_dictionary_or_warn/);

  const saveConfigBlock = commandBlock(backendSource, "save_config");
  assert.match(saveConfigBlock, /let should_sync_user_terms_sidecar = dictionary\.is_some\(\);/);
  assert.match(
    saveConfigBlock,
    /if should_sync_user_terms_sidecar \{[\s\S]*sync_user_terms_sidecar_from_dictionary_or_warn\(&config\.dictionary,/,
  );
});

test("前端初始化词库应优先读取 sidecar 命令，失败才回退 config.dictionary", async () => {
  const source = await readFile("src/hooks/useAppServiceController.ts", "utf8");
  const loadStart = source.indexOf("const loadConfig = useCallback");
  const loadEnd = source.indexOf("const handleSaveConfig", loadStart);
  assert.ok(loadStart >= 0 && loadEnd > loadStart, "应找到 loadConfig 代码块");
  const loadBlock = source.slice(loadStart, loadEnd);

  assert.match(loadBlock, /invoke<string\[\]>\("get_dictionary_entries"\)/);
  assert.match(loadBlock, /catch\s*\(.*\)\s*\{[\s\S]*config\.dictionary/);
  assert.match(loadBlock, /let dictionarySource/);
  assert.match(loadBlock, /setDictionary\(loadedDictionary\)/);
});
