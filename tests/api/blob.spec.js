// blob.spec.js — 图片存储层测试
// 测试 isLocalDev、uploadImage、downloadImage、deleteImage、isBlobUrl 等函数。

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

// mock 掉 @vercel/blob/client：上传走服务端签发短时令牌后直传的流程
vi.mock('@vercel/blob/client', () => ({ upload: vi.fn() }));

// 先 stub 环境变量，再导入模块
vi.stubEnv('VITE_DEV_PORT', '');

const { uploadImage, downloadImage, deleteImage, isBlobUrl, devUrlToAbsPath } =
  await import('../../src/api/blob.js');
const { upload: blobClientUpload } = await import('@vercel/blob/client');

const originalLocation = { ...window.location };

beforeEach(() => {
  blobClientUpload.mockReset();
});

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

describe('uploadImage — 部署的 Web 版（服务端代理）', () => {
  it('非本地开发时经短时令牌直传 Blob，返回远端 URL', async () => {
    setLocation({ port: '3000', origin: 'http://localhost:3000' });
    blobClientUpload.mockResolvedValueOnce({
      url: 'https://xxx.public.blob.vercel-storage.com/cat-abc123.png',
    });

    const fakeBlob = new Blob(['test'], { type: 'image/png' });
    const result = await uploadImage('cat.png', fakeBlob, 'tasks');

    expect(result).toBe('https://xxx.public.blob.vercel-storage.com/cat-abc123.png');
    expect(blobClientUpload).toHaveBeenCalledTimes(1);
    expect(blobClientUpload).toHaveBeenCalledWith(
      'cat.png',
      expect.any(Blob),
      expect.objectContaining({
        access: 'public',
        handleUploadUrl: '/api/blob/upload',
        contentType: 'image/png',
      })
    );
  });

  it('上传失败时抛出带部署提示的错误', async () => {
    setLocation({ port: '3000', origin: 'http://localhost:3000' });
    blobClientUpload.mockRejectedValue(new Error('fetch failed'));

    const fakeBlob = new Blob(['test'], { type: 'image/png' });
    await expect(uploadImage('cat.png', fakeBlob)).rejects.toThrow('Blob 上传失败');
    await expect(uploadImage('cat.png', fakeBlob)).rejects.toThrow('/api/blob/upload');
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

  it('Blob URL 走服务端删除 API', async () => {
    setLocation({ port: '3000', origin: 'http://localhost:3000' });
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true }));

    await deleteImage('https://xxx.public.blob.vercel-storage.com/cat-abc123.png');
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0][0]).toBe('/api/blob/delete');
    expect(fetch.mock.calls[0][1].method).toBe('POST');
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({
      urls: ['https://xxx.public.blob.vercel-storage.com/cat-abc123.png'],
    });
  });

  it('非 Blob 的 https URL 不发任何请求', async () => {
    setLocation({ port: '3000', origin: 'http://localhost:3000' });
    vi.stubGlobal('fetch', vi.fn());

    await deleteImage('https://example.com/image.png');
    expect(fetch).not.toHaveBeenCalled();
  });
});
