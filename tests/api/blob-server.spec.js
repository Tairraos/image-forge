// blob-server.spec.js — Vercel Functions（api/blob/*）的请求 wiring 测试。
// 用 Node 环境跑（handler 依赖 Buffer / EventEmitter，无 DOM 依赖）。

// @vitest-environment node

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { EventEmitter } from 'node:events';

vi.mock('@vercel/blob/client', () => ({ handleUpload: vi.fn() }));
vi.mock('@vercel/blob', () => ({ del: vi.fn() }));

const uploadHandler = (await import('../../api/blob/upload.js')).default;
const deleteHandler = (await import('../../api/blob/delete.js')).default;
const { handleUpload } = await import('@vercel/blob/client');
const { del } = await import('@vercel/blob');

function makeReq({ method = 'POST', headers = {}, body = '' } = {}) {
  const req = new EventEmitter();
  req.method = method;
  req.headers = headers;
  return {
    req,
    send() {
      if (body) req.emit('data', Buffer.from(body));
      req.emit('end');
    },
  };
}

function makeRes() {
  return {
    statusCode: 0,
    payload: undefined,
    status(code) {
      this.statusCode = code;
      return this;
    },
    json(payload) {
      this.payload = payload;
      return this;
    },
  };
}

beforeEach(() => {
  handleUpload.mockReset();
  del.mockReset();
  vi.stubEnv('BLOB_READ_WRITE_TOKEN', 'test-token');
});

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('api/blob/upload', () => {
  it('解析请求体并转发给 handleUpload，回 200 与结果', async () => {
    handleUpload.mockResolvedValueOnce({
      type: 'blob.generate-client-token',
      clientToken: 'signed-token',
    });
    const { req, send } = makeReq({
      headers: { host: 'app.example.com' },
      body: JSON.stringify({ type: 'blob.generate-client-token', pathname: 'tasks/a.png' }),
    });
    const res = makeRes();
    const done = uploadHandler(req, res);
    send();
    await done;

    expect(res.statusCode).toBe(200);
    expect(res.payload).toEqual({
      type: 'blob.generate-client-token',
      clientToken: 'signed-token',
    });
    expect(handleUpload).toHaveBeenCalledTimes(1);
    expect(handleUpload).toHaveBeenCalledWith(
      expect.objectContaining({
        token: 'test-token',
        body: { type: 'blob.generate-client-token', pathname: 'tasks/a.png' },
      })
    );
  });

  it('handleUpload 抛错时回 400 与错误信息', async () => {
    handleUpload.mockRejectedValueOnce(new Error('invalid token'));
    const { req, send } = makeReq({ body: '{}' });
    const res = makeRes();
    const done = uploadHandler(req, res);
    send();
    await done;

    expect(res.statusCode).toBe(400);
    expect(res.payload.error).toContain('invalid token');
  });

  it('请求体不是合法 JSON 时回 400', async () => {
    const { req, send } = makeReq({ body: 'not-json' });
    const res = makeRes();
    const done = uploadHandler(req, res);
    send();
    await done;

    expect(res.statusCode).toBe(400);
  });
});

describe('api/blob/delete', () => {
  it('非 POST 请求回 405', async () => {
    const { req } = makeReq({ method: 'GET' });
    const res = makeRes();
    await deleteHandler(req, res);

    expect(res.statusCode).toBe(405);
    expect(del).not.toHaveBeenCalled();
  });

  it('缺少 urls 数组时回 400', async () => {
    const { req, send } = makeReq({ body: '{}' });
    const res = makeRes();
    const done = deleteHandler(req, res);
    send();
    await done;

    expect(res.statusCode).toBe(400);
    expect(del).not.toHaveBeenCalled();
  });

  it('携带 urls 时调用 del 并回 200', async () => {
    del.mockResolvedValueOnce(undefined);
    const urls = ['https://xxx.public.blob.vercel-storage.com/a.png'];
    const { req, send } = makeReq({ body: JSON.stringify({ urls }) });
    const res = makeRes();
    const done = deleteHandler(req, res);
    send();
    await done;

    expect(res.statusCode).toBe(200);
    expect(res.payload).toEqual({ ok: true });
    expect(del).toHaveBeenCalledWith(urls, { token: 'test-token' });
  });

  it('del 失败时回 500 与错误信息', async () => {
    del.mockRejectedValueOnce(new Error('blob down'));
    const { req, send } = makeReq({ body: JSON.stringify({ urls: ['https://x/y.png'] }) });
    const res = makeRes();
    const done = deleteHandler(req, res);
    send();
    await done;

    expect(res.statusCode).toBe(500);
    expect(res.payload.error).toContain('blob down');
  });
});
