import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { DEFAULT_ASR_CACHE, FALLBACK_ASR_PROVIDER } from "../src/constants";
import type { AsrConfig, AsrProvider } from "../src/types";
import { isAsrConfigValid, normalizeAsrConfigWithFallback } from "../src/utils";

const readSource = (path: string) => readFile(path, "utf8");
const escapeRegExp = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const DOUBAO_IME_MISSING_FALLBACK_ERROR =
  "豆包输入法实时 ASR 暂不可用，且未配置备用 ASR。请稍后重试或在 ASR 设置中配置备用服务";

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

test("loadConfig 回退持久化应携带完整配置快照但不重写词库 sidecar", async () => {
  const source = await readSource("src/hooks/useAppServiceController.ts");
  const marker = source.indexOf("// 回退后持久化修正后的配置，避免下次启动重复回退");

  assert.ok(marker >= 0, "未找到初始化回退持久化代码块");

  const block = source.slice(marker, marker + 1600);

  assert.match(block, /saveConfigThroughGateway\(\{/);
  assert.match(block, /llmConfig:/);
  assert.match(block, /assistantConfig:/);
  assert.match(block, /dualHotkeyConfig:/);
  assert.match(block, /learningConfig:/);
  assert.doesNotMatch(block, /dictionaryEntries:/);
  assert.doesNotMatch(block, /storageDictionary:/);
  assert.match(block, /builtinDictionaryDomains:/);
  assert.match(block, /theme:/);
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

test("DoubaoIme 实时失败且无备用 ASR 时应返回明确提示", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  const escapedError = escapeRegExp(DOUBAO_IME_MISSING_FALLBACK_ERROR);

  assert.match(
    source,
    new RegExp(`const\\s+DOUBAO_IME_MISSING_FALLBACK_ERROR:\\s*&str\\s*=\\s*"${escapedError}";`),
  );

  const assistantStart = source.indexOf("async fn handle_assistant_mode(");
  const assistantEnd = source.indexOf("// 3. 解包 ASR 结果", assistantStart);
  assert.ok(assistantStart >= 0 && assistantEnd > assistantStart, "未找到 AI 助手备用 ASR 代码块");
  const assistantFallbackBlock = source.slice(assistantStart, assistantEnd);

  assert.match(
    assistantFallbackBlock,
    /matches!\(active_prov,\s*Some\(config::AsrProvider::DoubaoIme\)\)\s*&&\s*fallback_prov\.is_none\(\)/,
  );
  assert.match(
    assistantFallbackBlock,
    /Err\(anyhow::anyhow!\(DOUBAO_IME_MISSING_FALLBACK_ERROR\)\)/,
  );

  const dictationStart = source.indexOf("async fn fallback_transcription(");
  const dictationEnd = source.indexOf("/// 统一的错误处理辅助函数", dictationStart);
  assert.ok(dictationStart >= 0 && dictationEnd > dictationStart, "未找到听写备用 ASR 代码块");
  const dictationFallbackBlock = source.slice(dictationStart, dictationEnd);

  assert.match(
    dictationFallbackBlock,
    /matches!\(active_prov,\s*Some\(config::AsrProvider::DoubaoIme\)\)\s*&&\s*fallback_prov\.is_none\(\)/,
  );
  assert.match(
    dictationFallbackBlock,
    /Err\(anyhow::anyhow!\(DOUBAO_IME_MISSING_FALLBACK_ERROR\)\)/,
  );
});

test("通用未配置 ASR 仍保留原错误语义", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  const transcribeStart = source.indexOf("async fn transcribe_with_available_clients(");
  const transcribeEnd = source.indexOf("/// HTTP 模式转录处理", transcribeStart);

  assert.ok(transcribeStart >= 0 && transcribeEnd > transcribeStart, "未找到 HTTP ASR 统一转录代码块");
  const transcribeBlock = source.slice(transcribeStart, transcribeEnd);

  assert.match(
    transcribeBlock,
    /None\s*=>\s*\{[\s\S]*Err\(anyhow::anyhow!\("ASR 提供商未配置"\)\)/,
  );
});
