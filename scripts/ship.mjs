// 一键发布：verify → patch → cargo check → build（打包本地 .app，不出 .dmg）
// 用法：pnpm ship <next-version>
// 示例：pnpm ship 1.0.88

import { execFileSync } from 'node:child_process';
import { assertVersion, root } from './patch-version.mjs';

const version = process.argv[2];
if (!version) {
  console.error('用法：pnpm ship <next-version>');
  console.error('示例：pnpm ship 1.0.88');
  process.exit(1);
}
// 先校验版本号格式，再交给子进程执行
assertVersion(version);

function run(cmd, args, label) {
  console.log(`\n  → ${label}...`);
  execFileSync(cmd, args, { stdio: 'inherit', cwd: root });
}

try {
  run('pnpm', ['verify'], '验证（lint + 格式 + 测试 + Rust check）');
  run('pnpm', ['run', 'patch', '--', version], '升级 patch 版本');
  run('cargo', ['check', '--manifest-path', 'src-tauri/Cargo.toml'], 'Rust 类型检查');
  run('pnpm', ['build'], '打包本地 App（.app）并清理环境');
  console.log(`\n  ✓ 发布完成：${version}`);
} catch (err) {
  console.error(`\n  ✗ 发布失败：${err.message || err}`);
  process.exit(1);
}
