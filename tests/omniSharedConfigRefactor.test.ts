import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("Omni shared config: 前端类型与默认值应声明公共配置对象", async () => {
  const typesSource = await readSource("src/types/index.ts");
  const constantsSource = await readSource("src/constants/index.ts");

  assert.match(typesSource, /export interface OmniSharedConfig/);
  assert.match(typesSource, /omni_shared_config:\s*OmniSharedConfig;/);
  assert.match(constantsSource, /export const DEFAULT_OMNI_SHARED_CONFIG/);
});

test("Omni shared config: ASR 页面不应再把共享项缓存到服务商 profile", async () => {
  const source = await readSource("src/pages/AsrPage.tsx");

  assert.match(source, /omni_shared_config/);
  assert.match(source, /公共转录配置/);
  assert.match(source, /服务商连接配置/);
  assert.match(source, /这些设置不会跟随服务商预设切换/);
  assert.match(source, /checked=\{omniSharedConfig\.enable_thinking\}/);
  assert.match(source, /checked=\{omniSharedConfig\.include_builtin_dictionary\}/);
  assert.match(source, /value=\{omniSharedConfig\.custom_rules\}/);
  assert.doesNotMatch(source, /custom_rules:\s*cur\.custom_rules/);
  assert.doesNotMatch(source, /include_builtin_dictionary:\s*cur\.include_builtin_dictionary/);
  assert.doesNotMatch(source, /enable_thinking:\s*cur\.enable_thinking/);
});

test("Omni shared config: 右侧面板应通过公共配置切换内置词库", async () => {
  const source = await readSource("src/components/layout/RightPanel.tsx");

  assert.match(source, /omni_shared_config/);
  assert.match(source, /公共转录配置/);
  assert.match(source, /作用于所有 Omni 服务商预设/);
  assert.match(source, /checked=\{omniSharedConfig\.include_builtin_dictionary\}/);
  assert.doesNotMatch(source, /omni:\s*\{[\s\S]*include_builtin_dictionary:\s*checked/);
});

test("Omni shared config: 后端配置与运行时更新应读取公共配置对象", async () => {
  const configSource = await readSource("src-tauri/src/config.rs");
  const libSource = await readSource("src-tauri/src/lib.rs");

  assert.match(configSource, /pub struct OmniSharedConfig/);
  assert.match(configSource, /pub omni_shared_config:\s*OmniSharedConfig/);
  assert.match(libSource, /omni_shared_config/);
  assert.match(libSource, /cfg\.omni_shared_config|updated_config\.asr_config\.omni_shared_config/);
});
