import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("builtin updater 应在 setup 阶段初始化，并在 start_app 中把缓存注入 Omni 客户端", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  assert.match(source, /setup\(move \|app\|/);
  assert.match(source, /builtin_hotwords_raw/);
  const startAppBlock = source.match(
    /async fn start_app[\s\S]*?\n}\n\n#\[tauri::command\]\nasync fn stop_app/
  );
  assert.ok(startAppBlock, "应能匹配到 start_app 函数体");
  assert.doesNotMatch(startAppBlock[0], /fetch_remote_hotwords|start_builtin_dictionary_updater/);
  assert.match(startAppBlock[0], /builtin_hotwords_raw/);
  assert.match(startAppBlock[0], /OmniAsrClient::new/);
});

test("应注册 get_builtin_domains_raw 命令与 builtin_dictionary_updated 事件", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  assert.match(source, /fn get_builtin_domains_raw\(/);
  assert.match(source, /generate_handler!\[[\s\S]*get_builtin_domains_raw/);
  assert.match(source, /emit\("builtin_dictionary_updated"/);
});

test("builtin updater 不应再回退到仓库内置 hotwords.txt", async () => {
  const source = await readSource("src-tauri/src/builtin_dictionary_updater.rs");
  assert.doesNotMatch(source, /include_str!\("\.\.\/\.\.\/hotwords\.txt"\)/);
  assert.doesNotMatch(source, /EMBEDDED_HOTWORDS/);
});

test("Omni prompt 应按已选领域过滤远端词库，而不是注入整份 builtin_raw", async () => {
  const source = await readSource("src-tauri/src/asr/omni/prompt_builder.rs");
  assert.match(source, /selected_builtin_domains/);
  assert.match(source, /HashSet/);
  assert.doesNotMatch(source, /format_builtin_domains\(builtin_raw\);/);
});
