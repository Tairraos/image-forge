#!/usr/bin/env node
// 统一验证入口：完成任务后一次跑完主要检查。
// 用法：
//   pnpm verify            # lint + 格式 + 测试 + Rust check
//   pnpm verify --full     # 额外跑 Rust 测试（较慢）
//   pnpm verify --rust     # 只跑 Rust 侧检查（check + test）

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const args = process.argv.slice(2);
const full = args.includes('--full');
const rustOnly = args.includes('--rust');

// 每个检查项：label + 命令 + 参数
const steps = [];

if (!rustOnly) {
  steps.push({ label: 'ESLint', cmd: 'pnpm', args: ['lint'] });
  steps.push({ label: 'Prettier 格式检查', cmd: 'pnpm', args: ['fmt:check'] });
  steps.push({ label: 'Vitest 测试', cmd: 'pnpm', args: ['test'] });
}

steps.push({
  label: 'Rust cargo check',
  cmd: 'cargo',
  args: ['check', '--manifest-path', 'src-tauri/Cargo.toml'],
});
if (full) {
  steps.push({
    label: 'Rust cargo test',
    cmd: 'cargo',
    args: ['test', '--manifest-path', 'src-tauri/Cargo.toml'],
  });
}

let failed = false;
const startedAt = Date.now();

for (const step of steps) {
  console.log(`\n==> ${step.label} ...`);
  const result = spawnSync(step.cmd, step.args, { cwd: root, stdio: 'inherit' });
  if (result.status !== 0) {
    console.error(`\n✗ ${step.label} 失败（exit ${result.status}）`);
    failed = true;
    break;
  }
}

const elapsed = ((Date.now() - startedAt) / 1000).toFixed(1);
if (failed) {
  console.error(`\n验证未通过，用时 ${elapsed}s。请修复上面的失败项后重跑 pnpm verify。`);
  process.exit(1);
}
console.log(`\n✓ 全部验证通过，用时 ${elapsed}s。`);
