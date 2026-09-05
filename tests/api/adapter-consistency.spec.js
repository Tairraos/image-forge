// adapter-consistency.spec.js — 适配器导出一致性检查
// 确保 src/api/index.js 解构导出的每个函数，在两个 adapter 中都真实存在。
// 这是把「新增 adapter 函数必须同步两个 adapter + index.js」转成机器检查。

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

function read(rel) {
  return readFileSync(resolve(root, rel), 'utf8');
}

// 从 index.js 提取 `export const { ... } = adapter` 里的名字列表
function indexExports() {
  const source = read('src/api/index.js');
  const match = source.match(/export const \{([\s\S]*?)\} = adapter;/);
  if (!match) throw new Error('index.js 中未找到 export const { ... } = adapter');
  return match[1]
    .split(',')
    .map((name) => name.trim())
    .filter(Boolean);
}

// 提取某个模块里所有 `export ... name` 的导出名
function moduleExports(rel) {
  const source = read(rel);
  const names = new Set();
  const re = /export\s+(?:async\s+)?(?:const|function|class)\s+(\w+)/g;
  let m;
  while ((m = re.exec(source)) !== null) names.add(m[1]);
  return names;
}

describe('适配器导出一致性', () => {
  const expected = indexExports();
  const tauri = moduleExports('src/api/adapter-tauri.js');
  const web = moduleExports('src/api/adapter-web.js');

  it('index.js 导出的函数数应与 adapter 导出对齐', () => {
    expect(expected.length).toBeGreaterThan(30);
  });

  it('每个函数在 adapter-tauri.js 中都存在', () => {
    const missing = expected.filter((name) => !tauri.has(name));
    expect(missing).toEqual([]);
  });

  it('每个函数在 adapter-web.js 中都存在', () => {
    const missing = expected.filter((name) => !web.has(name));
    expect(missing).toEqual([]);
  });

  it('两个 adapter 不应导出 index.js 未解构的额外函数', () => {
    const expectedSet = new Set(expected);
    const tauriExtra = [...tauri].filter((name) => !expectedSet.has(name));
    const webExtra = [...web].filter((name) => !expectedSet.has(name));
    // onAgentEvent / onQueueChange 是 Web 版独有，允许存在于 web 但 tauri 不能有多余
    expect(tauriExtra).toEqual([]);
    expect(webExtra.filter((n) => !['onAgentEvent', 'onQueueChange'].includes(n))).toEqual([]);
  });
});
