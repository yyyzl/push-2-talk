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
  const next = await select.locator("option").evaluateAll(options => options.find(option => !option.selected).value);
  await page.evaluate(() => { window.testDesktop.failWrites = true; });
  await select.selectOption(next);
  await page.getByText("模拟配置写入失败", { exact: false }).first().waitFor();
  assert.equal(await select.inputValue(), next, "failed write must preserve the selected model");
  await page.evaluate(() => { window.testDesktop.failWrites = false; });
  await page.getByRole("button", { name: "重试保存" }).click();
  await page.waitForFunction(() => window.testDesktop.calls.filter(call => call.command === "update_config").length === 2);
  const saved = await writes(page);
  assert.equal(saved.length, 2);
  assert.deepEqual(saved[0].args.patch, saved[1].args.patch);
  assert.deepEqual(Object.keys(saved[1].args.patch), ["asr_config"]);
  assert.equal(await select.inputValue(), next);
});

test("输入 API Key 的自动保存只写该字段并保留后台的新设置", async t => {
  const page = await open(t, "?idle");
  await page.getByRole("button", { name: "语音识别引擎", exact: true }).click();
  await page.getByPlaceholder("sk-...", { exact: true }).first().fill("fixture-edited-key");
  await page.evaluate(() => window.testDesktop.emitConfig({ theme: "dark" }));
  await page.waitForFunction(() => window.testDesktop.calls.some(call => call.command === "update_config"));
  assert.deepEqual((await writes(page)).map(call => call.args.patch), [{ asr_config: { credentials: { qwen_api_key: "fixture-edited-key" } } }]);
});
