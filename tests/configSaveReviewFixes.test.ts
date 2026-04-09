import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("Omni-only: 侧边栏不再暴露 LLM、语句润色、AI 助手入口", async () => {
  const source = await readSource("src/components/layout/Sidebar.tsx");

  assert.match(source, /PushToTalk Omni/);
  assert.doesNotMatch(source, /LLM 模型配置/);
  assert.doesNotMatch(source, /语句润色/);
  assert.doesNotMatch(source, /AI 助手/);
});

test("Omni-only: Dashboard 右栏应只展示 Omni 状态、热键和词库", async () => {
  const source = await readSource("src/components/layout/RightPanel.tsx");

  assert.match(source, /当前引擎/);
  assert.match(source, /公共转录配置/);
  assert.match(source, /作用于所有 Omni 服务商预设/);
  assert.match(source, /包含内置词库/);
  assert.doesNotMatch(source, /TNL 规范化/);
  assert.doesNotMatch(source, /语句润色/);
  assert.doesNotMatch(source, /词库增强/);
  assert.doesNotMatch(source, /快捷助手/);
  assert.doesNotMatch(source, /实时流式模式/);
});

test("Omni-only: ASR 页面应只保留 Omni 配置入口", async () => {
  const source = await readSource("src/pages/AsrPage.tsx");

  assert.match(source, /当前分支仅保留 Omni 识别链路/);
  assert.match(source, /服务商预设/);
  assert.match(source, /请求 URL/);
  assert.match(source, /支持 OpenAI 兼容地址/);
  assert.match(source, /自定义转录规则（可选）/);
  assert.doesNotMatch(source, /豆包输入法/);
  assert.doesNotMatch(source, /Grok/);
  assert.doesNotMatch(source, /备用模型/);
});

test("Omni-only: 偏好页应移除学习与 LLM 绑定入口", async () => {
  const source = await readSource("src/pages/PreferencesPage.tsx");

  assert.doesNotMatch(source, /自动词库学习/);
  assert.doesNotMatch(source, /LLM 连接配置/);
  assert.doesNotMatch(source, /onNavigateToModels/);
  assert.match(source, /开机自启动/);
  assert.match(source, /录音时静音其他应用/);
});

test("Omni-only: 历史与最近活动统一使用 Omni 转写语义", async () => {
  const historySource = await readSource("src/pages/HistoryPage.tsx");
  const recentSource = await readSource("src/components/live/RecentActivity.tsx");
  const transcriptSource = await readSource("src/components/live/TranscriptDisplay.tsx");

  assert.match(historySource, /Omni 转写/);
  assert.match(historySource, /Omni 结果/);
  assert.doesNotMatch(historySource, /AI 助手/);
  assert.doesNotMatch(historySource, /润色后/);

  assert.match(recentSource, /Omni 转写/);
  assert.doesNotMatch(recentSource, /AI 助手/);
  assert.doesNotMatch(recentSource, /智能润色/);

  assert.match(transcriptSource, /Omni 结果/);
  assert.doesNotMatch(transcriptSource, /LLM \{/);
});

test("Omni-only: 通知窗口改为识别结果提示，不再承载学习建议", async () => {
  const source = await readSource("src/windows/NotificationWindow.tsx");

  assert.match(source, /识别通知区域/);
  assert.match(source, /transcription_cancelled/);
  assert.match(source, /本次转写失败/);
  assert.doesNotMatch(source, /transcription_complete/);
  assert.doesNotMatch(source, /vocabulary_learning_suggestion/);
  assert.doesNotMatch(source, /VocabularyLearningToast/);
});

test("Omni-only: Tauri 配置应切换到独立产品身份并关闭 updater 发布流", async () => {
  const source = await readSource("src-tauri/tauri.conf.json");

  assert.match(source, /"productName": "PushToTalk Omni"/);
  assert.match(source, /"identifier": "com\.pushtotalk\.omni"/);
  assert.match(source, /"title": "PushToTalk Omni"/);
  assert.match(source, /"title": "识别通知"/);
  assert.match(source, /"capabilities": \["default"\]/);
  assert.match(source, /"createUpdaterArtifacts": false/);
  assert.doesNotMatch(source, /"plugins":/);
});
