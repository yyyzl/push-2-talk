import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

for (const [label, newline] of [["LF", "\n"], ["CRLF", "\r\n"]]) {
  test(`builtin updater 应在 setup 阶段初始化而非 start_app (${label})`, async () => {
    const source = (await readSource("src-tauri/src/lib.rs")).replace(/\r?\n/g, newline);
    assert.match(source, /setup\(move \|app\|/);
    assert.match(source, /builtin_hotwords_raw/);
    const startAppBlock = source.match(
      /async fn start_app[\s\S]*?\r?\n}\r?\n\r?\n#\[tauri::command\]\r?\nasync fn stop_app/
    );
    assert.ok(startAppBlock, "应能匹配到 start_app 函数体");
    assert.doesNotMatch(
      startAppBlock[0],
      /builtin_dictionary|fetch_remote_hotwords|start_builtin_dictionary_updater|builtin_hotwords_raw/
    );
  });
}

test("应注册 get_builtin_domains_raw 命令与 builtin_dictionary_updated 事件", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  assert.match(source, /fn get_builtin_domains_raw\(/);
  assert.match(source, /generate_handler!\[[\s\S]*get_builtin_domains_raw/);
  assert.match(source, /emit\("builtin_dictionary_updated"/);
});
