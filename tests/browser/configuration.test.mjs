import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { chromium } from "playwright";
import { createServer } from "vite";
import { readFile } from "node:fs/promises";

let server, browser, base;
before(async () => {
  server = await createServer({ server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
  base = `http://127.0.0.1:${server.httpServer.address().port}`;
  browser = await chromium.launch({ headless: true });
});
after(async () => { await browser?.close(); await server?.close(); });

async function open(t, query = "") {
  const context = await browser.newContext();
  t.after(() => context.close());
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  t.after(() => assert.deepEqual(errors, [], "React must not throw"));
  await page.route(base + "/**", async route => {
    if (new URL(route.request().url()).pathname === "/") {
      const response = await route.fetch();
      await route.fulfill({ response, body: (await response.text()).replace('/src/main.tsx', '/tests/browser/entry.tsx') });
    } else await route.continue();
  });
  await page.goto(base + "/" + query);
  await page.waitForFunction(() => window.testDesktop?.calls.some(call => call.command === "start_app"));
  return page;
}
const writes = page => page.evaluate(() => window.testDesktop.calls.filter(call => ["update_config", "save_config", "patch_config_fields"].includes(call.command)));

test("线上配置启动及外部通知不会触发自动回写", async t => {
  const page = await open(t);
  const legacy = JSON.parse(await readFile(new URL("../fixtures/config/v1.6.1.json", import.meta.url), "utf8"));
  const started = await page.evaluate(() => window.testDesktop.calls.find(call => call.command === "start_app").args);
  assert.equal(started.apiKey, legacy.asr_config.credentials.qwen_api_key);
  assert.deepEqual(started.asrConfig.credentials, legacy.asr_config.credentials);
  assert.deepEqual(started.dualHotkeyConfig, legacy.dual_hotkey_config);
  assert.deepEqual(started.llmConfig.shared.providers, legacy.llm_config.shared.providers);
  await page.waitForTimeout(1200); // Longer than the actual save debounce.
  assert.deepEqual(await writes(page), []);
  await page.evaluate(() => window.testDesktop.emitConfig({ theme: "dark" }));
  await page.waitForTimeout(1200);
  assert.deepEqual(await writes(page), []);
});

test("实际配置开关保存失败保留选择，重试不会反向保存旧值", async t => {
  const page = await open(t, "?idle");
  const toggle = page.getByRole("switch").first();
  const before = await toggle.getAttribute("aria-checked");
  await page.evaluate(() => { window.testDesktop.failWrites = true; });
  await toggle.click();
  await page.getByText("模拟配置写入失败", { exact: false }).first().waitFor();
  assert.equal(await toggle.getAttribute("aria-checked"), before === "true" ? "false" : "true");
  await page.evaluate(() => { window.testDesktop.failWrites = false; });
  await page.getByRole("button", { name: "重试保存" }).click();
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "update_config").length === 2);
  const saved = await writes(page);
  assert.deepEqual(saved[0].args.patch, saved[1].args.patch);
});

test("真实模型选择器失败保留选择，用户可直接重试保存", async t => {
  const page = await open(t, "?idle");
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  const select = page.locator("#qwen-model-http");
  const next = "Audio 3.1 · 录音识别";
  await page.evaluate(() => { window.testDesktop.failWrites = true; });
  await select.click();
  await page.getByRole("option", { name: next, exact: true }).click();
  await page.getByText("模拟配置写入失败", { exact: false }).first().waitFor();
  assert.equal(await select.innerText(), next, "failed write must preserve the selected model");
  await page.evaluate(() => { window.testDesktop.failWrites = false; });
  await page.getByRole("button", { name: "重试保存" }).click();
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "update_config").length === 2);
  const saved = await writes(page);
  assert.equal(saved.length, 2);
  assert.deepEqual(saved[0].args.patch, saved[1].args.patch);
  assert.deepEqual(Object.keys(saved[1].args.patch), ["asr_config"]);
  assert.equal(await select.innerText(), next);
});

test("输入 API Key 的自动保存只写该字段并保留后台的新设置", async t => {
  const page = await open(t, "?idle");
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  await page.getByPlaceholder("sk-...", { exact: true }).first().fill("fixture-edited-key");
  await page.evaluate(() => window.testDesktop.emitConfig({ theme: "dark" }));
  await page.waitForFunction(() => window.testDesktop.calls.some(call => call.command === "update_config"));
  assert.deepEqual((await writes(page)).map(call => call.args.patch), [{ asr_config: { credentials: { qwen_api_key: "fixture-edited-key" } } }]);
});

