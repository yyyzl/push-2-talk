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
  assert.equal(DEFAULT_QWEN_ASR_PROFILE, "qwen_audio_3");
  assert.equal(
    QWEN_ASR_PROFILES.qwen_audio_3.httpModel,
    "qwen-audio-3.0-asr-flash",
  );
  assert.equal(
    QWEN_ASR_PROFILES.qwen_audio_3.realtimeModel,
    "qwen-audio-3.0-asr-flash-streaming",
  );
});

test("缺少 profile 的旧前端配置应归一化到最新版", () => {
  const normalized = normalizeAsrConfigWithFallback(createLegacyRuntimeConfig());

  assert.equal(normalized.didFallback, false);
  assert.equal(normalized.config.qwen_profile, DEFAULT_QWEN_ASR_PROFILE);
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
