import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
} from "node:fs";
import { join } from "node:path";
import { currentVersion, patchVersion, readJson, root } from "./patch-version.mjs";

process.chdir(root);

const requestedVersion = process.argv.slice(2).find((arg) => arg !== "--")?.trim();
if (requestedVersion) patchVersion(requestedVersion);

const version = currentVersion();
const tauriConfig = readJson("src-tauri/tauri.conf.json");
const productName = tauriConfig.productName;
const bundleDir = join(root, "src-tauri", "target", "release", "bundle");
const appPath = join(bundleDir, "macos", `${productName}.app`);
const releaseDir = join(root, "release");
const outputPath = join(releaseDir, `${productName}-${version}-${archName()}.app`);

try {
  moveToTrash(bundleDir);
  cleanIcons();
  run("pnpm", ["tauri", "icon", "src-tauri/icons/app-icon.png"]);
  run("pnpm", ["tauri", "build", "--bundles", "app"]);
  if (!existsSync(appPath)) throw new Error("没有生成 macOS .app 产物");
  run("codesign", ["--force", "--deep", "--sign", "-", appPath]);

  moveReleaseContentsToTrash();
  mkdirSync(releaseDir, { recursive: true });
  movePath(appPath, outputPath);
  console.log(`\nPrerelease App 已生成：\n${outputPath}`);
} finally {
  cleanProcessFiles();
}

function run(command, args) {
  console.log(`\n$ ${[command, ...args].join(" ")}`);
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed`);
}

function archName() {
  if (process.arch === "arm64") return "aarch64";
  if (process.arch === "x64") return "x64";
  return process.arch;
}

function moveReleaseContentsToTrash() {
  if (!existsSync(releaseDir)) return;
  const entries = readdirSync(releaseDir, { withFileTypes: true });
  if (!entries.length) return;
  const trashDir = join(process.env.HOME || root, ".Trash");
  mkdirSync(trashDir, { recursive: true });
  for (const entry of entries) {
    const source = join(releaseDir, entry.name);
    const target = uniqueTrashPath(trashDir, entry.name);
    renameSync(source, target);
    console.log(`已移到回收站：${source}`);
  }
}

function uniqueTrashPath(trashDir, name) {
  let candidate = join(trashDir, name);
  let index = 1;
  while (existsSync(candidate)) {
    candidate = join(trashDir, `${name}-${Date.now()}-${index}`);
    index += 1;
  }
  return candidate;
}

function movePath(from, to) {
  try {
    renameSync(from, to);
  } catch (error) {
    if (error.code !== "EXDEV") throw error;
    cpSync(from, to, { recursive: true });
    moveToTrash(from);
  }
}

function cleanIcons() {
  const iconDir = join(root, "src-tauri", "icons");
  for (const entry of readdirSync(iconDir)) {
    if (!["app-icon.png", "icon.png"].includes(entry)) {
      moveToTrash(join(iconDir, entry));
    }
  }
}

function cleanProcessFiles() {
  moveToTrash(join(root, "dist"));
  moveToTrash(join(root, "src-tauri", "target"));
  moveToTrash(join(root, "src-tauri", "gen"));
  cleanIcons();
}

function moveToTrash(path) {
  if (!existsSync(path)) return;
  const result = spawnSync("trash", [path], { cwd: root, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`无法将路径移入系统回收站：${path}`);
  }
}
