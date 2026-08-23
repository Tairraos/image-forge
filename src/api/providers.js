// Web 版生图 API 调用层，对应桌面版 images.rs。
// 支持 OpenAI / Gemini / Grok 三家协议，统一返回 { bytes, size, format } 结构。

const IMAGE_SIZE_MAP = {
  standard: "1K",
  "2k": "2K",
  "3k": "3K",
  "4k": "4K",
};

/**
 * 统一入口：根据 provider.modelType 分发到对应 API。
 * @param {Object} provider - API 配置 { baseUrl, apiKey, imageModel, modelType }
 * @param {Object} request - 生图请求参数
 * @returns {Promise<Array<{bytes: Uint8Array, size: string, output_format: string, revised_prompt: string}>>}
 */
export async function executeGeneration(provider, request) {
  const type = provider.modelType || "";
  if (type === "image-gemini") return callGemini(provider, request);
  if (type === "image-grok") return callGrok(provider, request);
  return callOpenAI(provider, request);
}

// ── OpenAI Images API ──

async function callOpenAI(provider, request) {
  const baseUrl = (provider.baseUrl || "").replace(/\/+$/, "");
  const apiKey = (provider.apiKey || "").trim();
  const model = provider.imageModel || "gpt-image-2";

  const refs = request.reference_paths || [];
  if (refs.length > 0) {
    return callOpenAIEdit(baseUrl, apiKey, model, request, refs);
  }
  return callOpenAIGenerate(baseUrl, apiKey, model, request);
}

async function callOpenAIGenerate(baseUrl, apiKey, model, request) {
  const payload = {
    model,
    prompt: request.prompt || "",
    n: request.count || 1,
    output_format: request.output_format || "png",
  };
  if (request.size) payload.size = request.size;
  if (request.quality) payload.quality = request.quality;
  if (request.background) payload.background = request.background;

  const res = await fetch(`${baseUrl}/images/generations`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(payload),
  });
  return parseOpenAIResponse(res, request);
}

async function callOpenAIEdit(baseUrl, apiKey, model, request, refs) {
  // 参考图编辑使用 multipart/form-data
  const form = new FormData();
  form.append("model", model);
  form.append("prompt", request.prompt || "");
  form.append("n", String(request.count || 1));
  form.append("output_format", request.output_format || "png");
  if (request.size) form.append("size", request.size);
  if (request.quality) form.append("quality", request.quality);

  for (const refPath of refs) {
    const blob = await urlToBlob(refPath);
    form.append("image", blob, "reference.png");
  }

  const res = await fetch(`${baseUrl}/images/edits`, {
    method: "POST",
    headers: { Authorization: `Bearer ${apiKey}`, Accept: "application/json" },
    body: form,
  });
  return parseOpenAIResponse(res, request);
}

async function parseOpenAIResponse(res, request) {
  const body = await res.text();
  if (!res.ok) {
    let msg = body;
    try { msg = JSON.parse(body).error?.message || body; } catch {}
    throw new Error(`OpenAI API 错误 (${res.status}): ${msg}`);
  }
  const json = JSON.parse(body);
  const data = json.data || [];
  const usage = json.usage || null;
  return data.map((item) => {
    let bytes;
    if (item.b64_json) {
      bytes = Uint8Array.from(atob(item.b64_json), (c) => c.charCodeAt(0));
    } else if (item.url) {
      throw new Error("OpenAI 返回了 URL 而非 base64，Web 版暂不支持 URL 下载");
    } else {
      throw new Error("OpenAI 未返回图像数据");
    }
    return {
      bytes,
      size: item.size || request.size || "",
      output_format: item.output_format || request.output_format || "png",
      revised_prompt: item.revised_prompt || "",
      background: item.background || "",
      quality: item.quality || request.quality || "",
      usage,
    };
  });
}

// ── Gemini Images API ──

async function callGemini(provider, request) {
  const baseUrl = (provider.baseUrl || "https://generativelanguage.googleapis.com/v1beta").replace(/\/+$/, "");
  const apiKey = (provider.apiKey || "").trim();
  const model = (provider.imageModel || "gemini-3.1-flash-image").replace(/^models\//, "");

  const parts = [{ text: request.prompt || "" }];
  const refs = request.reference_paths || [];
  for (const refPath of refs.slice(0, 14)) {
    const dataUrl = await urlToDataUrl(refPath);
    const [mimeType, data] = splitDataUrl(dataUrl);
    parts.push({ inlineData: { mimeType, data } });
  }

  const payload = {
    contents: [{ role: "user", parts }],
    generationConfig: {
      responseModalities: ["IMAGE"],
      imageConfig: {
        aspectRatio: request.ratio || "1:1",
        imageSize: IMAGE_SIZE_MAP[request.resolution] || "1K",
      },
    },
  };

  const res = await fetch(`${baseUrl}/models/${model}:generateContent`, {
    method: "POST",
    headers: {
      "x-goog-api-key": apiKey,
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(payload),
  });
  return parseGeminiResponse(res, request);
}

async function parseGeminiResponse(res, request) {
  const body = await res.text();
  if (!res.ok) {
    let msg = body;
    try { msg = JSON.parse(body).error?.message || body; } catch {}
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
          size: request.size || "",
          output_format: "png",
          revised_prompt: "",
          background: "",
          quality: "",
          usage: json.usageMetadata || null,
        });
      }
    }
  }
  if (!results.length) throw new Error("Gemini 未返回图像数据");
  return results;
}

// ── Grok Images API ──

async function callGrok(provider, request) {
  const baseUrl = (provider.baseUrl || "https://api.x.ai/v1").replace(/\/+$/, "");
  const apiKey = (provider.apiKey || "").trim();
  const model = provider.imageModel || "grok-imagine-image-quality";

  const refs = request.reference_paths || [];
  const payload = {
    model,
    prompt: request.prompt || "",
    n: request.count || 1,
    aspect_ratio: request.ratio || "1:1",
    resolution: (IMAGE_SIZE_MAP[request.resolution] || "1k").toLowerCase(),
    response_format: "b64_json",
  };

  let url = `${baseUrl}/images/generations`;
  if (refs.length > 0) {
    url = `${baseUrl}/images/edits`;
    const dataUrls = await Promise.all(refs.slice(0, 5).map(urlToDataUrl));
    if (dataUrls.length === 1) {
      payload.image = { url: dataUrls[0], type: "image_url" };
    } else {
      payload.images = dataUrls.map((url) => ({ url, type: "image_url" }));
    }
  }

  const res = await fetch(url, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(payload),
  });
  return parseOpenAIResponse(res, request);
}

// ── 工具函数 ──

async function urlToBlob(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`读取图片失败: ${res.status}`);
  return res.blob();
}

async function urlToDataUrl(url) {
  const blob = await urlToBlob(url);
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

function splitDataUrl(dataUrl) {
  const match = /^data:([^;]+);base64,(.+)$/.exec(dataUrl);
  if (!match) throw new Error("无效的 data URL");
  return [match[1], match[2]];
}

export { IMAGE_SIZE_MAP };