import { spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const home = homedir();
const oldRoot = process.env.IMAGE_FORGE_OLD_ROOT
  || join(home, "Library", "Application Support", "com.xiaole.imageforge");
const newRoot = join(home, ".image-forge");
const stagingRoot = `${newRoot}.migrating`;

if (!existsSync(oldRoot)) throw new Error(`找不到旧数据目录：${oldRoot}`);
if (existsSync(newRoot)) throw new Error(`新数据目录已存在，请先人工检查：${newRoot}`);
if (existsSync(stagingRoot)) throw new Error(`发现未完成的迁移临时目录：${stagingRoot}`);

mkdirSync(stagingRoot, { recursive: true });
try {
  cpSync(oldRoot, stagingRoot, { recursive: true, force: true });
  rewriteJsonPaths(stagingRoot, oldRoot, newRoot);
  ensureLayout(stagingRoot);
  verify(stagingRoot);
  renameSync(stagingRoot, newRoot);
  moveToTrash(oldRoot);
  securePermissions(newRoot);
  console.log(`迁移完成：${oldRoot} -> ${newRoot}`);
} catch (error) {
  moveToTrash(stagingRoot);
  throw error;
}

function moveToTrash(path) {
  if (!existsSync(path)) return;
  const result = spawnSync("trash", [path], { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`无法将路径移入系统回收站：${path}`);
  }
}

function rewriteJsonPaths(root, oldPrefix, newPrefix) {
  for (const path of walk(root)) {
    if (path.endsWith(".json")) {
      const value = JSON.parse(readFileSync(path, "utf8"));
      const rewritten = replaceStrings(value, oldPrefix, newPrefix);
      writeFileSync(path, `${JSON.stringify(rewritten, null, 2)}\n`);
    }
  }
}

function ensureLayout(root) {
  for (const name of ["outputs", "requests", "clipboard", "references"]) {
    mkdirSync(join(root, name), { recursive: true });
  }
}

function verify(root) {
  for (const name of ["settings.json", "history.json", "queue.json", "prompt-templates.json"]) {
    const path = join(root, name);
    if (existsSync(path)) JSON.parse(readFileSync(path, "utf8"));
  }
}

function* walk(root) {
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) yield* walk(path);
    else yield path;
  }
}

function replaceStrings(value, oldPrefix, newPrefix) {
  if (typeof value === "string") {
    return value.startsWith(oldPrefix) ? `${newPrefix}${value.slice(oldPrefix.length)}` : value;
  }
  if (Array.isArray(value)) return value.map((item) => replaceStrings(item, oldPrefix, newPrefix));
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [
      key,
      replaceStrings(item, oldPrefix, newPrefix),
    ]));
  }
  return value;
}

function securePermissions(root) {
  for (const path of walkDirectories(root)) chmodSync(path, 0o700);
  for (const path of walk(root)) chmodSync(path, 0o600);
  chmodSync(root, 0o700);
}

function* walkDirectories(root) {
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      yield path;
      yield* walkDirectories(path);
    }
  }
}