test("服务慢启动时保留用户输入，启动完成后才保存并应用最新设置", async t => {
  const page = await open(t, "?slow-start");
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  await page.getByPlaceholder("sk-...", { exact: true }).first().fill("fixture-during-startup");
  await page.waitForTimeout(1200);
  assert.deepEqual(await writes(page), [], "startup must finish before configuration saves can restart the service");
  await page.evaluate(() => window.testDesktop.finishStartup());
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "start_app").length === 2);
  const starts = await page.evaluate(() => window.testDesktop.calls.filter(call => call.command === "start_app"));
  assert.equal(starts[1].args.apiKey, "fixture-during-startup");
  assert.deepEqual((await writes(page)).map(call => call.args.patch), [{ asr_config: { credentials: { qwen_api_key: "fixture-during-startup" } } }]);
});


test("千问下拉显示分模式的全部模型，键盘取消不保存，选择只改对应模型", async t => {
  const page = await open(t, "?idle");
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  const http = page.getByRole("combobox", { name: "松开后识别的模型", exact: true });
  await http.click();
  await page.getByRole("listbox").waitFor({ timeout: 2000 });
  assert.equal(await page.getByRole("option").count(), 5);
  await page.keyboard.press("End");
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => document.activeElement?.id === "qwen-model-http");
  assert.deepEqual(await writes(page), []);
  await http.press("Enter");
  await page.getByRole("option", { name: "Qwen3 · 稳定版", exact: true }).press("Home");
  await page.waitForFunction(() => document.activeElement?.getAttribute("aria-label") === "Audio 3.1 · 录音识别");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.testDesktop.calls.some(call => call.command === "update_config"));
  assert.deepEqual((await writes(page))[0].args.patch, { asr_config: { qwen_models: { http: "qwen-audio-3.1-asr-flash" } } });
  const realtime = page.getByRole("combobox", { name: "边说边识别的模型", exact: true });
  await realtime.click();
  assert.equal(await page.getByRole("option").count(), 6);
  await page.getByRole("option", { name: "Audio 3.1 Message · 语音输入", exact: true }).click();
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "update_config").length === 2);
  assert.deepEqual((await writes(page))[1].args.patch, { asr_config: { qwen_models: { realtime: "qwen-audio-3.1-asr-flash-message" } } });
});

test("备用识别开关保留旧服务，不再静默覆盖为硅基流动", async t => {
  const page = await open(t, "?idle");
  await page.evaluate(() => window.testDesktop.emitConfig({ asr_config: { selection: { fallback_provider: "qwen", enable_fallback: true } } }));
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  const toggle = page.getByRole("switch", { name: "启用备用识别", exact: true });
  await toggle.waitFor({ timeout: 2000 });
  await toggle.click();
  await page.waitForFunction(() => window.testDesktop.calls.some(call => call.command === "update_config"));
  await toggle.click();
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "update_config").length === 2);
  assert.deepEqual((await writes(page)).map(call => call.args.patch), [
    { asr_config: { selection: { enable_fallback: false } } },
    { asr_config: { selection: { enable_fallback: true } } },
  ]);
  await page.getByText("已保存的备用服务：阿里千问", { exact: false }).waitFor();
});


test("千问全部 11 个模型可保存并在重载后恢复，凭据和另一模式保持不变", async t => {
  const page = await open(t, "?idle");
  const catalogue = JSON.parse(await readFile(new URL("../../src/shared/qwen-models.json", import.meta.url), "utf8"));
  await page.goto(base + "/tests/ui/asr-model-selection.html");
  for (const model of [...catalogue].reverse()) {
    const before = JSON.parse(await page.getByLabel("已保存的验收配置").innerText());
    await page.locator(`#qwen-model-${model.mode}`).click();
    await page.getByRole("option", { name: model.label, exact: true }).click();
    await page.waitForFunction(({ mode, id }) => JSON.parse(localStorage.getItem("ptt-asr-acceptance-fixture") || "{}").qwen_models?.[mode] === id, model);
    await page.reload();
    const after = JSON.parse(await page.getByLabel("已保存的验收配置").innerText());
    assert.deepEqual(after.credentials, before.credentials);
    assert.equal(after.qwen_models[model.mode], model.id);
    const other = model.mode === "http" ? "realtime" : "http";
    assert.equal(after.qwen_models[other], before.qwen_models?.[other]);
    assert.equal(await page.locator(`#qwen-model-${model.mode}`).innerText(), model.label);
  }
  await page.getByRole("button", { name: "载入未知模型", exact: true }).click();
  assert.match(await page.locator("#qwen-model-http").innerText(), /future-saved-model/);
  await page.getByLabel("服务运行中").check();
  assert.equal(await page.locator("#qwen-model-http").isDisabled(), true);
});

