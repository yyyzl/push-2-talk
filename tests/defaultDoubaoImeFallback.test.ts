import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { DEFAULT_ASR_CACHE, FALLBACK_ASR_PROVIDER } from "../src/constants";
import type { AsrConfig, AsrProvider } from "../src/types";
import { isAsrConfigValid, normalizeAsrConfigWithFallback } from "../src/utils";

const readSource = (path: string) => readFile(path, "utf8");


const createAsrConfig = (
  activeProvider: AsrProvider,
  credentials: Partial<AsrConfig["credentials"]> = {},
): AsrConfig => ({
  credentials: {
    qwen_api_key: "",
    sensevoice_api_key: "",
    doubao_app_id: "",
    doubao_access_token: "",
    doubao_ime_device_id: "",
    doubao_ime_token: "",
    doubao_ime_cdid: "",
    ...credentials,
  },
  selection: {
    active_provider: activeProvider,
    enable_fallback: false,
    fallback_provider: null,
  },
  qwen_profile: "qwen_audio_3",
  language_mode: "auto",
});

test("Rust 默认 ASR Provider 应为 DoubaoIme", async () => {
  const source = await readSource("src-tauri/src/config.rs");

  assert.match(
    source,
    /impl Default for AsrProvider \{[\s\S]*AsrProvider::DoubaoIme/,
  );
  assert.match(
    source,
    /impl Default for AsrSelection \{[\s\S]*active_provider:\s*AsrProvider::DoubaoIme/,
  );
});

test("前端 fallback 常量与默认缓存应保持一致", () => {
  assert.equal(FALLBACK_ASR_PROVIDER, "doubao_ime");
  assert.equal(DEFAULT_ASR_CACHE.active_provider, FALLBACK_ASR_PROVIDER);
});

test("normalizeAsrConfigWithFallback: 有效配置不应回退", () => {
  const validQwenConfig = createAsrConfig("qwen", { qwen_api_key: "sk-valid" });
  const normalized = normalizeAsrConfigWithFallback(validQwenConfig);

  assert.equal(isAsrConfigValid(validQwenConfig), true);
  assert.equal(normalized.didFallback, false);
  assert.deepEqual(normalized.config, validQwenConfig);
});

test("normalizeAsrConfigWithFallback: qwen 缺 key 时应回退到 fallback", () => {
  const invalidQwenConfig = createAsrConfig("qwen");
  const normalized = normalizeAsrConfigWithFallback(invalidQwenConfig);

  assert.equal(isAsrConfigValid(invalidQwenConfig), false);
  assert.equal(normalized.didFallback, true);
  assert.equal(normalized.config.selection.active_provider, FALLBACK_ASR_PROVIDER);
  assert.equal(isAsrConfigValid(normalized.config), true);
});

test("normalizeAsrConfigWithFallback: doubao 缺凭据时应回退到 fallback", () => {
  const invalidDoubaoConfig = createAsrConfig("doubao", { doubao_app_id: "app-id-only" });
  const normalized = normalizeAsrConfigWithFallback(invalidDoubaoConfig);

  assert.equal(isAsrConfigValid(invalidDoubaoConfig), false);
  assert.equal(normalized.didFallback, true);
  assert.equal(normalized.config.selection.active_provider, FALLBACK_ASR_PROVIDER);
});

test("手动启停回退提示应走通知通道而非 error 通道", async () => {
  const source = await readSource("src/hooks/useAppServiceController.ts");
  const startStopIdx = source.indexOf("const handleStartStop = useCallback(async () => {");
  const endIdx = source.indexOf("const handleCancelTranscription = useCallback(async () => {");

  assert.ok(startStopIdx >= 0 && endIdx > startStopIdx, "未找到 handleStartStop 代码块");

  const block = source.slice(startStopIdx, endIdx);

  assert.doesNotMatch(block, /setError\(`ASR Key 缺失，已自动切换至\$\{fallbackName\}`\);/);
  assert.match(block, /showToast\?\.\(/);
});
