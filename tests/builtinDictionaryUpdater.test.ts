import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("builtin updater 应在 setup 阶段初始化而非 start_app", async () => {
  const libSource = await readSource("src-tauri/src/lib.rs");
  const lifecycleSource = await readSource("src-tauri/src/commands/lifecycle.rs");
  assert.match(libSource, /setup\(move \|app\|/);
  assert.match(libSource, /builtin_hotwords_raw/);
  const startAppBlock = lifecycleSource.match(
    /#\[tauri::command\][\s\S]*?pub async fn start_app[\s\S]*?#\[tauri::command\][\s\S]*?pub async fn stop_app/
  );
  assert.ok(startAppBlock, "应能匹配到 start_app 函数体");
  assert.doesNotMatch(
    startAppBlock[0],
    /builtin_dictionary|fetch_remote_hotwords|start_builtin_dictionary_updater|builtin_hotwords_raw/
  );
});

test("应注册 get_builtin_domains_raw 命令与 builtin_dictionary_updated 事件", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  const traySource = await readSource("src-tauri/src/tray.rs");
  const configCommands = await readSource("src-tauri/src/commands/config.rs");
  assert.match(configCommands, /fn get_builtin_domains_raw\(/);
  assert.match(source, /generate_handler!\[[\s\S]*commands::config::get_builtin_domains_raw/);
  assert.match(traySource, /emit\("builtin_dictionary_updated"/);
});
