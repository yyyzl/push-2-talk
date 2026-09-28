import assert from "node:assert/strict";
import test from "node:test";
import { getSyncWindowNoticeMessage } from "../src/utils/configSyncWindow";

test("全局同步提示文案会基于 source 区分初始加载与外部更新", () => {
  assert.equal(getSyncWindowNoticeMessage("initial_load"), "正在加载初始配置");
  assert.equal(getSyncWindowNoticeMessage("external_config_updated"), "正在同步外部配置");
  assert.equal(getSyncWindowNoticeMessage(null), null);
});
