// Web 版生图 API 调用层，对应桌面版 images.rs。
// 支持 OpenAI / Gemini / Grok 三家协议，统一返回 { bytes, size, format } 结构。

import { sizeForPreset } from '../lib/options.js';
import { convertFileSrc } from '../tauri';

const IMAGE_SIZE_MAP = {
  standard: '1K',
  '2k': '2K',
  '3k': '3K',
  '4k': '4K',
};

/**
 * 统一入口：根据 provider.modelType 分发到对应 API。
 * @param {Object} provider - API 配置 { baseUrl, apiKey, imageModel, modelType }
 * @param {Object} request - 生图请求参数
 * @param {AbortSignal} [signal] - 取消当前任务的请求、参考图读取及结果下载
 * @returns {Promise<Array<{bytes: Uint8Array, size: string, output_format: string, revised_prompt: string}>>}
 */
export async function executeGeneration(provider, request, signal) {
  signal?.throwIfAborted();
  const resolution = String(request.resolution || 'standard').toLowerCase();
  request = {
    ...request,
    resolution,
    size: request.size || sizeForPreset(resolution, request.ratio),
  };
  const type = provider.modelType || '';
  if (type === 'image-gemini') return callGemini(provider, request, signal);
  if (type === 'image-grok') return callGrok(provider, request, signal);
  return callOpenAI(provider, request, signal);
}

// ── OpenAI Images API ──

async function callOpenAI(provider, request, signal) {
  const baseUrl = (provider.baseUrl || '').replace(/\/+$/, '');
  const apiKey = (provider.apiKey || '').trim();
  const model = provider.imageModel || 'gpt-image-2';

  const refs = request.reference_paths || [];
  if (refs.length > 0) {
    return callOpenAIEdit(baseUrl, apiKey, model, request, refs, signal);
  }
  return callOpenAIGenerate(baseUrl, apiKey, model, request, signal);
}

async function callOpenAIGenerate(baseUrl, apiKey, model, request, signal) {
  const payload = {
    model,
    prompt: request.prompt || '',
    n: request.count || 1,
    output_format: request.output_format || 'png',
  };
  if (request.size) payload.size = request.size;
  if (request.quality) payload.quality = request.quality;
  if (request.background) payload.background = request.background;

  const res = await fetch(`${baseUrl}/images/generations`, {
    signal,
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(payload),
  });
  return parseOpenAIResponse(res, request, signal);
}

async function callOpenAIEdit(baseUrl, apiKey, model, request, refs, signal) {
  // 参考图编辑使用 multipart/form-data
  const form = new FormData();
  form.append('model', model);
  form.append('prompt', request.prompt || '');
  form.append('n', String(request.count || 1));
  form.append('output_format', request.output_format || 'png');
  if (request.size) form.append('size', request.size);
  if (request.quality) form.append('quality', request.quality);
  if (request.background) form.append('background', request.background);

  for (const refPath of refs) {
    const blob = await urlToBlob(refPath, signal);
    form.append('image', blob, 'reference.png');
  }

  const res = await fetch(`${baseUrl}/images/edits`, {
    signal,
    method: 'POST',
    headers: { Authorization: `Bearer ${apiKey}`, Accept: 'application/json' },
    body: form,
  });
  return parseOpenAIResponse(res, request, signal);
}

async function parseOpenAIResponse(res, request, signal) {
  const body = await res.text();
  if (!res.ok) {
    let msg = body;
    try {
      msg = JSON.parse(body).error?.message || body;
    } catch {
      /* ignore parse error */
    }
    throw new Error(`OpenAI API 错误 (${res.status}): ${msg}`);
  }
  const json = JSON.parse(body);
  const data = json.data || [];
  if (!Array.isArray(data) || !data.length) throw new Error('生图服务未返回图像数据');
  const usage = json.usage || null;
  const results = [];
  for (const item of data) {
    let bytes;
    if (item.b64_json) {
      bytes = Uint8Array.from(atob(item.b64_json), (c) => c.charCodeAt(0));
    } else if (item.url) {
      // 部分中转网关只回图片 URL：主动下载成字节，行为对齐桌面版
      const download = await fetch(item.url, { signal });
      if (!download.ok) {
        throw new Error(`下载生成图片失败: HTTP ${download.status} ${item.url}`);
      }
      bytes = new Uint8Array(await download.arrayBuffer());
    } else {
      throw new Error('OpenAI 未返回图像数据');
    }
    if (!bytes.length) throw new Error('生图服务返回了空图片');
    results.push({
      bytes,
      size: item.size || request.size || '',
      output_format: item.output_format || request.output_format || 'png',
      revised_prompt: item.revised_prompt || '',
      background: item.background || '',
      quality: item.quality || request.quality || '',
      usage,
    });
  }
  return results;
}

