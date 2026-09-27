#!/usr/bin/env node
// 打包本地 App：可选升版本 → pnpm tauri build（前端 vite build + Rust 编译）→ ad-hoc 签名 .app（默认不出 .dmg）
// → 产物统一收进 release/（旧版本产物移入系统回收站，只留当前版本）→ 收尾清理：dist/、src-tauri/gen/
// 与生成的图标移入回收站；src-tauri/target/ 只保留 release/ 作 Rust 增量编译缓存（由用户手动清理）。
// 用法：
//   pnpm build            # 用项目当前版本打包（只出 .app，不出 .dmg）
//   pnpm build 1.0.104    # 先升版本到 1.0.104 再打包
//   pnpm build --dmg      # 额外生成 .dmg，可与版本参数同用

import { spawnSync } from 'node:child_process';
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  renameSync,
  symlinkSync,
} from 'node:fs';
import { basename, join } from 'node:path';
import { tmpdir } from 'node:os';
import { currentVersion, patchVersion, readJson, root } from './patch-version.mjs';

process.chdir(root);

const rawArgs = process.argv.slice(2).map((arg) => arg.trim());
const withDmg = rawArgs.includes('--dmg') || rawArgs.includes('--dmg=true');
const unknownFlags = rawArgs.filter(
  (arg) => arg.startsWith('--') && arg !== '--' && !['--dmg', '--dmg=true'].includes(arg)
);
if (unknownFlags.length) {
  console.error(`未知参数: ${unknownFlags.join(' ')}（可用参数：<版本号>、--dmg、--dmg=true）`);
  process.exit(1);
}
const requestedVersion = rawArgs.find((arg) => arg !== '--' && !arg.startsWith('--')) ?? '';
if (requestedVersion) patchVersion(requestedVersion);

if (process.platform !== 'darwin') {
  console.error('本地打包目前仅支持 macOS（tauri.conf.json 的 bundle targets 只配置了 app）。');
  console.error('Windows / Linux 安装包请用 pnpm release 走云端三平台构建。');
  process.exit(1);
}
// 硬依赖系统 trash 命令做回收站式清理；缺失时给出安装提示而不是构建到一半才报错
if (spawnSync('trash', ['-v'], { stdio: 'ignore' }).error?.code === 'ENOENT') {
  console.error('缺少 trash 命令（构建清理依赖它把临时文件移入系统回收站）：');
  console.error('  brew install trash');
  process.exit(1);
}

const version = currentVersion();
const packageJson = readJson('package.json');
const tauriConfig = readJson('src-tauri/tauri.conf.json');
const productName = tauriConfig.productName;
const bundleDir = join(root, 'src-tauri', 'target', 'release', 'bundle');
const releaseDir = join(root, 'release');

try {
  moveToTrash(bundleDir);
  cleanIcons();
  run('pnpm', ['tauri', 'icon', 'src-tauri/icons/app-icon.png']);

  try {
    run('pnpm', ['tauri', 'build']);
  } catch (error) {
    // 构建失败时不收集半成品：bundle 目录整体进回收站，
    // 避免不完整的 .app 被收进 release/ 顶掉上一个可用版本
    moveToTrash(bundleDir);
    throw error;
  }

  if (process.platform === 'darwin' && macAppPath()) prepareMacBundles();

  const outputs = collectReleaseFiles();
  assertExpectedOutputs(outputs);
  moveOldReleaseBundlesToTrash(outputs);
  console.log('\n发布包已生成：');
  for (const output of outputs) console.log(output);
} finally {
  try {
    cleanProcessFiles();
  } catch (cleanupError) {
    // 收尾失败不得吞掉原始构建错误：保留现场并提示，退出码仍由主流程决定
    console.warn(
      `构建收尾清理失败（文件可能留在原位，可手动清理）: ${cleanupError.message || cleanupError}`
    );
  }
}

function run(command, args, options = {}) {
  console.log(`\n$ ${[command, ...args].join(' ')}`);
  const result = spawnSync(command, args, { cwd: options.cwd || root, stdio: 'inherit' });
  if (result.status !== 0) throw new Error(`${command} ${args.join(' ')} failed`);
}

function archName() {
  if (process.arch === 'arm64') return 'aarch64';
  if (process.arch === 'x64') return 'x64';
  return process.arch;
}

function macAppPath() {
  const path = join(bundleDir, 'macos', `${productName}.app`);
  return existsSync(path) ? path : '';
}

function createSimpleDmg() {
  const appPath = macAppPath();
  if (!appPath) return;
  const dmgDir = join(bundleDir, 'dmg');
  const dmgPath = join(dmgDir, `${productName}_${version}_${archName()}.dmg`);
  const stagingDir = mkdtempSync(join(tmpdir(), `${packageJson.name}-dmg-`));

  try {
    mkdirSync(dmgDir, { recursive: true });
    cpSync(appPath, join(stagingDir, `${productName}.app`), { recursive: true });
    symlinkSync('/Applications', join(stagingDir, 'Applications'));
    run('hdiutil', [
      'create',
      '-volname',
      productName,
      '-srcfolder',
      stagingDir,
      '-ov',
      '-format',
      'UDZO',
      dmgPath,
    ]);
  } finally {
    moveToTrash(stagingDir);
  }
}

