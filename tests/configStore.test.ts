import assert from "node:assert/strict";
import test from "node:test";
import { ConfigStore, type ConfigSnapshot, type DeepPatch } from "../src/state/configStore";

type Settings = { theme: string; asr: { key: string; model: string }; llm: { model?: string | null }; enabled: boolean };
const initial = (): Settings => ({ theme: "dark", asr: { key: "saved-key", model: "saved-model" }, llm: {}, enabled: true });
const loaded = () => { const store = new ConfigStore(initial()); store.receive({ revision: 1, config: initial() }); return store; };
const deferred = <T>() => { let resolve!: (value: T) => void; let reject!: (error: Error) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };

test("loading and external snapshots never become edits or echo back to the backend", async () => {
  const store = loaded(); let calls = 0;
  store.receive({ revision: 2, config: { ...initial(), theme: "light" } });
  await store.flush(async () => { calls++; return { revision: 3, config: initial() }; });
  assert.equal(store.getSnapshot().config.theme, "light");
  assert.equal(store.getSnapshot().dirty, false); assert.equal(calls, 0);
});

test("an unrelated backend change survives a local nested model edit", async () => {
  const store = loaded();
  store.edit(c => ({ ...c, asr: { ...c.asr, model: "chosen" } }));
  store.receive({ revision: 2, config: { ...initial(), asr: { key: "refreshed-key", model: "saved-model" } } });
  assert.equal(store.getSnapshot().config.asr.key, "refreshed-key");
  assert.equal(store.getSnapshot().config.asr.model, "chosen");
  let patch: DeepPatch<Settings> | undefined;
  await store.flush(async change => { patch = change; return { revision: 3, config: store.getSnapshot().config }; });
  assert.deepEqual(patch, { asr: { model: "chosen" } }); assert.equal(store.getSnapshot().dirty, false);
});

test("stale responses and events cannot roll back a newer backend revision", () => {
  const store = loaded();
  store.receive({ revision: 4, config: { ...initial(), theme: "newest" } });
  store.receive({ revision: 2, config: initial() });
  assert.equal(store.getSnapshot().config.theme, "newest");
});

test("failed saves keep the draft and can be retried without entering credentials again", async () => {
  const store = loaded(); store.edit(c => ({ ...c, theme: "light" }));
  await assert.rejects(store.flush(async () => { throw Error("disk full"); }), /disk full/);
  assert.equal(store.getSnapshot().dirty, true); assert.equal(store.getSnapshot().saving, false);
  assert.equal(store.getSnapshot().config.theme, "light");
  await store.flush(async patch => { assert.deepEqual(patch, { theme: "light" }); return { revision: 2, config: { ...initial(), theme: "light" } }; });
  assert.equal(store.getSnapshot().dirty, false); assert.equal(store.getSnapshot().config.asr.key, "saved-key");
});

test("edits made while a save is in flight are serialized and preserved", async () => {
  const store = loaded(); const first = deferred<ConfigSnapshot<Settings>>(); const patches: DeepPatch<Settings>[] = [];
  store.edit(c => ({ ...c, theme: "light" }));
  const flushing = store.flush(async patch => { patches.push(patch); return patches.length === 1 ? first.promise : { revision: 3, config: { ...initial(), theme: "final" } }; });
  store.edit(c => ({ ...c, theme: "final" }));
  first.resolve({ revision: 2, config: { ...initial(), theme: "light" } });
  await flushing;
  assert.deepEqual(patches, [{ theme: "light" }, { theme: "final" }]);
  assert.equal(store.getSnapshot().config.theme, "final"); assert.equal(store.getSnapshot().dirty, false);
});

test("reverting during a save still writes the requested final value", async () => {
  const store = loaded(); const first = deferred<ConfigSnapshot<Settings>>(); const patches: DeepPatch<Settings>[] = [];
  store.edit(c => ({ ...c, enabled: false }));
  const flushing = store.flush(async patch => { patches.push(patch); return patches.length === 1 ? first.promise : { revision: 3, config: initial() }; });
  store.edit(c => ({ ...c, enabled: true }));
  first.resolve({ revision: 2, config: { ...initial(), enabled: false } });
  await flushing;
  assert.deepEqual(patches, [{ enabled: false }, { enabled: true }]);
});

test("clearing an optional model is explicit null, omission leaves it unchanged", async () => {
  const store = loaded(); store.receive({ revision: 2, config: { ...initial(), llm: { model: "custom" } } });
  store.edit(c => ({ ...c, llm: {} }));
  let saved: DeepPatch<Settings> | undefined;
  await store.flush(async patch => { saved = patch; return { revision: 3, config: initial() }; });
  assert.deepEqual(saved, { llm: { model: null } });
});

test("a field restored before any save does not cause a write", async () => {
  const store = loaded(); store.edit(c => ({ ...c, theme: "light" })); store.edit(c => ({ ...c, theme: "dark" }));
  await store.flush(async () => { throw Error("no changes to save"); });
  assert.equal(store.getSnapshot().dirty, false);
});


test("an edit from a stale rendered form does not overwrite freshly refreshed credentials", async () => {
  const store = loaded(); const rendered = store.getSnapshot().config;
  store.receive({ revision: 2, config: { ...initial(), asr: { key: "refreshed", model: "saved-model" } } });
  store.edit(c => ({ ...c, asr: { ...c.asr, model: "chosen" } }), rendered);
  assert.deepEqual(store.getSnapshot().config.asr, { key: "refreshed", model: "chosen" });
  let saved: DeepPatch<Settings> | undefined;
  await store.flush(async patch => { saved = patch; return { revision: 3, config: store.getSnapshot().config }; });
  assert.deepEqual(saved, { asr: { model: "chosen" } });
});

test("a synchronously throwing transport can be retried", async () => {
  const store = loaded(); store.edit(c => ({ ...c, theme: "light" }));
  await assert.rejects(store.flush(() => { throw Error("invoke unavailable"); }));
  await store.flush(async () => ({ revision: 2, config: { ...initial(), theme: "light" } }));
  assert.equal(store.getSnapshot().dirty, false);
});
