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
  const source = await readSource("src-tauri/src/lib.rs");

  assert.match(
    source,
    /async fn load_config\(\) -> Result<AppConfig, String> \{[\s\S]*let _guard = CONFIG_LOCK[\s\S]*\.lock\(\)/,
  );
});

test("P2-B: save_config 未传 hotkey_config 时应保留旧值", async () => {
  const source = await readSource("src-tauri/src/lib.rs");

  assert.match(
    source,
    /hotkey_config:\s*hotkey_config\.or_else\(\|\|\s*existing\.hotkey_config\.clone\(\)\)/,
  );
});

test("P2-C: patch_config_fields 应对白名单 theme/close_action 做校验", async () => {
  const source = await readSource("src-tauri/src/lib.rs");

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
  const source = await readSource("src-tauri/src/lib.rs");

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

  assert.match(source, /async\s+fn\s+patch_config_fields\s*\(\s*app:\s*AppHandle\s*,\s*patch:\s*ConfigFieldPatch\s*\)/);
  assert.match(source, /invoke_handler\(tauri::generate_handler!\[[\s\S]*patch_config_fields,/);
  assert.match(
    source,
    /async\s+fn\s+set_learning_enabled\s*\(\s*app:\s*AppHandle\s*,\s*enabled:\s*bool\s*\)/,
  );
  assert.match(source, /patch_config_fields\(\s*app\s*,\s*ConfigFieldPatch\s*\{/);
  assert.match(source, /learning_enabled:\s*Some\(enabled\)/);
  assert.match(source, /set_learning_enabled\s*,/);
});

test("S2: Preferences 学习开关应改为调用 set_learning_enabled", async () => {
  const source = await readSource("src/pages/PreferencesPage.tsx");

  assert.doesNotMatch(source, /invoke<string>\("set_learning_enabled",\s*\{\s*enabled:\s*newValue\s*\}\s*\)/);
  assert.match(source, /onSetLearningEnabled:\s*\(enabled:\s*boolean\)\s*=>\s*Promise<void>/);
  assert.match(source, /await\s+onSetLearningEnabled\(newValue\)/);
});

test("S2+: 后端配置写入应通过统一 mutate helper", async () => {
  const source = await readSource("src-tauri/src/lib.rs");

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

test("A1: AssistantPage 联网搜索 API 配置入口应始终可见", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /配置 API/);
  assert.match(source, /管理搜索引擎/);
  assert.match(source, /hasConfiguredSearchApi/);
  assert.match(source, /配置搜索服务商后再显示启用开关和高级参数/);
  assert.doesNotMatch(source, /\{assistantConfig\.enable_web_search\s*&&\s*\(/);
});

test("A2: AssistantPage 应展示自动保存状态", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /useConfigSave/);
  assert.match(source, /syncStatus/);
  assert.match(source, /function AutoSaveStatus/);
  assert.match(source, /if \(syncStatus === "idle"\) return null/);
  assert.match(source, /自动保存/);
  assert.match(source, /保存中/);
  assert.match(source, /保存失败/);
});

test("A3: debounce 自动保存应更新全局保存状态", async () => {
  const source = await readSource("src/App.tsx");

  assert.match(
    source,
    /autoSaveTimerRef\.current = window\.setTimeout\(\(\) => \{[\s\S]*setSyncStatus\("syncing"\)[\s\S]*await handleSaveConfigRef\.current\(\)[\s\S]*setSyncStatus\("success"\)/,
  );
  assert.match(
    source,
    /autoSaveTimerRef\.current = window\.setTimeout\(\(\) => \{[\s\S]*catch\s*\(err\)[\s\S]*setSyncStatus\("error"\)/,
  );
});

test("A4: AssistantPage 联网搜索高级项应配置后再展开", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /searchDetailsExpanded/);
  assert.match(source, /shouldShowSearchDetails/);
  assert.match(source, /setSearchDetailsExpanded/);
  assert.match(source, /Chevron(Down|Up|Right)/);
  assert.match(source, /aria-label=\{searchDetailsExpanded/);
  assert.doesNotMatch(source, /展开设置/);
  assert.doesNotMatch(source, /收起设置/);
  assert.match(source, /shouldShowSearchDetails\s*&&\s*\(/);
  assert.match(source, /hasConfiguredSearchApi\s*\?\s*\(/);
});

test("A5: AssistantPage 应将默认搜索引擎修正到可运行配置", async () => {
  const source = await readSource("src/utils/searchRuntime.ts");
  const pageSource = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /function isSearchProviderApiConfigured/);
  assert.match(source, /function isSearchProviderRuntimeUsable/);
  assert.match(source, /function resolveSearchDefaultProviderId/);
  assert.match(source, /providers\.find\(isSearchProviderRuntimeUsable\)/);
  assert.match(source, /const firstConfiguredProvider = providers\.find\(isSearchProviderApiConfigured\)/);
  assert.doesNotMatch(source, /return providers\[0\]\?\.id \?\? null/);
  assert.match(pageSource, /setSearchConfig\(\(prev\) => \{[\s\S]*default_provider_id: nextDefaultProviderId/);
  assert.match(pageSource, /label="可用引擎"/);
});

test("A6: AssistantPage 搜索默认引擎选择应启用已配置 provider 且拒绝未配置 provider", async () => {
  const source = await readSource("src/utils/searchRuntime.ts");
  const pageSource = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /function selectSearchDefaultProvider/);
  assert.match(
    source,
    /if \(!selectedProvider \|\| !isSearchProviderApiConfigured\(selectedProvider\)\) return config;/,
  );
  assert.match(source, /provider\.id === providerId && !provider\.enabled[\s\S]*enabled: true/);
  assert.match(
    pageSource,
    /onClick=\{\(\) =>\s*setSearchConfig\(\(prev\) =>\s*selectSearchDefaultProvider\(prev, selectedSearchProvider\.id\),\s*\)\s*\}/,
  );
  assert.match(pageSource, /isRunning \|\| !isSearchProviderApiConfigured\(selectedSearchProvider\)/);
});

test("A7: AssistantPage 搜索 API 抽屉应采用 provider 列表加详情布局", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /selectedSearchProviderId/);
  assert.match(source, /setSelectedSearchProviderId\(id\)/);
  assert.match(source, /selectedSearchProvider/);
  assert.match(source, /aria-label="搜索服务商列表"/);
  assert.match(source, /aria-label="搜索服务商详情"/);
  assert.match(source, /选择左侧服务商后编辑连接信息/);
  assert.doesNotMatch(
    source,
    /searchConfig\.providers\.map\(\(provider\)\s*=>\s*\(\s*<div[\s\S]{0,800}className="space-y-3 rounded-xl border/,
  );
});

test("A8: AssistantPage 搜索服务商 API Key 应复用可显隐输入框", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /import\s+\{\s*ApiKeyInput[\s\S]*LlmConnectionConfig[\s\S]*\}\s+from\s+"..\/components\/common"/);
  assert.match(source, /showSearchApiKey/);
  assert.match(source, /setShowSearchApiKey/);
  assert.match(
    source,
    /<ApiKeyInput[\s\S]*value=\{selectedSearchProvider\.api_key \?\? ""\}[\s\S]*onToggleShow=\{\(\) => setShowSearchApiKey\(\(prev\) => !prev\)\}/,
  );
  assert.doesNotMatch(source, /label="API Key"[\s\S]{0,260}type="password"/);
});

test("A9: AssistantPage 局部 ToggleSwitch 应使用 switch 语义且圆点不会越界", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");
  const toggleSource = source.match(/function ToggleSwitch[\s\S]*?function NumberField/)?.[0] ?? "";

  assert.match(toggleSource, /role="switch"/);
  assert.match(toggleSource, /aria-checked=\{checked\}/);
  assert.doesNotMatch(toggleSource, /aria-pressed/);
  assert.match(toggleSource, /"absolute top-0\.5 left-0\.5 h-5 w-5/);
  assert.match(toggleSource, /checked \? "translate-x-5" : "translate-x-0"/);
  assert.doesNotMatch(toggleSource, /translate-x-0\.5/);
});

test("A10: AssistantPage 搜索引擎管理应为居中 modal 而不是右侧 drawer", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /role="dialog"/);
  assert.match(source, /aria-modal="true"/);
  assert.match(source, /aria-labelledby="search-provider-modal-title"/);
  assert.match(source, /fixed inset-0 z-50 flex items-center justify-center/);
  assert.match(source, /max-h-\[calc\(100vh-2rem\)\]/);
  assert.match(source, /max-w-5xl/);
  assert.doesNotMatch(source, /fixed inset-0 z-50 flex justify-end/);
  assert.doesNotMatch(source, /border-l border-\[var\(--stone\)\] bg-white shadow-2xl/);
});

test("A11: AssistantPage 搜索 API 连接测试成功反馈应是绿色结构化状态", async () => {
  const source = await readSource("src/pages/AssistantPage.tsx");

  assert.match(source, /type SearchProviderTestState/);
  assert.match(source, /function SearchProviderTestFeedback/);
  assert.match(source, /status:\s*"success"/);
  assert.match(source, /连接正常/);
  assert.match(source, /返回 \{state\.resultCount\} 条结果/);
  assert.match(source, /bg-emerald-50 border-emerald-100 text-emerald-800/);
  assert.match(source, /role="status"/);
  assert.doesNotMatch(source, /setTestMessages/);
});

test("A12: 结果面板 pending 联网状态应要求搜索引擎运行可用", async () => {
  const source = await readSource("src-tauri/src/lib.rs");

  assert.match(source, /fn resolve_pending_web_search_enabled/);
  assert.match(
    source,
    /processor\.is_web_search_requested\(prompt_mode, preference, Some\(search_config\)\)[\s\S]*SearchRegistry::runtime_unavailable_reason\(search_config\)\.is_none\(\)/,
  );
  assert.match(
    source,
    /let web_search_enabled = resolve_pending_web_search_enabled\([\s\S]*&search_config,[\s\S]*\);/,
  );
  assert.match(
    source,
    /let web_search_allowed = resolve_pending_web_search_enabled\([\s\S]*&search_config,[\s\S]*\);/,
  );
  assert.doesNotMatch(
    source,
    /let web_search_(enabled|allowed)\s*=\s*processor\.is_web_search_requested/,
  );
});

test("A13: 搜索达到轮数上限后应强制基于已有结果生成最终回答", async () => {
  const source = await readSource("src-tauri/src/assistant_processor.rs");

  assert.match(source, /fn search_loop_limit_final_answer_prompt/);
  assert.match(
    source,
    /messages\.push\(Message::user\(search_loop_limit_final_answer_prompt\(\s*max_loops,\s*\)\)\);/,
  );
  assert.match(
    source,
    /\.chat_stream\([\s\S]*&messages,[\s\S]*self\.options_for_prompt_mode\(prompt_mode\),[\s\S]*None,[\s\S]*cancel_token\.clone\(\),/,
  );
  assert.doesNotMatch(
    source,
    /response\.tool_calls\.is_empty\(\)[\s\S]*\|\| loop_round >= max_loops/,
  );
});

test("A14: 口语流畅化模式应通过 tnl_config 字段级 patch 完成前后端闭环", async () => {
  const typesSource = await readSource("src/types/index.ts");
  const constantsSource = await readSource("src/constants/index.ts");
  const appSource = await readSource("src/App.tsx");
  const preferencesSource = await readSource("src/pages/PreferencesPage.tsx");
  const controllerSource = await readSource("src/hooks/useAppServiceController.ts");
  const eventsSource = await readSource("src/hooks/useTauriEventListeners.ts");
  const backendSource = await readSource("src-tauri/src/lib.rs");

  assert.match(typesSource, /export\s+type\s+DisfluencyMode\s*=\s*"off"\s*\|\s*"conservative"\s*\|\s*"aggressive"/);
  assert.match(typesSource, /export\s+interface\s+TnlConfig\s*\{[\s\S]*disfluency_mode:\s*DisfluencyMode/);
  assert.match(typesSource, /tnl_config:\s*TnlConfig;/);

  assert.match(constantsSource, /export\s+const\s+DEFAULT_TNL_CONFIG:\s*TnlConfig/);
  assert.match(constantsSource, /export\s+function\s+normalizeTnlConfig/);

  assert.match(controllerSource, /setTnlConfig:\s*React\.Dispatch<React\.SetStateAction<TnlConfig>>/);
  assert.match(controllerSource, /setTnlConfig\(normalizeTnlConfig\(config\.tnl_config\)\)/);
  assert.match(controllerSource, /tnlConfig\?:\s*\{[\s\S]*disfluencyMode\?:\s*DisfluencyMode/);

  assert.match(eventsSource, /setTnlConfig\?:\s*React\.Dispatch<React\.SetStateAction<TnlConfig>>/);
  assert.match(eventsSource, /setTnlConfig\?\.\(normalizeTnlConfig\(config\.tnl_config\)\)/);

  assert.match(appSource, /const\s+\[tnlConfig,\s*setTnlConfig\]\s*=\s*useState<TnlConfig>\(DEFAULT_TNL_CONFIG\)/);
  assert.match(appSource, /previousTnlConfig/);
  assert.match(appSource, /setTnlConfig\(normalizeTnlConfig\(\{[\s\S]*disfluency_mode:\s*patch\.tnlConfig\.disfluencyMode/);
  assert.match(appSource, /await\s+saveFieldPatchWithStatus\(\{\s*tnlConfig:\s*\{\s*disfluencyMode:\s*mode\s*\}\s*\}\)/);

  assert.match(preferencesSource, /口语流畅化/);
  assert.match(preferencesSource, /onSetDisfluencyMode:\s*\(mode:\s*DisfluencyMode\)\s*=>\s*Promise<void>/);
  assert.match(preferencesSource, /DISFLUENCY_MODE_OPTIONS\.map/);

  assert.match(backendSource, /struct\s+TnlConfigFieldPatch\s*\{[\s\S]*disfluency_mode:\s*Option<crate::tnl::DisfluencyMode>/);
  assert.match(backendSource, /tnl_config:\s*Option<TnlConfigFieldPatch>/);
  assert.match(backendSource, /config\.tnl_config\.disfluency_mode\s*=\s*mode;/);
});
