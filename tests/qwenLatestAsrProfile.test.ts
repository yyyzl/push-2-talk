import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_QWEN_ASR_PROFILE,
  QWEN_ASR_PROFILES,
} from "../src/constants";
import type { AsrConfig } from "../src/types";
import { normalizeAsrConfigWithFallback } from "../src/utils";

const createLegacyRuntimeConfig = (): AsrConfig => ({
  credentials: {
    qwen_api_key: "sk-valid",
    sensevoice_api_key: "",
    doubao_app_id: "",
    doubao_access_token: "",
    doubao_ime_device_id: "",
    doubao_ime_token: "",
    doubao_ime_cdid: "",
  },
  selection: {
    active_provider: "qwen",
    enable_fallback: false,
    fallback_provider: null,
  },
  language_mode: "auto",
} as AsrConfig);

test("千问最新版 ASR 应作为默认 profile", () => {
  assert.equal(DEFAULT_QWEN_ASR_PROFILE, "qwen_audio_3_1");
  assert.equal(QWEN_ASR_PROFILES[DEFAULT_QWEN_ASR_PROFILE].httpModel, "qwen-audio-3.1-asr-flash");
  assert.equal(QWEN_ASR_PROFILES[DEFAULT_QWEN_ASR_PROFILE].realtimeModel, "qwen-audio-3.1-asr-flash-streaming");
  assert.equal(
    QWEN_ASR_PROFILES.qwen_audio_3.httpModel,
    "qwen-audio-3.0-asr-flash",
  );
  assert.equal(
    QWEN_ASR_PROFILES.qwen_audio_3.realtimeModel,
    "qwen-audio-3.0-asr-flash-streaming",
  );
});

test("三种模型选择经过配置保存与恢复后不能被默认值覆盖", () => {
  const profiles = ["qwen_audio_3_1", "qwen_audio_3", "qwen3_legacy"];
  assert.deepEqual(Object.keys(QWEN_ASR_PROFILES), profiles);
  for (const profile of profiles) {
    const saved = JSON.stringify({ ...createLegacyRuntimeConfig(), qwen_profile: profile });
    const restored = normalizeAsrConfigWithFallback(JSON.parse(saved));
    assert.equal(restored.config.qwen_profile, profile);
    assert.equal(restored.didFallback, false);
  }
});

test("历史 qwen_audio3 别名保留 3.0，未知值回到默认值", () => {
  for (const [input, expected] of [
    ["qwen_audio3", "qwen_audio_3"],
    ["invalid", DEFAULT_QWEN_ASR_PROFILE],
    ["toString", DEFAULT_QWEN_ASR_PROFILE],
  ]) {
    const restored = normalizeAsrConfigWithFallback({
      ...createLegacyRuntimeConfig(), qwen_profile: input,
    } as AsrConfig);
    assert.equal(restored.config.qwen_profile, expected);
  }
});

test("缺少 profile 的旧前端配置应保留原 Qwen3 模型", () => {
  const normalized = normalizeAsrConfigWithFallback(createLegacyRuntimeConfig());

  assert.equal(normalized.didFallback, false);
  assert.equal(normalized.config.qwen_profile, "qwen3_legacy");
});

test("显式选择旧版时应保留兼容 profile", () => {
  const legacy = {
    ...createLegacyRuntimeConfig(),
    qwen_profile: "qwen3_legacy" as const,
  };

  const normalized = normalizeAsrConfigWithFallback(legacy);

  assert.equal(normalized.config.qwen_profile, "qwen3_legacy");
  assert.equal(QWEN_ASR_PROFILES.qwen3_legacy.httpModel, "qwen3-asr-flash");
  assert.equal(
    QWEN_ASR_PROFILES.qwen3_legacy.realtimeModel,
    "qwen3-asr-flash-realtime",
  );
});
