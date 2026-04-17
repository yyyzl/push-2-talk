import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("NotificationWindow: 成功转写不再显示悬浮通知", async () => {
  const source = await readSource("src/windows/NotificationWindow.tsx");

  assert.match(source, /transcription_cancelled/);
  assert.match(source, /本次转写失败/);
  assert.doesNotMatch(source, /Omni 转写完成/);
  assert.doesNotMatch(source, /listen\("transcription_complete"/);
});

test("NotificationWindow: 临时提示应持有稳定的窗口引用，避免重渲染清掉自动隐藏计时器", async () => {
  const source = await readSource("src/windows/NotificationWindow.tsx");

  assert.match(source, /useRef\(getCurrentWindow\(\)\)/);
  assert.doesNotMatch(source, /\}, \[notificationWindow\]\)/);
});

test("NotificationWindow: 显示提示必须走后端 show_notification_window，不能前端直接 show 当前窗口", async () => {
  const source = await readSource("src/windows/NotificationWindow.tsx");

  assert.match(source, /invoke\("show_notification_window"\)/);
  assert.doesNotMatch(source, /notificationWindow\.show\(\)/);
});

test("NotificationWindow: 提示内容应在通知窗口内居中，避免再叠加额外 bottom 偏移", async () => {
  const source = await readSource("src/windows/NotificationWindow.tsx");

  assert.match(source, /fixed inset-0 flex items-center justify-center pointer-events-none/);
  assert.doesNotMatch(source, /bottom-12 left-1\/2 -translate-x-1\/2/);
});

test("NotificationWindow: 后端弹窗定位应与录音悬浮球共用底部居中基线", async () => {
  const source = await readSource("src-tauri/src/lib.rs");

  assert.match(source, /show_notification_window/);
  assert.match(source, /notification\s*\.\s*outer_size\(\)/);
  assert.match(source, /let y = monitor_pos\.y \+ screen_size\.height as i32\s*-\s*notification_size\.height as i32\s*-\s*\(100\.0 \* scale_factor\) as i32;/);
  assert.doesNotMatch(source, /bottom_offset = \(260\.0 \* scale_factor\) as i32/);
});

test("NotificationWindow: Tauri 通知窗口高度应与录音悬浮球一致，保证视觉落点一致", async () => {
  const source = await readSource("src-tauri/tauri.conf.json");

  assert.match(source, /"label": "notification"/);
  assert.match(source, /"width": 360/);
  assert.match(source, /"height": 80/);
});

test("Toggle: 轨道按钮应禁止 flex 压缩，避免公共转录配置里样式变形", async () => {
  const toggleSource = await readSource("src/components/common/Toggle.tsx");
  const configToggleSource = await readSource("src/components/common/ConfigToggle.tsx");

  assert.match(toggleSource, /shrink-0/);
  assert.match(configToggleSource, /shrink-0/);
});
