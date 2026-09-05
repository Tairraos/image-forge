// templateBundle.spec.js — Web 版模板包导出/导入测试（与桌面版 template_bundle.rs 同构格式）
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { webcrypto } from 'node:crypto';
import JSZip from 'jszip';

const { uploadImageMock } = vi.hoisted(() => ({ uploadImageMock: vi.fn() }));
vi.mock('../../src/api/blob.js', () => ({ uploadImage: uploadImageMock, isLocalDev: () => false }));

import { exportTemplates, importTemplates } from '../../src/api/adapter-web.js';

const PNG_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
const PNG_DATA_URL = `data:image/png;base64,${PNG_BASE64}`;

async function pngBytes() {
  return new Uint8Array(await (await fetch(PNG_DATA_URL)).arrayBuffer());
}

async function sha256Hex(bytes) {
  const hash = await webcrypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(hash))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}

function template(overrides = {}) {
  return {
    id: '7',
    title: '默认标题',
    shortTitle: '',
    category: '常用',
    content: '默认内容',
    referencePaths: [],
    effectImagePath: '',
    notes: '',
    tags: [],
    favorite: false,
    modelHint: '',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function createLocalStorageMock() {
  let store = {};
  return {
    getItem: vi.fn((key) => (key in store ? store[key] : null)),
    setItem: vi.fn((key, value) => {
      store[key] = String(value);
    }),
    removeItem: vi.fn((key) => {
      delete store[key];
    }),
    clear: vi.fn(() => {
      store = {};
    }),
  };
}

const localStorageMock = createLocalStorageMock();
Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock, writable: true });