function prepareMacBundles() {
  const appPath = macAppPath();
  run('codesign', ['--force', '--deep', '--sign', '-', appPath]);
  if (!withDmg) return;
  moveToTrash(join(bundleDir, 'dmg'));
  createSimpleDmg();
}

function collectReleaseFiles() {
  mkdirSync(releaseDir, { recursive: true });
  const outputs = [];

  const appPath = macAppPath();
  if (appPath) outputs.push(movePath(appPath, releaseName('.app')));

  for (const dmgFile of findFiles(join(bundleDir, 'dmg'), '.dmg')) {
    if (!basename(dmgFile).startsWith('rw.')) outputs.push(movePath(dmgFile, releaseName('.dmg')));
  }

  return outputs;
}

function releaseName(extension, suffix = '') {
  const parts = [productName, version, archName()];
  if (suffix) parts.push(suffix);
  return join(releaseDir, `${parts.join('-')}${extension}`);
}

function movePath(from, to) {
  moveToTrash(to);
  try {
    renameSync(from, to);
  } catch (error) {
    if (error.code !== 'EXDEV') throw error;
    cpSync(from, to, { recursive: true });
    moveToTrash(from);
  }
  return to;
}

// 新版本发布成功后，只保留当前产物，旧 app/dmg 移入系统回收站。
function moveOldReleaseBundlesToTrash(currentOutputs) {
  const current = new Set(currentOutputs.map((file) => basename(file)));
  const bundlePattern = new RegExp(`^${productName}-.*-.*\\.(app|dmg)$`);
  const oldBundles = readdirSync(releaseDir, { withFileTypes: true })
    .filter((entry) => bundlePattern.test(entry.name))
    .filter((entry) => !current.has(entry.name))
    .map((entry) => join(releaseDir, entry.name));
  if (!oldBundles.length) return;
  const trashDir = join(process.env.HOME || root, '.Trash');
  mkdirSync(trashDir, { recursive: true });
  for (const bundle of oldBundles) {
    const target = uniqueTrashPath(trashDir, basename(bundle));
    renameSync(bundle, target);
    console.log(`已移到回收站：${bundle}`);
  }
}

function uniqueTrashPath(trashDir, name) {
  const dotIndex = name.lastIndexOf('.');
  const stem = dotIndex > 0 ? name.slice(0, dotIndex) : name;
  const extension = dotIndex > 0 ? name.slice(dotIndex) : '';
  let candidate = join(trashDir, name);
  let index = 1;
  while (existsSync(candidate)) {
    candidate = join(trashDir, `${stem}-${Date.now()}-${index}${extension}`);
    index += 1;
  }
  return candidate;
}

function findFiles(dir, extension) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(dir, entry.name);
      return entry.isDirectory() ? findFiles(path, extension) : [path];
    })
    .filter((path) => path.endsWith(extension))
    .sort();
}

function assertExpectedOutputs(outputs) {
  if (process.platform === 'darwin' && !outputs.some((file) => file.endsWith('.app'))) {
    throw new Error('macOS 发布包必须包含 .app');
  }
  if (process.platform === 'darwin' && withDmg && !outputs.some((file) => file.endsWith('.dmg'))) {
    throw new Error('macOS 发布包必须包含 .dmg');
  }
}

function cleanIcons() {
  const iconDir = join(root, 'src-tauri', 'icons');
  for (const entry of readdirSync(iconDir)) {
    if (!['app-icon.png', 'icon.png'].includes(entry)) {
      moveToTrash(join(iconDir, entry));
    }
  }
}

function cleanProcessFiles() {
  moveToTrash(join(root, 'dist'));
  moveToTrash(join(root, 'src-tauri', 'gen'));
  cleanIcons();
  cleanTargetExceptRelease();
}

// target/release 保留为 Rust 增量编译缓存（由用户手动清理），
// 其余对增量编译没有帮助的构建过程文件在编译结束后立即移入回收站。
function cleanTargetExceptRelease() {
  const targetDir = join(root, 'src-tauri', 'target');
  if (!existsSync(targetDir)) return;
  for (const entry of readdirSync(targetDir, { withFileTypes: true })) {
    if (entry.name === 'release') continue;
    moveToTrash(join(targetDir, entry.name));
  }
  moveToTrash(join(targetDir, 'release', 'bundle'));
}

function moveToTrash(path) {
  if (!existsSync(path)) return;
  const result = spawnSync('trash', [path], { cwd: root, stdio: 'inherit' });
  if (result.status !== 0) {
    throw new Error(`无法将路径移入系统回收站：${path}`);
  }
}
