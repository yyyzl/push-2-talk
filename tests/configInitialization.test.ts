import assert from "node:assert/strict";
import test from "node:test";
import { createConfigInitialization } from "../src/utils/configInitialization";
import { createConfigSyncWindowController } from "../src/utils/configSyncWindow";

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test("慢加载期间，即使同步窗口已结束且保存计时器到期，也不能写回默认配置", async () => {
  const gate = createConfigInitialization();
  const sync = createConfigSyncWindowController();
  const pending = deferred();
  let config = "defaults";
  const writes: string[] = [];
  const loading = gate.run(async () => { await pending.promise; config = "old saved config"; });
  const external = sync.begin("external_config_updated");
  sync.complete(external);
  const onDebounce = () => { if (gate.isReady() && !sync.isSuppressed()) writes.push(config); };
  onDebounce();
  assert.deepEqual(writes, []);
  pending.resolve();
  await loading;
  onDebounce();
  assert.deepEqual(writes, ["old saved config"]);
});

test("加载期间重复渲染不会再发起加载，成功后也不会重复加载", async () => {
  const gate = createConfigInitialization();
  const pending = deferred();
  let calls = 0;
  const load = async () => { calls += 1; await pending.promise; };
  const first = gate.run(load);
  await gate.run(load);
  assert.equal(calls, 1);
  assert.equal(gate.isReady(), false);
  pending.resolve();
  await first;
  await gate.run(load);
  assert.equal(calls, 1);
  assert.equal(gate.isReady(), true);
});

test("加载失败后禁止自动保存，成功重试后才恢复", async () => {
  const gate = createConfigInitialization();
  await assert.rejects(gate.run(async () => { throw new Error("invalid old config"); }));
  assert.equal(gate.isReady(), false);
  assert.equal(gate.isStarted(), false);
  await gate.run(async () => {});
  assert.equal(gate.isReady(), true);
});