// ── Gemini Images API ──

async function callGemini(provider, request, signal) {
  const baseUrl = (provider.baseUrl || 'https://generativelanguage.googleapis.com/v1beta').replace(
    /\/+$/,
    ''
  );
  const apiKey = (provider.apiKey || '').trim();
  const model = (provider.imageModel || 'gemini-3.1-flash-image').replace(/^models\//, '');

  const parts = [{ text: request.prompt || '' }];
  const refs = request.reference_paths || [];
  for (const refPath of refs.slice(0, 14)) {
    const dataUrl = await urlToDataUrl(refPath, signal);
    const [mimeType, data] = splitDataUrl(dataUrl);
    parts.push({ inlineData: { mimeType, data } });
  }

  const payload = {
    contents: [{ role: 'user', parts }],
    generationConfig: {
      responseModalities: ['IMAGE'],
      imageConfig: {
        aspectRatio: request.ratio || '1:1',
        imageSize: IMAGE_SIZE_MAP[request.resolution] || '1K',
      },
    },
  };

  const res = await fetch(`${baseUrl}/models/${model}:generateContent`, {
    signal,
    method: 'POST',
    headers: {
      'x-goog-api-key': apiKey,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(payload),
  });
  return parseGeminiResponse(res, request);
}

async function parseGeminiResponse(res, request) {
  const body = await res.text();
  if (!res.ok) {
    let msg = body;
    try {
      msg = JSON.parse(body).error?.message || body;
    } catch {
      /* ignore parse error */
    }
    throw new Error(`Gemini API 错误 (${res.status}): ${msg}`);
  }
  const json = JSON.parse(body);
  const candidates = json.candidates || [];
  const results = [];
  for (const candidate of candidates) {
    for (const part of candidate.content?.parts || []) {
      if (part.inlineData?.data) {
        const bytes = Uint8Array.from(atob(part.inlineData.data), (c) => c.charCodeAt(0));
        results.push({
          bytes,
          size: request.size || '',
          output_format: 'png',
          revised_prompt: '',
          background: '',
          quality: '',
          usage: json.usageMetadata || null,
        });
      }
    }
  }
  if (!results.length) throw new Error('Gemini 未返回图像数据');
  return results;
}

// ── Grok Images API ──

async function callGrok(provider, request, signal) {
  const baseUrl = (provider.baseUrl || 'https://api.x.ai/v1').replace(/\/+$/, '');
  const apiKey = (provider.apiKey || '').trim();
  const model = provider.imageModel || 'grok-imagine-image-quality';

  const refs = request.reference_paths || [];
  const payload = {
    model,
    prompt: request.prompt || '',
    n: request.count || 1,
    aspect_ratio: request.ratio || '1:1',
    resolution: (IMAGE_SIZE_MAP[request.resolution] || '1k').toLowerCase(),
    response_format: 'b64_json',
  };

  let url = `${baseUrl}/images/generations`;
  if (refs.length > 0) {
    url = `${baseUrl}/images/edits`;
    const dataUrls = await Promise.all(refs.slice(0, 5).map((url) => urlToDataUrl(url, signal)));
    if (dataUrls.length === 1) {
      payload.image = { url: dataUrls[0], type: 'image_url' };
    } else {
      payload.images = dataUrls.map((url) => ({ url, type: 'image_url' }));
    }
  }

  const res = await fetch(url, {
    signal,
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(payload),
  });
  return parseOpenAIResponse(res, request, signal);
}

// ── 工具函数 ──

async function urlToBlob(url, signal) {
  const res = await fetch(convertFileSrc(url), { signal });
  if (!res.ok) throw new Error(`读取图片失败: ${res.status}`);
  return res.blob();
}

async function urlToDataUrl(url, signal) {
  const blob = await urlToBlob(url, signal);
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

function splitDataUrl(dataUrl) {
  const match = /^data:([^;]+);base64,(.+)$/.exec(dataUrl);
  if (!match) throw new Error('无效的 data URL');
  return [match[1], match[2]];
}

export { IMAGE_SIZE_MAP };
