import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("C1: 网关应优先从 asrConfig.credentials 同步顶层 key", async () => {
  const source = await readSource("src/hooks/useAppServiceController.ts");

  assert.match(source, /const finalAsrConfig = overrides\.asrConfig \?\? asrConfig;/);
  assert.match(
    source,
    /apiKey:\s*finalAsrConfig\.credentials\.qwen_api_key\s*\|\|\s*overrides\.apiKey\s*\|\|\s*apiKey/,
  );
  assert.match(
    source,
    /fallbackApiKey:\s*finalAsrConfig\.credentials\.sensevoice_api_key\s*\|\|\s*overrides\.fallbackApiKey\s*\|\|\s*fallbackApiKey/,
  );
});

test("M1: learningConfig 应在 resolveSaveConfig 中状态兜底", async () => {
  const source = await readSource("src/hooks/useAppServiceController.ts");

  assert.match(source, /const finalLearningConfig = normalizeLearningConfig\(/);
  assert.match(source, /overrides\.learningConfig \?\? learningConfig/);
  assert.match(source, /learningConfig:\s*finalLearningConfig/);
});

test("P0: App 初始化 effect 应有 hasLoadedConfigRef 守卫避免重复初始化", async () => {
  const source = await readSource("src/App.tsx");

  assert.match(source, /useEffect\(\(\)\s*=>\s*\{\s*if\s*\(hasLoadedConfigRef\.current\)\s*return;/);
});

test("P1-D: 迁移保存应显式传入 learningConfig，避免默认值覆盖", async () => {
  const source = await readSource("src/hooks/useAppServiceController.ts");

  assert.match(
    source,
    /learningConfig:\s*config\.learning_config\s*\|\|\s*DEFAULT_LEARNING_CONFIG/,
  );
});

test("P1-A: saveFieldPatchWithStatus 应主动开启同步窗口并在 finally 释放", async () => {
  const source = await readSource("src/App.tsx");

  assert.match(
    source,
    /const saveFieldPatchWithStatus[\s\S]*cancelAutoSaveDebounce\(\);[\s\S]*const syncToken = configSyncWindowControllerRef\.current\.begin\("external_config_updated"\);/,
  );
  assert.match(
    source,
    /const saveFieldPatchWithStatus[\s\S]*finally\s*\{[\s\S]*releaseConfigSyncWindow\(syncToken\);/,
  );
});

test("P1-B: load_config 命令应持有 CONFIG_LOCK，避免与 save rename 竞态", async () => {
  const source = await readSource("src-tauri/src/commands/config.rs");

  assert.match(
    source,
    /async fn load_config\(\) -> Result<AppConfig, String> \{[\s\S]*let _guard = CONFIG_LOCK[\s\S]*\.lock\(\)/,
  );
});

test("P2-B: save_config 未传 hotkey_config 时应保留旧值", async () => {
  const source = await readSource("src-tauri/src/commands/config.rs");

  assert.match(
    source,
    /hotkey_config:\s*hotkey_config\.or_else\(\|\|\s*existing\.hotkey_config\.clone\(\)\)/,
  );
});

test("P2-C: patch_config_fields 应对白名单 theme/close_action 做校验", async () => {
  const source = await readSource("src-tauri/src/commands/config.rs");

  assert.match(
    source,
    /if matches!\(theme,\s*"light"\s*\|\s*"dark"\) \{/,
  );
  assert.match(
    source,
    /if let Some\(close_action_patch\) = patch\.close_action \{[\s\S]*if matches!\(action,\s*"close"\s*\|\s*"minimize"\)/,
  );
});

test("M2: PreferencesPage 不应再在切换学习开关时 load_config", async () => {
  const source = await readSource("src/pages/PreferencesPage.tsx");

  assert.doesNotMatch(source, /const\s+config\s*=\s*await\s+invoke<\{\s*learning_config:/);
  assert.doesNotMatch(source, /\.\.\.config\.learning_config/);
  assert.match(source, /\.\.\.learningConfig/);
});

test("S3: 托盘配置切换应拆分磁盘保存与事件派发，避免长时间持锁", async () => {
  const source = await readSource("src-tauri/src/tray.rs");

  assert.match(source, /fn\s+save_persisted_config_without_emit\s*\(/);
  assert.match(source, /save_persisted_config_without_emit\(&config\)\?;/);
  assert.match(source, /emit_config_updated\(app_handle,\s*&updated_config\);/);
});

test("m1: 热键录制 handleKeyUp 应仅 stopPropagation", async () => {
  const source = await readSource("src/hooks/useHotkeyRecording.ts");

  assert.match(source, /const\s+handleKeyUp\s*=\s*\(e:\s*KeyboardEvent\)\s*=>/);
  assert.match(source, /handleKeyUp[\s\S]*e\.stopPropagation\(\);/);
  assert.doesNotMatch(source, /handleKeyUp[\s\S]*e\.preventDefault\(\);/);
});

test("m2: 顶部全局提示条应使用高度过渡避免布局抖动", async () => {
  const source = await readSource("src/components/layout/TopStatusBar.tsx");

  assert.match(source, /overflow-hidden transition-all duration-200/);
  assert.match(source, /globalNotice\s*\?\s*\"max-h-10 opacity-100\"\s*:\s*\"max-h-0 opacity-0\"/);
});

test("m3: CONFIG_LOCK 应仅在 lib.rs 顶部统一导入", async () => {
  const source = await readSource("src-tauri/src/lib.rs");

  assert.match(source, /use\s+config::\{\s*AppConfig\s*,\s*CONFIG_LOCK\s*\};/);
  assert.doesNotMatch(source, /use\s+crate::config::CONFIG_LOCK\s*;/);
});

test("S5: 即时保存 overrides 命名应统一为 dictionaryEntries", async () => {
  const contextSource = await readSource("src/contexts/ConfigSaveContext.tsx");
  const controllerSource = await readSource("src/hooks/useAppServiceController.ts");

  assert.match(contextSource, /dictionaryEntries\?:\s*DictionaryEntry\[\];/);
  assert.doesNotMatch(contextSource, /dictionary\?:\s*DictionaryEntry\[\];/);

  assert.match(controllerSource, /dictionaryEntries\?:\s*DictionaryEntry\[\];/);
  assert.match(controllerSource, /dictionaryEntries:\s*overrides\?\.dictionaryEntries/);
  assert.doesNotMatch(controllerSource, /dictionaryEntries:\s*overrides\?\.dictionary\b/);
  assert.doesNotMatch(controllerSource, /if\s*\(overrides\?\.dictionary\b\)/);
});

test("S2: 后端应提供 set_learning_enabled 字段级 patch 命令", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  const configCommands = await readSource("src-tauri/src/commands/config.rs");
  const systemCommands = await readSource("src-tauri/src/commands/system.rs");

  assert.match(configCommands, /async\s+fn\s+patch_config_fields\s*\(\s*app:\s*AppHandle\s*,\s*patch:\s*ConfigFieldPatch\s*,?\s*\)/);
  assert.match(source, /invoke_handler\(tauri::generate_handler!\[[\s\S]*patch_config_fields,/);
  assert.match(
    systemCommands,
    /async\s+fn\s+set_learning_enabled\s*\(\s*app:\s*AppHandle\s*,\s*enabled:\s*bool\s*\)/,
  );
  assert.match(systemCommands, /patch_config_fields\(\s*app\s*,\s*ConfigFieldPatch\s*\{/);
  assert.match(systemCommands, /learning_enabled:\s*Some\(enabled\)/);
  assert.match(source, /commands::config::patch_config_fields\s*,/);
  assert.match(source, /commands::system::set_learning_enabled\s*,/);
});

test("S2: Preferences 学习开关应改为调用 set_learning_enabled", async () => {
  const source = await readSource("src/pages/PreferencesPage.tsx");

  assert.doesNotMatch(source, /invoke<string>\("set_learning_enabled",\s*\{\s*enabled:\s*newValue\s*\}\s*\)/);
  assert.match(source, /onSetLearningEnabled:\s*\(enabled:\s*boolean\)\s*=>\s*Promise<void>/);
  assert.match(source, /await\s+onSetLearningEnabled\(newValue\)/);
});

test("S2+: 后端配置写入应通过统一 mutate helper", async () => {
  const source = await readSource("src-tauri/src/tray.rs");

  assert.match(source, /fn\s+mutate_persisted_config_with_result<\s*R\s*,\s*F\s*>\s*\(/);
  assert.match(source, /fn\s+mutate_persisted_config<\s*F\s*>\s*\(/);
  assert.match(source, /save_persisted_config_without_emit\(&config\)\?;/);
  assert.match(source, /let\s+\(updated_config\s*,\s*new_value\)\s*=\s*mutate_persisted_config_with_result\(/);
});

test("S2+: 前端应通过 patch_config_fields 保存轻量字段", async () => {
  const controllerSource = await readSource("src/hooks/useAppServiceController.ts");
  const appSource = await readSource("src/App.tsx");

  assert.match(controllerSource, /const\s+patchConfigFields\s*=\s*useCallback\(/);
  assert.match(controllerSource, /invoke<string>\("patch_config_fields",\s*\{\s*patch\s*\}\)/);
  assert.match(controllerSource, /await\s+patchConfigFields\(\{\s*closeAction:\s*action\s*\}\)/);

  assert.match(appSource, /await\s+saveFieldPatchWithStatus\(\{\s*theme:\s*newTheme\s*\}\)/);
  assert.match(appSource, /onSetLearningEnabled=\{async\s*\(enabled\)\s*=>\s*\{/);
  assert.match(appSource, /onSetEnableMuteOtherApps=\{async\s*\(next\)\s*=>\s*\{/);
});

test("m4: global notice 相关 import 不应使用 .ts 后缀", async () => {
  const globalNoticeSource = await readSource("src/utils/globalNotice.ts");
  const packageJsonSource = await readSource("package.json");

  assert.doesNotMatch(globalNoticeSource, /from\s+"\.\/configSyncWindow\.ts"/);
  assert.match(packageJsonSource, /"test:ts"\s*:\s*"tsx --test tests\/\*\.test\.ts"/);
});
