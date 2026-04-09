import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { ASR_CACHE_STORAGE_KEY, FALLBACK_ASR_PROVIDER, HISTORY_KEY, USAGE_STATS_KEY } from "../src/constants";
import type { AsrConfig } from "../src/types";
import { isAsrConfigValid, normalizeAsrConfigWithFallback } from "../src/utils";

const readSource = (path: string) => readFile(path, "utf8");

const createAsrConfig = (patch: Partial<AsrConfig> = {}): AsrConfig => ({
  credentials: {
    qwen_api_key: "",
    sensevoice_api_key: "",
    doubao_app_id: "",
    doubao_access_token: "",
    doubao_ime_device_id: "",
    doubao_ime_token: "",
    doubao_ime_cdid: "",
  },
  selection: {
    active_provider: "omni",
    enable_fallback: false,
    fallback_provider: null,
  },
  language_mode: "auto",
  omni_shared_config: {
    custom_rules: "",
    include_builtin_dictionary: true,
    include_focused_window_screenshot: true,
    debug_save_focused_window_screenshot: false,
    enable_thinking: false,
  },
  omni: {
    api_key: "",
    model: "LongCat-Flash-Omni-2603",
    active_profile_key: "https://api.longcat.chat/openai/v1/chat/completions",
    endpoint: "https://api.longcat.chat/openai/v1/chat/completions",
    endpoint_profiles: {},
    skip_post_processing: true,
    force_stream: false,
    thinking_supported: false,
  },
  ...patch,
});

test("Omni-only: fallback provider 与存储 key 应切换到 Omni 独立身份", () => {
  assert.equal(FALLBACK_ASR_PROVIDER, "omni");
  assert.equal(HISTORY_KEY, "pushtotalk_omni_history");
  assert.equal(USAGE_STATS_KEY, "pushtotalk_omni_usage_stats_v1");
  assert.equal(ASR_CACHE_STORAGE_KEY, "pushtotalk_omni_asr_cache");
});

test("Omni-only: isAsrConfigValid 仅校验 omni endpoint 与 api key", () => {
  const invalid = createAsrConfig();
  const valid = createAsrConfig({
    omni: {
      ...createAsrConfig().omni!,
      api_key: "sk-omni",
    },
  });

  assert.equal(isAsrConfigValid(invalid), false);
  assert.equal(isAsrConfigValid(valid), true);
});

test("Omni-only: normalizeAsrConfigWithFallback 应强制收敛到 omni provider", () => {
  const normalized = normalizeAsrConfigWithFallback(createAsrConfig({
    selection: {
      active_provider: "grok",
      enable_fallback: true,
      fallback_provider: "siliconflow",
    },
    omni: {
      ...createAsrConfig().omni!,
      api_key: "sk-omni",
    },
  }));

  assert.equal(normalized.didFallback, true);
  assert.equal(normalized.config.selection.active_provider, "omni");
  assert.equal(normalized.config.selection.enable_fallback, false);
  assert.equal(normalized.config.selection.fallback_provider, null);
});

test("Omni-only: 新配置与统计目录应切到 PushToTalkOmni", async () => {
  const configSource = await readSource("src-tauri/src/config.rs");
  const statsSource = await readSource("src-tauri/src/usage_stats.rs");
  const builtinSource = await readSource("src-tauri/src/builtin_dictionary_updater.rs");

  assert.match(configSource, /config_dir\.join\("PushToTalkOmni"\)/);
  assert.match(statsSource, /config_dir\.join\("PushToTalkOmni"\)/);
  assert.match(builtinSource, /config_dir\.join\("PushToTalkOmni"\)/);
});