describe('模板包导出/导入（与桌面版同构）', () => {
  let exportedBlobs;

  beforeEach(() => {
    localStorageMock.clear();
    uploadImageMock.mockReset();
    uploadImageMock.mockImplementation(
      async (fileName) => `/image-forge-data/references/${fileName}`
    );
    exportedBlobs = [];
    URL.createObjectURL = vi.fn((blob) => {
      exportedBlobs.push(blob);
      return `blob:mock-${exportedBlobs.length}`;
    });
    URL.revokeObjectURL = vi.fn();
    vi.stubGlobal('crypto', webcrypto);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('导出的 ZIP 与桌面版结构一致（manifest + markdown + images/<sha256>.png）', async () => {
    localStorage.setItem(
      'if_templates',
      JSON.stringify([
        template({ id: '1', referencePaths: [PNG_DATA_URL], effectImagePath: PNG_DATA_URL }),
        template({
          id: '2',
          title: '第二个',
          content: '另一段内容',
          referencePaths: [PNG_DATA_URL],
        }),
      ])
    );

    const name = await exportTemplates('ImageForge-templates.zip');
    expect(name).toBe('ImageForge-templates.zip');

    const zip = await JSZip.loadAsync(exportedBlobs[0]);
    const names = Object.keys(zip.files).filter((n) => !zip.files[n].dir);
    const imagePath = `images/${await sha256Hex(await pngBytes())}.png`;
    expect(names.sort()).toEqual(['ImageForge-templates.md', 'manifest.json', imagePath].sort());

    const manifest = JSON.parse(await zip.file('manifest.json').async('text'));
    expect(manifest.format).toBe('image-forge-template-bundle');
    expect(manifest.version).toBe(1);
    expect(manifest.templates).toHaveLength(2);
    expect(manifest.templates[0].sourceId).toBe('1');
    expect(manifest.templates[0].content).toBe('默认内容');
    expect(manifest.templates[0].references).toEqual([imagePath]);
    expect(manifest.templates[0].effectImage).toBe(imagePath);
    expect(manifest.templates[1].references).toEqual(manifest.templates[0].references);

    const markdown = await zip.file('ImageForge-templates.md').async('text');
    expect(markdown).toContain('# Image Forge 提示词模板');
    expect(markdown).toContain('## 模板 1 · 默认标题');
    expect(markdown).toContain('### 参考图');
    expect(markdown).toContain(`](${imagePath})`);
  });

  it('导入桌面版格式的 ZIP：校验哈希、按内容寻址转存参考图并分配新 ID', async () => {
    const imageBytes = await pngBytes();
    const hash = await sha256Hex(imageBytes);
    const zip = new JSZip();
    zip.file(
      'manifest.json',
      JSON.stringify({
        format: 'image-forge-template-bundle',
        version: 1,
        exportedAt: '2026-09-05T00:00:00Z',
        templates: [
          {
            sourceId: '9',
            title: '来自桌面',
            content: '桌面内容',
            references: [`images/${hash}.png`],
            effectImage: '',
          },
        ],
      })
    );
    zip.file('ImageForge-templates.md', '# Image Forge 提示词模板\n');
    zip.file(`images/${hash}.png`, imageBytes);
    const blob = await zip.generateAsync({ type: 'blob' });

    const result = await importTemplates(new File([blob], 'templates.zip'));
    expect(result.importedCount).toBe(1);
    expect(result.skippedCount).toBe(0);
    expect(uploadImageMock).toHaveBeenCalledWith(`${hash}.png`, expect.any(Blob), 'references');
    const imported = result.templates[result.templates.length - 1];
    expect(imported.title).toBe('来自桌面');
    expect(imported.content).toBe('桌面内容');
    expect(imported.category).toBe('常用');
    expect(imported.referencePaths).toEqual([`/image-forge-data/references/${hash}.png`]);
    expect(imported.id).not.toBe('9');
    expect(imported.id).toMatch(/^tpl-/);
  });

  it('文件名哈希与内容不一致时拒绝导入', async () => {
    const zip = new JSZip();
    zip.file(
      'manifest.json',
      JSON.stringify({
        format: 'image-forge-template-bundle',
        version: 1,
        exportedAt: 'x',
        templates: [
          {
            sourceId: '9',
            title: 'T',
            content: 'C',
            references: ['images/deadbeef.png'],
            effectImage: '',
          },
        ],
      })
    );
    zip.file('images/deadbeef.png', await pngBytes());
    const blob = await zip.generateAsync({ type: 'blob' });

    await expect(importTemplates(new File([blob], 'templates.zip'))).rejects.toThrow(
      '参考图完整性校验失败'
    );
  });

  it('manifest 引用的图片缺失时拒绝导入', async () => {
    const imageBytes = await pngBytes();
    const hash = await sha256Hex(imageBytes);
    const zip = new JSZip();
    zip.file(
      'manifest.json',
      JSON.stringify({
        format: 'image-forge-template-bundle',
        version: 1,
        exportedAt: 'x',
        templates: [
          {
            sourceId: '9',
            title: 'T',
            content: 'C',
            references: [`images/${hash}.png`],
            effectImage: '',
          },
        ],
      })
    );
    const blob = await zip.generateAsync({ type: 'blob' });
    await expect(importTemplates(new File([blob], 'templates.zip'))).rejects.toThrow(
      '模板包缺少文件'
    );
  });

  it('重复导入同一模板包时按签名跳过', async () => {
    const imageBytes = await pngBytes();
    const hash = await sha256Hex(imageBytes);
    const zip = new JSZip();
    zip.file(
      'manifest.json',
      JSON.stringify({
        format: 'image-forge-template-bundle',
        version: 1,
        exportedAt: 'x',
        templates: [
          {
            sourceId: '9',
            title: '同一条',
            content: '同一段内容',
            references: [`images/${hash}.png`],
            effectImage: '',
          },
        ],
      })
    );
    zip.file(`images/${hash}.png`, imageBytes);
    const blob = await zip.generateAsync({ type: 'blob' });

    const first = await importTemplates(new File([blob], 'templates.zip'));
    expect(first.importedCount).toBe(1);
    const second = await importTemplates(new File([blob], 'templates.zip'));
    expect(second.importedCount).toBe(0);
    expect(second.skippedCount).toBe(1);
  });

  it('兼容旧版 Web 导出包（无 format 字段，prompt 字段）', async () => {
    const zip = new JSZip();
    zip.file(
      'manifest.json',
      JSON.stringify({
        version: 1,
        exportedAt: 'x',
        templates: [
          {
            id: 'x1',
            title: '旧包',
            prompt: '旧内容',
            referencePaths: ['/old/ref.png'],
            effectImagePath: '',
          },
        ],
      })
    );
    zip.file('images/ref.png', await pngBytes());
    const blob = await zip.generateAsync({ type: 'blob' });

    const result = await importTemplates(new File([blob], 'templates.zip'));
    expect(result.importedCount).toBe(1);
    const imported = result.templates[result.templates.length - 1];
    expect(imported.content).toBe('旧内容');
    expect(imported.referencePaths).toHaveLength(1);
  });

  it('兼容旧版 Markdown ZIP（无 manifest）', async () => {
    const zip = new JSZip();
    zip.file(
      'ImageForge-templates.md',
      '# Image Forge 提示词模板\n\n> 导出时间：x\n> 模板数量：1\n\n---\n\n## 模板 3 · \n\n只有 Markdown 的旧模板\n\n### 参考图\n\n![模板 3 参考图 1](images/pic.png)\n\n'
    );
    zip.file('images/pic.png', await pngBytes());
    const blob = await zip.generateAsync({ type: 'blob' });

    const result = await importTemplates(new File([blob], 'templates.zip'));
    expect(result.importedCount).toBe(1);
    const imported = result.templates[result.templates.length - 1];
    expect(imported.content).toBe('只有 Markdown 的旧模板');
    expect(imported.referencePaths).toHaveLength(1);
  });

  it('缺少 manifest 和 Markdown 的 ZIP 被拒绝', async () => {
    const zip = new JSZip();
    zip.file('other.txt', 'hello');
    const blob = await zip.generateAsync({ type: 'blob' });
    await expect(importTemplates(new File([blob], 'templates.zip'))).rejects.toThrow(
      '模板包缺少 manifest.json 或 ImageForge-templates.md'
    );
  });
});
