#!/usr/bin/env node
// 一条命令改版本：Cargo.toml 的 [workspace.package] version、package.json 的 version，
// 并用 cargo update -w 把 Cargo.lock 里的 workspace 成员版本一起带动（否则带 --locked 的 CI 会红）。
// 顺带按 changelog/README.md 的格式建 changelog/<version>.md 模板。
//
// 用法：pnpm release:bump 0.2.0-alpha.3
//
// 只接受显式版本号，不猜 patch/minor：0.2.0-alpha.1 之后该发 alpha.2 还是 0.2.0 是人的决定。

import { spawnSync } from "node:child_process";
import { access, readFile, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?(?:\+[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?$/;

function fail(message) {
  console.error(`release:bump: ${message}`);
  process.exit(1);
}

function readWorkspaceVersion(cargoText) {
  let section = "";
  for (const line of cargoText.split(/\r?\n/)) {
    const header = line.match(/^\s*\[([^\]]+)\]/);
    if (header) {
      section = header[1].trim();
      continue;
    }
    if (section !== "workspace.package") continue;
    const match = line.match(/^\s*version\s*=\s*"([^"]*)"/);
    if (match) return match[1];
  }
  return null;
}

const [rawVersion, ...extra] = process.argv.slice(2);
if (!rawVersion || extra.length > 0) {
  fail("用法：pnpm release:bump <version>，例如 pnpm release:bump 0.2.0-alpha.3");
}
const version = rawVersion.replace(/^v/, "");
if (!SEMVER.test(version)) {
  fail(`版本号不合法：${rawVersion}；写成 0.2.0-alpha.3 这类 semver，脚本不替你猜下一个号`);
}

const root = fileURLToPath(new URL("..", import.meta.url));

const cargoPath = path.join(root, "Cargo.toml");
const cargoText = await readFile(cargoPath, "utf8");
const previous = readWorkspaceVersion(cargoText);
if (!previous) fail("Cargo.toml 里没找到 [workspace.package] 的 version");

const tauriPath = path.join(root, "crates/app/tauri.conf.json");
const tauriConfig = JSON.parse(await readFile(tauriPath, "utf8"));
if (Object.hasOwn(tauriConfig, "version")) {
  fail("crates/app/tauri.conf.json 不要写 version：安装包版本统一取自 Cargo.toml");
}

// 只替换 [workspace.package] 段里的 version，[workspace.dependencies] 里的版本号不碰。
const eol = cargoText.includes("\r\n") ? "\r\n" : "\n";
let section = "";
let replaced = false;
const cargoLines = cargoText.split(/\r?\n/).map((line) => {
  const header = line.match(/^\s*\[([^\]]+)\]/);
  if (header) {
    section = header[1].trim();
    return line;
  }
  if (replaced || section !== "workspace.package") return line;
  const match = line.match(/^(\s*version\s*=\s*")([^"]*)(".*)$/);
  if (!match) return line;
  replaced = true;
  return `${match[1]}${version}${match[3]}`;
});
if (!replaced) fail("Cargo.toml 里没找到 [workspace.package] 的 version 行");

const pkgPath = path.join(root, "package.json");
const pkg = JSON.parse(await readFile(pkgPath, "utf8"));

await writeFile(cargoPath, cargoLines.join(eol), "utf8");
await writeFile(pkgPath, `${JSON.stringify({ ...pkg, version }, null, 2)}\n`, "utf8");

const lock = spawnSync("cargo", ["update", "-w"], { cwd: root, encoding: "utf8" });
if (lock.status !== 0) {
  console.error(lock.stdout ?? "", lock.stderr ?? "");
  fail("cargo update -w 失败：Cargo.lock 没跟上版本，带 --locked 的 CI 会红");
}

const versionInCargo = readWorkspaceVersion(await readFile(cargoPath, "utf8"));
const versionInPackage = JSON.parse(await readFile(pkgPath, "utf8")).version;
if (versionInCargo !== version || versionInPackage !== version) {
  fail(`写回后两处不一致：Cargo.toml=${versionInCargo}，package.json=${versionInPackage}`);
}

const changelogPath = path.join(root, "changelog", `${version}.md`);
let changelogCreated = false;
try {
  await access(changelogPath, constants.F_OK);
} catch {
  await writeFile(
    changelogPath,
    `# ${version}\n\n<!-- 一句话说这个版本交付了什么；按「新增」「改动」「修复」「已知问题」分组，空的组删掉。 -->\n\n## 新增\n\n## 改动\n\n## 修复\n\n## 已知问题\n`,
    "utf8",
  );
  changelogCreated = true;
}

console.log(previous === version ? `版本没变，仍是 ${version}` : `版本：${previous} → ${version}`);
console.log("  Cargo.toml              [workspace.package] version");
console.log("  package.json            version");
console.log("  Cargo.lock              cargo update -w 已同步");
console.log(
  `  changelog/${version}.md  ${changelogCreated ? "模板已建，记得写内容" : "已存在，没有覆盖"}`,
);
console.log("");
console.log("接着做：");
console.log(`  1. 写 changelog/${version}.md`);
console.log("  2. 跑门禁：cargo fmt --all --check、cargo clippy --workspace --all-targets -- -D warnings、cargo test --workspace");
console.log(`  3. Actions 里手动触发 Release 干跑（workflow_dispatch，版本填 ${version}）`);
console.log(`  4. 全绿再打标签：git tag v${version}`);
