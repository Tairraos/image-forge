// providers.spec.js — Web 版生图 API 协议测试（mock fetch，不发真实请求）
// 覆盖 OpenAI / Gemini / Grok 三家协议的请求组装、响应解析与错误处理。

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { executeGeneration } from '../../src/api/providers.js';

const fetchMock = vi.fn();

const openAIProvider = {
  modelType: 'image-gpt',
  baseUrl: 'https://api.example.com/v1/',
  apiKey: 'sk-1',
  imageModel: 'gpt-image-2',
};
const geminiProvider = {
  modelType: 'image-gemini',
  baseUrl: '',
  apiKey: 'g-key',
  imageModel: 'models/gemini-image',
};
const grokProvider = {
  modelType: 'image-grok',
  baseUrl: 'https://api.x.ai/v1',
  apiKey: 'x-key',
  imageModel: 'grok-image',
};

const pngBytes = new Uint8Array([1, 2, 3, 4]);

function base64Of(bytes) {
  return btoa(String.fromCharCode(...bytes));
}

function jsonResponse(body, status = 200) {
  const text = JSON.stringify(body);
  return { ok: status >= 200 && status < 300, status, text: async () => text };
}

// 参考图 URL 的响应（urlToBlob / urlToDataUrl 用）
function imageSourceResponse() {
  return {
    ok: true,
    status: 200,
    blob: async () => new Blob([new Uint8Array([9, 9])], { type: 'image/jpeg' }),
  };
}

const jpegBase64 = base64Of(new Uint8Array([9, 9]));

beforeEach(() => {
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('OpenAI 协议（image-gpt）', () => {
  it('生图请求组装与 b64 响应解析', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        data: [
          {
            b64_json: base64Of(pngBytes),
            size: '1024x1024',
            output_format: 'png',
            revised_prompt: '改良',
          },
        ],
        usage: { total_tokens: 10 },
      })
    );

    const results = await executeGeneration(openAIProvider, {
      prompt: '猫',
      count: 1,
      output_format: 'png',
      size: '1024x1024',
      quality: 'high',
      background: 'transparent',
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    // 尾部斜杠被剥掉
    expect(url).toBe('https://api.example.com/v1/images/generations');
    expect(init.headers.Authorization).toBe('Bearer sk-1');
    const payload = JSON.parse(init.body);
    expect(payload).toMatchObject({
      model: 'gpt-image-2',
      prompt: '猫',
      n: 1,
      output_format: 'png',
      size: '1024x1024',
      quality: 'high',
      background: 'transparent',
    });

    expect(results).toHaveLength(1);
    expect(results[0].bytes).toEqual(pngBytes);
    expect(results[0].size).toBe('1024x1024');
    expect(results[0].revised_prompt).toBe('改良');
    expect(results[0].usage).toEqual({ total_tokens: 10 });
  });

  it('带参考图时改走 /images/edits 的 multipart 请求', async () => {
    fetchMock
      .mockResolvedValueOnce(imageSourceResponse())
      .mockResolvedValueOnce(jsonResponse({ data: [{ b64_json: base64Of(pngBytes) }] }));

    const results = await executeGeneration(openAIProvider, {
      prompt: '改图',
      reference_paths: ['http://localhost:1421/image-forge-data/ref.png'],
    });

    const [refCall, editCall] = fetchMock.mock.calls;
    expect(refCall[0]).toBe('http://localhost:1421/image-forge-data/ref.png');
    expect(editCall[0]).toBe('https://api.example.com/v1/images/edits');
    expect(editCall[1].headers.Authorization).toBe('Bearer sk-1');
    // multipart 的 Content-Type 由浏览器生成，代码不手工指定
    expect(editCall[1].headers['Content-Type']).toBeUndefined();

    const form = editCall[1].body;
    expect(form).toBeInstanceOf(FormData);
    expect(form.get('model')).toBe('gpt-image-2');
    expect(form.get('prompt')).toBe('改图');
    expect(form.get('n')).toBe('1');
    expect(form.get('image')).toBeInstanceOf(Blob);

    expect(results[0].bytes).toEqual(pngBytes);
  });

  it('API 错误提取 error.message', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ error: { message: '配额不足' } }, 400));
    await expect(executeGeneration(openAIProvider, { prompt: 'x' })).rejects.toThrow(
      'OpenAI API 错误 (400): 配额不足'
    );
  });

  it('非 JSON 错误体回退为原始文本', async () => {
    fetchMock.mockResolvedValueOnce({ ok: false, status: 502, text: async () => 'Bad Gateway' });
    await expect(executeGeneration(openAIProvider, { prompt: 'x' })).rejects.toThrow(
      'OpenAI API 错误 (502): Bad Gateway'
    );
  });

  it('返回 URL 而非 base64 时报错', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ data: [{ url: 'https://cdn.example.com/a.png' }] })
    );
    await expect(executeGeneration(openAIProvider, { prompt: 'x' })).rejects.toThrow(
      'OpenAI 返回了 URL 而非 base64'
    );
  });

  it('条目既无 b64_json 也无 url 时报错', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ data: [{}] }));
    await expect(executeGeneration(openAIProvider, { prompt: 'x' })).rejects.toThrow(
      'OpenAI 未返回图像数据'
    );
  });
});

