import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("Omni screenshot context: 前端共享配置应声明截图发送与调试保存开关", async () => {
  const typesSource = await readSource("src/types/index.ts");
  const constantsSource = await readSource("src/constants/index.ts");

  assert.match(typesSource, /include_focused_window_screenshot:\s*boolean;/);
  assert.match(typesSource, /debug_save_focused_window_screenshot:\s*boolean;/);
  assert.match(constantsSource, /include_focused_window_screenshot:\s*true/);
  assert.match(constantsSource, /debug_save_focused_window_screenshot:\s*false/);
});

test("Omni screenshot context: ASR 页面应暴露焦点窗口截图开关", async () => {
  const source = await readSource("src/pages/AsrPage.tsx");

  assert.match(source, /焦点窗口截图/);
  assert.match(source, /调试保存截图/);
  assert.match(source, /latest-focused-window\.png/);
  assert.match(source, /checked=\{omniSharedConfig\.include_focused_window_screenshot\}/);
  assert.match(source, /checked=\{omniSharedConfig\.debug_save_focused_window_screenshot\}/);
  assert.match(
    source,
    /updateOmniSharedConfig\(\{\s*include_focused_window_screenshot:\s*v\s*}\)/
  );
  assert.match(
    source,
    /updateOmniSharedConfig\(\{\s*debug_save_focused_window_screenshot:\s*v\s*}\)/
  );
});

test("Omni screenshot context: 后端配置与截图模块应接入共享开关", async () => {
  const configSource = await readSource("src-tauri/src/config.rs");
  const libSource = await readSource("src-tauri/src/lib.rs");
  const captureSource = await readSource("src-tauri/src/window_capture.rs");

  assert.match(configSource, /pub include_focused_window_screenshot:\s*bool/);
  assert.match(configSource, /include_focused_window_screenshot:\s*true/);
  assert.match(configSource, /pub debug_save_focused_window_screenshot:\s*bool/);
  assert.match(configSource, /debug_save_focused_window_screenshot:\s*false/);
  assert.match(libSource, /include_focused_window_screenshot/);
  assert.match(libSource, /debug_save_focused_window_screenshot/);
  assert.match(captureSource, /PrintWindow/);
  assert.match(captureSource, /BitBlt/);
  assert.match(captureSource, /latest-focused-window\.png/);
  assert.match(captureSource, /write_debug_screenshot/);
});

test("Omni screenshot context: Omni 请求构建应在有截图时附带图像输入和约束提示", async () => {
  const source = await readSource("src-tauri/src/asr/omni/mod.rs");

  assert.match(source, /build_request_body/);
  assert.match(source, /FocusedWindowScreenshot/);
  assert.match(source, /image_url|input_image/);
  assert.match(source, /截图仅作为辅助线索/);
  assert.match(source, /不要凭截图臆造/);
});
