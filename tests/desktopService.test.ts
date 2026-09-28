import assert from "node:assert/strict";
import test from "node:test";
import { createDesktop, type StartApp } from "../src/services/desktop";
import { initialConfig } from "../src/state/appConfig";
import { parseEntry, entriesToStorageFormat } from "../src/utils/dictionaryUtils";
import { buildRuntimeDictionary } from "../src/utils/runtimeDictionary";

test("配置 IPC 保存局部 patch，只有词库导入才携带 dictionary", async () => {
  const calls: unknown[] = [];
  const api = createDesktop(async <T>(command: string, args?: unknown): Promise<T> => {
    calls.push([command, args]);
    return { revision: 2, config: initialConfig() } as T;
  });
  await api.updateConfig({ tnl_config: { disfluency_mode: "aggressive" } });
  await api.saveDictionary([]);
  assert.deepEqual(calls, [
    ["update_config", { patch: { tnl_config: { disfluency_mode: "aggressive" } } }],
    ["save_config", { apiKey: "", fallbackApiKey: "", dictionary: [] }],
  ]);
});

test("服务启停按顺序执行，失败不能阻塞后续恢复", async () => {
  const calls: string[] = [];
  let fail!: (error: Error) => void;
  const waiting = new Promise<void>((_, reject) => { fail = reject; });
  const api = createDesktop(async <T>(command: string): Promise<T> => {
    calls.push(command);
    if (command === "start_app") await waiting;
    return "ok" as T;
  });
  const starting = api.start({} as StartApp);
  const stopping = api.stop();
  const quitting = api.quit();
  await Promise.resolve();
  assert.deepEqual(calls, ["start_app"]);
  fail(new Error("unavailable device"));
  await assert.rejects(starting);
  await stopping; await quitting;
  assert.deepEqual(calls, ["start_app", "stop_app", "quit_app"]);
});

test("运行时热词合并不污染持久化词库，手动词条优先并去重", () => {
  const entries = [parseEntry("TypeScript")];
  const stored = entriesToStorageFormat(entries);
  const runtime = buildRuntimeDictionary(entries, [], ["typescript|recent|code_symbol", "OpenAI|recent|generic"]);
  assert.equal(runtime.filter(value => value.toLowerCase().startsWith("typescript")).length, 1);
  assert.ok(runtime.includes("OpenAI|recent|generic"));
  assert.deepEqual(entriesToStorageFormat(entries), stored);
  assert.ok(stored.every(value => !value.includes("OpenAI")));
});