describe('Gemini 协议（image-gemini）', () => {
  it('生图请求组装、模型前缀剥离与 inlineData 解析', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        candidates: [
          {
            content: {
              parts: [{ inlineData: { mimeType: 'image/png', data: base64Of(pngBytes) } }],
            },
          },
        ],
        usageMetadata: { totalTokenCount: 7 },
      })
    );

    const results = await executeGeneration(geminiProvider, {
      prompt: '山',
      ratio: '16:9',
      resolution: '2k',
    });

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe(
      'https://generativelanguage.googleapis.com/v1beta/models/gemini-image:generateContent'
    );
    expect(init.headers['x-goog-api-key']).toBe('g-key');
    const payload = JSON.parse(init.body);
    expect(payload.contents[0].parts[0]).toEqual({ text: '山' });
    expect(payload.generationConfig.imageConfig).toEqual({ aspectRatio: '16:9', imageSize: '2K' });

    expect(results[0].bytes).toEqual(pngBytes);
    expect(results[0].output_format).toBe('png');
    expect(results[0].usage).toEqual({ totalTokenCount: 7 });
  });

  it('未知分辨率回退到 1K', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        candidates: [{ content: { parts: [{ inlineData: { data: base64Of(pngBytes) } }] } }],
      })
    );
    await executeGeneration(geminiProvider, { prompt: 'x', resolution: '8k' });
    const payload = JSON.parse(fetchMock.mock.calls[0][1].body);
    expect(payload.generationConfig.imageConfig.imageSize).toBe('1K');
  });

  it('参考图读取后转成 inlineData parts', async () => {
    fetchMock.mockResolvedValueOnce(imageSourceResponse()).mockResolvedValueOnce(
      jsonResponse({
        candidates: [{ content: { parts: [{ inlineData: { data: base64Of(pngBytes) } }] } }],
      })
    );

    await executeGeneration(geminiProvider, {
      prompt: 'x',
      reference_paths: ['http://localhost:1421/image-forge-data/ref.jpg'],
    });

    const payload = JSON.parse(fetchMock.mock.calls[1][1].body);
    const parts = payload.contents[0].parts;
    expect(parts).toHaveLength(2);
    expect(parts[1].inlineData).toEqual({ mimeType: 'image/jpeg', data: jpegBase64 });
  });

  it('错误响应与无图像数据分别报错', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ error: { message: '密钥无效' } }, 429));
    await expect(executeGeneration(geminiProvider, { prompt: 'x' })).rejects.toThrow(
      'Gemini API 错误 (429): 密钥无效'
    );

    fetchMock.mockResolvedValueOnce(jsonResponse({ candidates: [] }));
    await expect(executeGeneration(geminiProvider, { prompt: 'x' })).rejects.toThrow(
      'Gemini 未返回图像数据'
    );
  });
});

describe('Grok 协议（image-grok）', () => {
  it('生图请求组装（aspect_ratio、小写 resolution、b64_json）', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ data: [{ b64_json: base64Of(pngBytes) }] }));

    const results = await executeGeneration(grokProvider, {
      prompt: '狗',
      ratio: '4:3',
      resolution: '3k',
      count: 2,
    });

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe('https://api.x.ai/v1/images/generations');
    const payload = JSON.parse(init.body);
    expect(payload).toMatchObject({
      model: 'grok-image',
      prompt: '狗',
      n: 2,
      aspect_ratio: '4:3',
      resolution: '3k',
      response_format: 'b64_json',
    });
    expect(results[0].bytes).toEqual(pngBytes);
  });

  it('单参考图走 /images/edits 且 payload.image 为对象', async () => {
    fetchMock
      .mockResolvedValueOnce(imageSourceResponse())
      .mockResolvedValueOnce(jsonResponse({ data: [{ b64_json: base64Of(pngBytes) }] }));

    await executeGeneration(grokProvider, {
      prompt: 'x',
      reference_paths: ['http://localhost:1421/image-forge-data/a.jpg'],
    });

    const [refCall, apiCall] = fetchMock.mock.calls;
    expect(refCall[0]).toBe('http://localhost:1421/image-forge-data/a.jpg');
    expect(apiCall[0]).toBe('https://api.x.ai/v1/images/edits');
    const payload = JSON.parse(apiCall[1].body);
    expect(payload.image).toEqual({
      url: `data:image/jpeg;base64,${jpegBase64}`,
      type: 'image_url',
    });
  });

  it('多参考图走 payload.images 数组', async () => {
    fetchMock
      .mockResolvedValueOnce(imageSourceResponse())
      .mockResolvedValueOnce(imageSourceResponse())
      .mockResolvedValueOnce(jsonResponse({ data: [{ b64_json: base64Of(pngBytes) }] }));

    await executeGeneration(grokProvider, {
      prompt: 'x',
      reference_paths: ['http://l/a.jpg', 'http://l/b.jpg'],
    });

    const payload = JSON.parse(fetchMock.mock.calls[2][1].body);
    expect(payload.images).toHaveLength(2);
    expect(payload.image).toBeUndefined();
  });

  it('错误响应复用 OpenAI 解析器', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ error: { message: '超速' } }, 429));
    await expect(executeGeneration(grokProvider, { prompt: 'x' })).rejects.toThrow(
      'OpenAI API 错误 (429): 超速'
    );
  });
});

describe('协议分发', () => {
  it('未知 modelType 默认走 OpenAI 协议', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ data: [{ b64_json: base64Of(pngBytes) }] }));
    await executeGeneration({ baseUrl: 'https://p.com/v1', apiKey: 'k' }, { prompt: 'x' });
    expect(fetchMock.mock.calls[0][0]).toBe('https://p.com/v1/images/generations');
  });
});
