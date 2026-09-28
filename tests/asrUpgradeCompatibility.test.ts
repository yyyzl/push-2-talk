import assert from "node:assert/strict";
import test from "node:test";
import { normalizeTnlConfig } from "../src/constants";

test("旧配置升级不得自动开启新引入的口语清洗", () => {
  assert.equal(normalizeTnlConfig({ enabled: true }).disfluency_mode, "off");
  assert.equal(normalizeTnlConfig({ enabled: true, disfluency_mode: "aggressive" }).disfluency_mode, "aggressive");
});

import { QWEN_MODELS, selectedQwenModel, qwenModelOptions, withQwenModel } from "../src/utils/qwenModels";
import { normalizeAsrConfigWithFallback } from "../src/utils/hotkey";
import type { AsrConfig } from "../src/types";

const legacy = {
  credentials: { qwen_api_key: "fixture-key" },
  selection: { active_provider: "qwen", enable_fallback: false, fallback_provider: null },
  language_mode: "zh",
} as AsrConfig;

test("旧配置保留原模型，每种模式的选择独立保存", () => {
  assert.equal(selectedQwenModel(legacy, "http"), "qwen3-asr-flash");
  assert.equal(selectedQwenModel(legacy, "realtime"), "qwen3-asr-flash-realtime");
  for (const model of QWEN_MODELS) {
    const changed = withQwenModel(legacy, model.mode, model.id);
    const restored = normalizeAsrConfigWithFallback(JSON.parse(JSON.stringify(changed))).config;
    assert.equal(selectedQwenModel(restored, model.mode), model.id);
    const other = model.mode === "http" ? "realtime" : "http";
    assert.equal(selectedQwenModel(restored, other), selectedQwenModel(legacy, other));
    assert.deepEqual(restored.credentials, legacy.credentials);
    assert.deepEqual(restored.selection, legacy.selection);
  }
});

test("未知模型保留可见，不悄悄改成默认模型或切换服务商", () => {
  const config = withQwenModel(legacy, "http", "future-saved-model");
  assert.equal(qwenModelOptions(config, "http")[0].value, "future-saved-model");
  assert.equal(normalizeAsrConfigWithFallback(config).didFallback, false);
  assert.equal(selectedQwenModel(config, "http"), "future-saved-model");
  assert.ok(qwenModelOptions(config, "realtime").every(option => option.value !== "future-saved-model"));
});

import { normalizeLoadedAssistant, normalizeLoadedLlm } from "../src/utils/loadedConfig";
import { DEFAULT_ASSISTANT_CONFIG, DEFAULT_LLM_CONFIG } from "../src/constants";

test("升级保留用户清空的预设列表，补全一个提示词不能覆盖其他助手设置", () => {
  const llm = { ...DEFAULT_LLM_CONFIG, presets: [], active_preset_id: "" };
  assert.deepEqual(normalizeLoadedLlm(llm).presets, []);
  const assistant = { ...DEFAULT_ASSISTANT_CONFIG, qa_system_prompt: "", text_processing_system_prompt: "My exact prompt", enabled: true, web_search_max_loops: 1 };
  const loaded = normalizeLoadedAssistant(assistant);
  assert.equal(loaded.text_processing_system_prompt, "My exact prompt");
  assert.equal(loaded.enabled, true);
  assert.equal(loaded.web_search_max_loops, 1);
});

test("加载旧配置不得恢复用户主动清空的助手提示词", () => {
  const assistant = { ...DEFAULT_ASSISTANT_CONFIG, qa_system_prompt: "", text_processing_system_prompt: "" };
  assert.deepEqual(normalizeLoadedAssistant(assistant), assistant);
});

test("旧配置中缺失的新纠错开关保持关闭，包括整个 TNL 段缺失", () => {
  for (const tnl of [undefined, null, {}, { enabled: true }, { enabled: false }]) {
    const loaded = normalizeTnlConfig(tnl);
    assert.equal(loaded.disfluency_mode, "off");
    assert.equal(loaded.enable_personalization_exact_text_pass, false);
    assert.equal(loaded.enable_personalization_syllable_match_pass, false);
    assert.equal(loaded.enable_personalization_hotwords, false);
    assert.equal(loaded.enable_context_hotwords, false);
    assert.equal(normalizeTnlConfig(JSON.parse(JSON.stringify(loaded))).enable_personalization_exact_text_pass, false);
  }
});

test("用户明确开启的新功能不被兼容处理关闭", () => {
  const loaded = normalizeTnlConfig({ enabled: true, disfluency_mode: "aggressive", enable_personalization_exact_text_pass: true, enable_personalization_syllable_match_pass: false });
  assert.equal(loaded.disfluency_mode, "aggressive");
  assert.equal(loaded.enable_personalization_exact_text_pass, true);
  assert.equal(loaded.enable_personalization_syllable_match_pass, false);
});
