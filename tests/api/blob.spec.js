// blob.spec.js — 图片存储层测试
// 测试 isLocalDev、uploadImage、downloadImage、deleteImage、isBlobUrl 等函数。

import { describe, expect, it, vi, afterEach } from 'vitest';

// 先 stub 环境变量，再导入模块
vi.stubEnv('VITE_BLOB_READ_WRITE_TOKEN', '');
vi.stubEnv('VITE_DEV_PORT', '');

const { uploadImage, downloadImage, deleteImage, isBlobUrl, devUrlToAbsPath } =
  await import('../../src/api/blob.js');

const originalLocation = { ...window.location };

afterEach(() => {
  Object.defineProperty(window, 'location', {
    value: originalLocation,
    writable: true,
  });
  vi.restoreAllMocks();
});

function setLocation(overrides = {}) {
  Object.defineProperty(window, 'location', {
    value: {
      protocol: 'http:',
      hostname: 'localhost',
      port: '1421',
      origin: 'http://localhost:1421',
      ...overrides,
    },
    writable: true,
  });
}

describe('isBlobUrl', () => {
  it('识别 Vercel Blob URL', () => {
    expect(isBlobUrl('https://xxx.public.blob.vercel-storage.com/cat.png')).toBe(true);
  });

  it('非 Vercel Blob URL 返回 false', () => {
    expect(isBlobUrl('https://example.com/image.png')).toBe(false);
    expect(isBlobUrl('/image-forge-data/tasks/cat.png')).toBe(false);
    expect(isBlobUrl('')).toBe(false);
    expect(isBlobUrl(undefined)).toBe(false);
  });
});

describe('devUrlToAbsPath', () => {
  it('非 /image-forge-data URL 返回 null', () => {
    expect(devUrlToAbsPath('https://example.com')).toBeNull();
    expect(devUrlToAbsPath('')).toBeNull();
    expect(devUrlToAbsPath(undefined)).toBeNull();
  });
});

describe('uploadImage — 本地开发模式', () => {
  it('本地开发时将图片 POST 到 /image-forge-data，返回 dev URL', async () => {
    setLocation();
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ url: '/image-forge-data/tasks/cat.png', size: 1024 }),
      })
    );

    const fakeBlob = new Blob(['test'], { type: 'image/png' });
    const result = await uploadImage('cat.png', fakeBlob, 'tasks');

    expect(result).toBe('/image-forge-data/tasks/cat.png');
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0][0]).toContain('/image-forge-data/tasks/cat.png');
    expect(fetch.mock.calls[0][1].method).toBe('POST');
  });

  it('上传失败时抛出错误', async () => {
    setLocation();
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
        text: () => Promise.resolve('disk full'),
      })
    );

    const fakeBlob = new Blob(['test'], { type: 'image/png' });
    await expect(uploadImage('cat.png', fakeBlob, 'tasks')).rejects.toThrow(
      '写入 ~/.image-forge 失败'
    );
  });
});

describe('uploadImage — 非本地开发且无 token', () => {
  it('非本地开发且无 token 时抛出错误', async () => {
    setLocation({ port: '3000', origin: 'http://localhost:3000' });

    const fakeBlob = new Blob(['test'], { type: 'image/png' });
    await expect(uploadImage('cat.png', fakeBlob)).rejects.toThrow(
      'Web 版未配置 VITE_BLOB_READ_WRITE_TOKEN'
    );
  });
});

describe('downloadImage', () => {
  it('下载成功返回 Blob', async () => {
    const fakeBlob = new Blob(['image data'], { type: 'image/png' });
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        blob: () => Promise.resolve(fakeBlob),
      })
    );

    const result = await downloadImage('https://example.com/image.png');
    expect(result).toBe(fakeBlob);
  });

  it('下载失败时抛出错误', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
      })
    );

    await expect(downloadImage('https://example.com/image.png')).rejects.toThrow(
      '图片下载失败: 404'
    );
  });
});

describe('deleteImage', () => {
  it('url 为空时直接返回', async () => {
    vi.stubGlobal('fetch', vi.fn());
    await deleteImage('');
    expect(fetch).not.toHaveBeenCalled();
  });

  it('本地开发时对 /image-forge-data URL 发送 DELETE', async () => {
    setLocation();
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true }));

    await deleteImage('/image-forge-data/tasks/cat.png');
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0][1].method).toBe('DELETE');
  });
});
