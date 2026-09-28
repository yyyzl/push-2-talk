import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("P0: App 初始化 effect 应区分加载中与加载完成，避免重复初始化", async () => {
  const source = await readSource("src/App.tsx");

  assert.match(source, /useEffect\(\(\)\s*=>\s*\{\s*if\s*\(configInitializationRef\.current\.isStarted\(\)\)\s*return;/);
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
  const source = await readSource("src-tauri/src/application/assistant.rs");

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
