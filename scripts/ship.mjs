#!/usr/bin/env node
// 一键发布：patch → build → cargo check → prerelease
// 用法：pnpm ship <next-version>
// 示例：pnpm ship 1.0.85

import { execSync } from 'node:child_process';

const version = process.argv[2];
if (!version) {
  console.error('用法：pnpm ship <next-version>');
  console.error('示例：pnpm ship 1.0.85');
  process.exit(1);
}

function run(cmd, label) {
  console.log(`\n  → ${label}...`);
  execSync(cmd, { stdio: 'inherit', cwd: process.cwd() });
}

try {
  run(`pnpm run patch -- ${version}`, '升级 patch 版本');
  run('pnpm build', '构建前端');
  run('cargo check --manifest-path src-tauri/Cargo.toml', 'Rust 类型检查');
  run('pnpm run prerelease', '生成 prerelease .app');
  console.log(`\n  ✓ 发布完成：${version}`);
} catch (err) {
  console.error(`\n  ✗ 发布失败：${err.message || err}`);
  process.exit(1);
}
