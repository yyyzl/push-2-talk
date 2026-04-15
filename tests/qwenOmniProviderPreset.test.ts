import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { OMNI_ENDPOINT_PRESETS } from "../src/constants";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const readSource = (relativePath: string) => readFile(path.join(testDir, "..", relativePath), "utf8");

test("Qwen Omni preset: should expose the exact DashScope defaults", () => {
  const qwenPreset = OMNI_ENDPOINT_PRESETS.find((preset) => preset.label === "Qwen");

  assert.ok(qwenPreset, "Qwen preset should exist");
  assert.equal(
    qwenPreset.profileKey,
    "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
  );
  assert.deepEqual(qwenPreset.defaults, {
    endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
    api_key: "",
    model: "qwen3.5-omni-plus",
    thinking_supported: false,
    skip_post_processing: true,
  });
});

test("Qwen Omni preset: AsrPage should expose accessible provider tabs", async () => {
  const pageSource = await readSource("src/pages/AsrPage.tsx");

  assert.ok(pageSource.includes('role="tablist"'));
  assert.ok(pageSource.includes('role="tab"'));
  assert.ok(pageSource.includes('aria-label="Omni 服务商预设"'));
  assert.ok(pageSource.includes('role="tabpanel"'));
  assert.ok(pageSource.includes('aria-labelledby={activeOmniTabId}'));
  assert.ok(pageSource.includes('tabIndex={selected ? 0 : -1}'));
  assert.ok(pageSource.includes('id={activeOmniPanelId}'));
  assert.ok(pageSource.includes('aria-controls={activeOmniPanelId}'));
  assert.ok(pageSource.includes('onKeyDown={(event) => handleOmniPresetTabKeyDown(event, presetIndex)}'));
});

test("Qwen Omni preset: AsrPage should associate provider tabs with stable ids", async () => {
  const pageSource = await readSource("src/pages/AsrPage.tsx");

  assert.ok(pageSource.includes('const activeOmniTabId = `omni-provider-tab-${activeOmniPreset.label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`'));
  assert.ok(pageSource.includes('const getOmniPresetTabId = (label: string) => `omni-provider-tab-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`'));
  assert.ok(pageSource.includes('const focusOmniPresetTab = (presetKey: string) => {'));
  assert.ok(pageSource.includes('const handleOmniPresetTabKeyDown = ('));
  assert.ok(pageSource.includes('id={tabId}'));
});

test("Qwen Omni preset: AsrPage should keep the tablist outside the controlled panel", async () => {
  const pageSource = await readSource("src/pages/AsrPage.tsx");
  const tablistIndex = pageSource.indexOf('role="tablist"');
  const panelIdIndex = pageSource.indexOf('id={activeOmniPanelId}');
  const tabpanelRoleIndex = pageSource.indexOf('role="tabpanel"');

  assert.notEqual(tablistIndex, -1);
  assert.notEqual(panelIdIndex, -1);
  assert.notEqual(tabpanelRoleIndex, -1);
  assert.ok(panelIdIndex > tablistIndex, "tabpanel section should appear after the tablist in source order");
  assert.ok(tabpanelRoleIndex > tablistIndex, "tabpanel role should appear after the tablist in source order");
});

test("Qwen Omni preset: custom preset should still exist after Qwen insertion", () => {
  const customPreset = OMNI_ENDPOINT_PRESETS.find((preset) => preset.label === "自定义");

  assert.ok(customPreset, "custom preset should still exist");
  assert.equal(customPreset.profileKey, "__custom__");
});
