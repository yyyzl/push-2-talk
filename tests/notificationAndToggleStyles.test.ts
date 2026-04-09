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

test("Toggle: 轨道按钮应禁止 flex 压缩，避免公共转录配置里样式变形", async () => {
  const toggleSource = await readSource("src/components/common/Toggle.tsx");
  const configToggleSource = await readSource("src/components/common/ConfigToggle.tsx");

  assert.match(toggleSource, /shrink-0/);
  assert.match(configToggleSource, /shrink-0/);
});
