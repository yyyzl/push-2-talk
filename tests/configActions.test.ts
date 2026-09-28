import assert from "node:assert/strict";
import test from "node:test";
import { initialConfig } from "../src/state/appConfig";
import { ConfigStore } from "../src/state/configStore";
import { loadConfiguration, saveConfiguration } from "../src/state/configActions";
import type { AppConfig } from "../src/types";
import type { ConfigPatch } from "../src/services/desktop";

function setup() {
  let config = initialConfig();
  config.asr_config.credentials.qwen_api_key = "fixture-only";
  config.asr_config.selection.active_provider = "qwen";
  config.learning_config.enabled = false;
  config.dictionary = ["旧快照"];
  let revision = 0;
  const writes: ConfigPatch[] = [];
  const dictionaryWrites: string[][] = [];
  const store = new ConfigStore(initialConfig());
  const port = {
    getConfig: async () => ({ revision, config: structuredClone(config) }),
    getDictionary: async () => ["数据库词条"],
    updateConfig: async (patch: ConfigPatch) => {
      writes.push(patch);
      // The application actions send only leaf changes; simulate the repository merge.
      const merge = (base: any, change: any): any => Object.fromEntries(Object.entries({ ...base, ...change }).map(([key, value]) => [key,
        change[key] && typeof change[key] === "object" && !Array.isArray(change[key]) ? merge(base[key], change[key]) : value]));
      config = merge(config, patch) as AppConfig;
      return { revision: ++revision, config: structuredClone(config) };
    },
    saveDictionary: async (entries: string[]) => { dictionaryWrites.push(entries); config.dictionary = entries; revision++; return "ok"; },
  };
  return { store, port, config, writes, dictionaryWrites };
}

test("加载有效线上配置不发生写入，启动使用数据库词库", async () => {
  const { store, port, config, writes, dictionaryWrites } = setup();
  const loaded = await loadConfiguration(store, port);
  assert.equal(loaded.didFallback, false);
  assert.equal(loaded.config.asr_config.credentials.qwen_api_key, config.asr_config.credentials.qwen_api_key);
  assert.deepEqual(loaded.dictionary.map(entry => entry.word), ["数据库词条"]);
  assert.deepEqual(writes, []);
  assert.deepEqual(dictionaryWrites, []);
  assert.equal(store.getSnapshot().dirty, false);
});

test("缺凭据的启动修复只改 ASR 选择，保留学习、模型、热键和词库", async () => {
  const { store, port, config, writes, dictionaryWrites } = setup();
  config.asr_config.credentials.qwen_api_key = "";
  const before = structuredClone(config);
  const loaded = await loadConfiguration(store, port);
  assert.equal(loaded.didFallback, true);
  assert.deepEqual(writes, [{ asr_config: { selection: loaded.config.asr_config.selection } }]);
  for (const key of ["learning_config", "llm_config", "dual_hotkey_config", "dictionary"] as const) {
    assert.deepEqual(loaded.config[key], before[key]);
  }
  assert.deepEqual(dictionaryWrites, []);
});

test("词库读取失败回退已有快照；空数据库不能复活已删除词条", async () => {
  const { store, port } = setup();
  port.getDictionary = async () => { throw new Error("unavailable"); };
  assert.deepEqual((await loadConfiguration(store, port)).dictionary.map(entry => entry.word), ["旧快照"]);
  port.getDictionary = async () => [];
  assert.deepEqual((await loadConfiguration(store, port)).dictionary, []);
});

test("即时保存仅写变动字段，后台更新的凭据及未改动设置保留", async () => {
  const { store, port, config, writes, dictionaryWrites } = setup();
  await loadConfiguration(store, port);
  const rendered = store.getSnapshot().config;
  config.asr_config.credentials.qwen_api_key = "refreshed-fixture";
  store.receive(await port.getConfig());
  const saved = await saveConfiguration(store, port, { asrConfig: { ...rendered.asr_config, language_mode: "zh" } }, rendered);
  assert.deepEqual(writes, [{ asr_config: { language_mode: "zh" } }]);
  assert.equal(saved.asr_config.credentials.qwen_api_key, "refreshed-fixture");
  assert.equal(saved.learning_config.enabled, false);
  assert.deepEqual(dictionaryWrites, []);
});

test("只有显式批量词库操作才写入词库，空列表可清空", async () => {
  const { store, port, dictionaryWrites } = setup();
  await loadConfiguration(store, port);
  const saved = await saveConfiguration(store, port, { dictionaryEntries: [] });
  assert.deepEqual(dictionaryWrites, [[]]);
  assert.deepEqual(saved.dictionary, []);
});

test("加载失败不允许保存默认值", async () => {
  const { store, port, writes } = setup();
  port.getConfig = async () => { throw new Error("invalid config"); };
  await assert.rejects(loadConfiguration(store, port));
  await assert.rejects(saveConfiguration(store, port, { theme: "dark" }), /尚未加载/);
  assert.deepEqual(writes, []);
});
