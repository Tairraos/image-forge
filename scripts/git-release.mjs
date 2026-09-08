#!/usr/bin/env node
// 一键云端发布：升级版本（可选）→ commit → 打 tag → push，触发 GitHub Actions 三平台构建并自动发布 Release。
// 用法：
//   pnpm release            # 用项目当前版本发布（要求工作区干净、该版本 tag 不存在）
//   pnpm release 1.0.104    # 先把版本升到 1.0.104，再 commit、打 tag、push
// 说明：push 分支会触发云端 pnpm verify（ci.yml），push tag 会触发三平台构建与自动发布（release.yml）。

import { execFileSync } from 'node:child_process';
import { currentVersion, patchVersion, root } from './patch-version.mjs';

const requestedVersion = process.argv
  .slice(2)
  .find((arg) => arg !== '--')
  ?.trim();
const remote = 'origin';

function git(args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
}

function fail(message) {
  console.error(`\n✗ ${message}`);
  process.exit(1);
}

function repoWebUrl() {
  const url = git(['remote', 'get-url', remote]);
  const match = url.match(/github\.com[:/](.+?)(?:\.git)?$/);
  return match ? `https://github.com/${match[1]}` : url;
}

// —— 前置检查：git 仓库、分支、远端、干净工作区 ——
try {
  git(['rev-parse', '--git-dir']);
} catch {
  fail('当前目录不是 git 仓库');
}

const branch = git(['rev-parse', '--abbrev-ref', 'HEAD']);
if (branch === 'HEAD') fail('当前处于 detached HEAD 状态，请先切回分支再发布');

try {
  git(['remote', 'get-url', remote]);
} catch {
  fail(`没有配置远端 ${remote}，无法推送`);
}

const dirty = git(['status', '--porcelain']);
if (dirty) {
  console.error(`工作区存在未提交改动：\n${dirty}`);
  fail('请先提交（或 git stash 暂存）后再执行 pnpm release');
}

const version = requestedVersion || currentVersion();

// tag 冲突检查（本地 + 远端），避免重复发布同一版本
if (git(['tag', '--list', version])) {
  fail(
    `本地已存在 tag ${version}。如确要重发该版本，先执行 git tag -d ${version}，或指定更高的版本号`
  );
}
try {
  if (git(['ls-remote', '--tags', remote, `refs/tags/${version}`])) {
    fail(`远端 ${remote} 已存在 tag ${version}，请指定更高的版本号`);
  }
} catch {
  fail(`无法连接远端 ${remote}，请检查网络后重试`);
}

// —— 升级版本并提交（仅当带版本参数时）——
if (requestedVersion) {
  console.log(`\n→ 升级版本到 ${requestedVersion} ...`);
  try {
    patchVersion(requestedVersion); // 内部校验版本号格式，且必须高于当前版本
  } catch (error) {
    fail(error.message);
  }
  // 此前已确认工作区干净，此时的改动全部来自版本号升级
  const changed = git(['status', '--porcelain']);
  if (!changed) fail('版本升级后没有产生任何文件改动，请检查版本号');
  console.log(changed);
  git(['add', '-A']);
  try {
    git(['commit', '-m', `chore(release): 发布 ${requestedVersion}`]);
  } catch {
    fail('git commit 失败（可能被 pre-commit 钩子拒绝），请检查上方输出后重试');
  }
}

// —— 打 tag 并推送（沿用现有裸版本号 tag 习惯，轻量 tag）——
git(['tag', version]);
console.log(`\n→ 推送 ${branch} 和 tag ${version} ...`);
try {
  git(['push', remote, branch]);
  git(['push', remote, `refs/tags/${version}`]);
} catch {
  fail(
    `推送失败；本地 commit 与 tag ${version} 已保留，修复网络/权限后可手动执行：\n` +
      `   git push ${remote} ${branch} && git push ${remote} refs/tags/${version}`
  );
}

const web = repoWebUrl();
console.log(`
✓ 已触发发布 ${version}：
   构建进度：${web}/actions
   Release 页（构建完成后自动公开）：${web}/releases
`);
