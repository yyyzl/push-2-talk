import assert from "node:assert/strict";
import test from "node:test";
import { permissionRequirements, platformKeyLabels, type PlatformStatus } from "../src/utils/platform";
const status: PlatformStatus = { os: "macos", microphone: "granted", accessibility: "denied", input_monitoring: "not_determined", other_app_mute: false, text_observation: true };
test("permission requirements distinguish missing permission from unsupported capabilities", () => {
  assert.deepEqual(permissionRequirements(status), ["accessibility", "input_monitoring"]);
});
test("permission recovery makes service ready without enabling unsupported mute", () => {
  const granted = { ...status, accessibility: "granted", input_monitoring: "granted" } as PlatformStatus;
  assert.deepEqual(permissionRequirements(granted), []);
  assert.equal(granted.other_app_mute, false);
});
test("restricted microphone is never treated as granted", () => {
  assert.ok(permissionRequirements({ ...status, microphone: "restricted" }).includes("microphone"));
});
test("Mac uses Command and Option while Windows retains its labels", () => {
  assert.equal(platformKeyLabels("macos").meta_left, "Cmd(左)");
  assert.equal(platformKeyLabels("macos").alt_right, "Option(右)");
  assert.equal(platformKeyLabels("windows").meta_left, "Win(左)");
});
