import assert from "node:assert/strict";
import { createHash, createPublicKey, verify } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

function verifySignature(data, signature, publicKey) {
  const keyLines = Buffer.from(publicKey, "base64").toString().trim().split(/\r?\n/);
  const packet = Buffer.from(keyLines[1] ?? "", "base64");
  assert(packet.length === 42 && packet.subarray(0, 2).toString() === "Ed", "公钥格式错误");
  const lines = Buffer.from(signature, "base64").toString().trim().split(/\r?\n/);
  const signed = Buffer.from(lines[1] ?? "", "base64");
  assert(signed.length === 74 && ["Ed", "ED"].includes(signed.subarray(0, 2).toString()), "签名格式错误");
  assert(signed.subarray(2, 10).equals(packet.subarray(2, 10)), "签名公钥标识不匹配");
  const key = createPublicKey({
    key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), packet.subarray(10)]),
    format: "der", type: "spki",
  });
  const message = signed.subarray(0, 2).toString() === "ED"
    ? createHash("blake2b512").update(data).digest() : data;
  assert(verify(null, message, key, signed.subarray(10)), "签名验证失败");
  assert(lines[2]?.startsWith("trusted comment: "), "签名说明格式错误");
  const globalMessage = Buffer.concat([signed.subarray(10), Buffer.from(lines[2].slice(17))]);
  assert(verify(null, globalMessage, key, Buffer.from(lines[3] ?? "", "base64")), "签名说明验证失败");
}

export function createManifest({ version, repository, notes, pubDate, publicKey, files }) {
  assert(/^\d+\.\d+\.\d+$/.test(version), "版本号必须为 x.y.z");
  assert(/^[\w.-]+\/[\w.-]+$/.test(repository), "仓库名称格式错误");
  assert(notes.trim(), "发布说明不能为空");
  assert(Number.isFinite(Date.parse(pubDate)), "发布日期无效");
  const required = (name) => {
    const data = files.get(name);
    assert(data?.length, `缺少发布产物：${name}`);
    return data;
  };
  const platforms = {};
  for (const [platform, suffix, bundle] of [
    ["windows-x86_64", "x64-setup.exe", "nsis"],
    ["darwin-aarch64", "aarch64.app.tar.gz", "app"],
    ["darwin-x86_64", "x64.app.tar.gz", "app"],
  ]) {
    const name = `PushToTalk_${version}_${suffix}`;
    const data = required(name);
    const signature = required(`${name}.sig`).toString().trim();
    verifySignature(data, signature, publicKey);
    const entry = { signature, url: `https://github.com/${repository}/releases/download/v${version}/${name}` };
    platforms[platform] = entry;
    platforms[`${platform}-${bundle}`] = entry;
  }
  for (const arch of ["aarch64", "x64"]) required(`PushToTalk_${version}_${arch}.dmg`);
  return { version, notes: notes.trim(), pub_date: pubDate, platforms };
}

function readProject() {
  const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
  const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
  const cargo = fs.readFileSync("src-tauri/Cargo.toml", "utf8").match(/^version = "([^"]+)"/m)?.[1];
  const cargoLock = fs.readFileSync("src-tauri/Cargo.lock", "utf8").match(/name = "push-to-talk"\r?\nversion = "([^"]+)"/)?.[1];
  for (const version of [config.version, lock.version, lock.packages[""].version, cargo, cargoLock]) {
    assert.equal(version, pkg.version, "项目版本号不一致");
  }
  const notes = fs.readFileSync(`docs/releases/v${pkg.version}.md`, "utf8");
  return { version: pkg.version, publicKey: config.plugins.updater.pubkey, notes };
}

function main() {
  const [command, argument, destination] = process.argv.slice(2);
  const project = readProject();
  if (command === "check-version") {
    if (argument) assert.equal(argument, `v${project.version}`, "标签与项目版本不一致");
    console.log(project.version);
  } else if (command === "stage") {
    const bundle = path.join("src-tauri/target", argument, "release/bundle");
    fs.mkdirSync(destination, { recursive: true });
    const copy = (source, name) => fs.copyFileSync(path.join(bundle, source), path.join(destination, name));
    if (argument === "x86_64-pc-windows-msvc") {
      const name = `PushToTalk_${project.version}_x64-setup.exe`;
      copy(`nsis/${name}`, name);
      copy(`nsis/${name}.sig`, `${name}.sig`);
    } else {
      assert(["aarch64-apple-darwin", "x86_64-apple-darwin"].includes(argument), "未知构建目标");
      const arch = argument.startsWith("aarch64") ? "aarch64" : "x64";
      const name = `PushToTalk_${project.version}_${arch}`;
      copy(`dmg/${name}.dmg`, `${name}.dmg`);
      copy("macos/PushToTalk.app.tar.gz", `${name}.app.tar.gz`);
      copy("macos/PushToTalk.app.tar.gz.sig", `${name}.app.tar.gz.sig`);
    }
  } else if (command === "manifest") {
    const files = new Map(fs.readdirSync(argument)
      .filter((name) => name.startsWith(`PushToTalk_${project.version}_`))
      .map((name) => [name, fs.readFileSync(path.join(argument, name))]));
    const manifest = createManifest({ ...project, files, repository: destination, pubDate: new Date().toISOString() });
    const json = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
    fs.writeFileSync(path.join(argument, "latest.json"), json);
    files.set("latest.json", json);
    const checksums = [...files].sort(([a], [b]) => a.localeCompare(b))
      .map(([name, data]) => `${createHash("sha256").update(data).digest("hex")}  ${name}`).join("\n");
    fs.writeFileSync(path.join(argument, "SHA256SUMS.txt"), `${checksums}\n`);
    console.log(`已验证 ${files.size - 1} 个产物及三个平台的更新签名。`);
  } else {
    throw new Error("用法：release-artifacts.mjs check-version [tag] | stage <target> <dir> | manifest <dir> <owner/repo>");
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