test("豆包输入法的备用服务能选择千问并独立保存 HTTP 模型", async t => {
  const page = await open(t, "?idle");
  await page.evaluate(() => window.testDesktop.emitConfig({ asr_config: { selection: { active_provider: "doubao_ime" } } }));
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  await page.getByRole("combobox", { name: "备用服务", exact: true }).click();
  assert.equal(await page.getByRole("option").count(), 3);
  await page.getByRole("option", { name: "阿里千问", exact: true }).click();
  await page.locator("#fallback-qwen-model-http").click();
  await page.getByRole("option", { name: "Audio 3.0 · 录音识别", exact: true }).click();
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "update_config").length === 2);
  assert.deepEqual((await writes(page)).map(call => call.args.patch), [
    { asr_config: { selection: { fallback_provider: "qwen" } } },
    { asr_config: { qwen_models: { http: "qwen-audio-3.0-asr-flash" } } },
  ]);
});


test("公共选择器保留未知值，支持空值继承、搜索及禁用项，弹层不受父容器裁切", async t => {
  const page = await open(t, "?idle");
  await page.setViewportSize({ width: 800, height: 600 });
  await page.goto(base + "/tests/ui/select-controls.html");
  const select = page.getByRole("combobox", { name: "模型选择", exact: true });
  assert.equal(await select.innerText(), "saved-removed");
  await select.click();
  const popup = page.getByRole("listbox");
  const box = await popup.boundingBox();
  assert.ok(box.y + box.height <= 600 && box.width >= 220 && box.height > 100);
  assert.equal(await page.getByRole("option", { name: "不可选旧模型", exact: true }).getAttribute("aria-disabled"), "true");
  await page.getByRole("option", { name: "跟随默认", exact: true }).click();
  assert.equal(await page.getByLabel("当前值").innerText(), '""');
  assert.equal(await page.getByLabel("父项点击次数").innerText(), "0");
  await select.click();
  await page.getByRole("option", { name: "跟随默认", exact: true }).press("ArrowDown");
  await page.waitForFunction(() => document.activeElement?.textContent === "Alpha");
  await page.keyboard.press("Enter");
  assert.equal(await page.getByLabel("当前值").innerText(), '"alpha"');
  await select.click();
  await page.getByRole("option", { name: "Alpha", exact: true }).press("m");
  await page.waitForFunction(() => document.activeElement?.textContent === "Model 0");
  await page.keyboard.press("Enter");
  assert.equal(await page.getByLabel("当前值").innerText(), '"model-0"');
  await select.click();
  // Radix 在 effect 中延迟注册外部点击监听；先等弹层定位和焦点就绪。
  // 使用带可操作性等待的点击，避免裸 mouse.click 抢在监听安装前到达。
  await popup.waitFor({ state: "visible" });
  await page.waitForFunction(() => document.activeElement?.textContent === "Model 0");
  const outside = { x: 700, y: 550 };
  const openedBox = await popup.boundingBox();
  assert.ok(outside.x > openedBox.x + openedBox.width, "关闭测试必须点击弹层外部");
  await page.locator("html").click({ position: outside });
  await popup.waitFor({ state: "hidden", timeout: 2000 });
  await page.waitForFunction(() => document.activeElement?.getAttribute("aria-label") === "模型选择");
  assert.equal(await page.getByLabel("当前值").innerText(), '"model-0"', "外部关闭不能改变选择");
  assert.equal(await page.getByRole("combobox", { name: "空列表", exact: true }).isDisabled(), true);
});
