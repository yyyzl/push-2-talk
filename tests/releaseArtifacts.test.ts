import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import test from "node:test";
import { createManifest } from "../scripts/release-artifacts.mjs";

// 使用临时测试密钥生成真实 Ed25519 / Minisign 签名，不接触发布私钥。
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const keyId = Buffer.from("0102030405060708", "hex");
const publicPacket = Buffer.concat([
  Buffer.from("Ed"), keyId,
  publicKey.export({ type: "spki", format: "der" }).subarray(-32),
]);
const encodedPublicKey = Buffer.from(`untrusted comment: test key\n${publicPacket.toString("base64")}\n`).toString("base64");

function signature(data: Buffer) {
  const message = createHash("blake2b512").update(data).digest();
  const raw = sign(null, message, privateKey);
  const packet = Buffer.concat([Buffer.from("ED"), keyId, raw]);
  const comment = "timestamp:1\tfile:test";
  const global = sign(null, Buffer.concat([raw, Buffer.from(comment)]), privateKey);
  return Buffer.from(`untrusted comment: test signature\n${packet.toString("base64")}\ntrusted comment: ${comment}\n${global.toString("base64")}\n`).toString("base64");
}

function fixture() {
  const files = new Map<string, Buffer>();
  for (const name of [
    "PushToTalk_1.7.0_x64-setup.exe",
    "PushToTalk_1.7.0_aarch64.app.tar.gz",
  ]) {
    const data = Buffer.from(`package: ${name}`);
    files.set(name, data);
    files.set(`${name}.sig`, Buffer.from(signature(data)));
  }
  files.set("PushToTalk_1.7.0_aarch64.dmg", Buffer.from("arm dmg"));
  return { version: "1.7.0", repository: "yyyzl/push-2-talk", notes: "实际更新说明", pubDate: "2026-09-28T12:00:00Z", publicKey: encodedPublicKey, files };
}

test("仅发布 Windows x64 与 Apple Silicon，并保留旧版 Windows 更新键", () => {
  const manifest = createManifest(fixture());
  assert.equal(manifest.version, "1.7.0");
  assert.equal(manifest.notes, "实际更新说明");
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    "darwin-aarch64", "darwin-aarch64-app",
    "windows-x86_64", "windows-x86_64-nsis",
  ]);
  assert.equal(manifest.platforms["windows-x86_64"].url, "https://github.com/yyyzl/push-2-talk/releases/download/v1.7.0/PushToTalk_1.7.0_x64-setup.exe");
  assert.equal(manifest.platforms["darwin-aarch64"].url.endsWith("_aarch64.app.tar.gz"), true);
});

test("缺少任何安装包或签名时不生成半成品更新清单", () => {
  for (const name of fixture().files.keys()) {
    const input = fixture();
    input.files.delete(name);
    assert.throws(() => createManifest(input), /缺少发布产物/);
  }
});

test("修改包体、替换签名或公钥时拒绝发布", () => {
  const modified = fixture();
  modified.files.set("PushToTalk_1.7.0_x64-setup.exe", Buffer.from("modified"));
  assert.throws(() => createManifest(modified), /签名验证失败/);
  const wrongSignature = fixture();
  wrongSignature.files.set("PushToTalk_1.7.0_x64-setup.exe.sig", wrongSignature.files.get("PushToTalk_1.7.0_aarch64.app.tar.gz.sig")!);
  assert.throws(() => createManifest(wrongSignature), /签名验证失败/);
  assert.throws(() => createManifest({ ...fixture(), publicKey: "invalid" }), /公钥格式/);
});

test("拒绝损坏的签名说明以及错误版本、空说明", () => {
  const input = fixture();
  const name = "PushToTalk_1.7.0_x64-setup.exe.sig";
  const text = Buffer.from(input.files.get(name)!.toString(), "base64").toString().replace("timestamp:1", "timestamp:2");
  input.files.set(name, Buffer.from(Buffer.from(text).toString("base64")));
  assert.throws(() => createManifest(input), /签名说明验证失败/);
  assert.throws(() => createManifest({ ...fixture(), version: "1.7.1" }), /缺少发布产物/);
  assert.throws(() => createManifest({ ...fixture(), notes: " " }), /发布说明不能为空/);
});
