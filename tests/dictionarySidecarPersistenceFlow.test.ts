import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

function commandBlock(source: string, name: string): string {
  const start = source.indexOf(`async fn ${name}`);
  assert.notEqual(start, -1, `missing command ${name}`);

  const nextCommand = source.indexOf("#[tauri::command]", start + 1);
  return source.slice(start, nextCommand === -1 ? undefined : nextCommand);
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
