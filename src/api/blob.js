// Vercel Blob 图片存储层。
// 桌面版用本地文件系统，Web 版用 Vercel Blob。
// API 参考：https://vercel.com/docs/storage/vercel-blob

const BLOB_TOKEN = import.meta.env.VITE_BLOB_READ_WRITE_TOKEN || "";
const BLOB_BASE = "https://blob.vercel-storage.com";

/** 上传图片到 Vercel Blob，返回 URL */
export async function uploadImage(fileName, blob) {
  if (!BLOB_TOKEN) {
    // 无 token 时回退到 base64 data URL（仅开发用途）
    return fallbackDataUrl(blob);
  }
  const form = new FormData();
  form.append("file", blob, fileName);
  const res = await fetch(`${BLOB_BASE}/api/upload?filename=${encodeURIComponent(fileName)}`, {
    method: "POST",
    headers: { Authorization: `Bearer ${BLOB_TOKEN}` },
    body: form,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Blob 上传失败: ${res.status} ${text}`);
  }
  const data = await res.json();
  return data.url;
}

/** 从 URL 下载图片为 Blob */
export async function downloadImage(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`图片下载失败: ${res.status}`);
  return res.blob();
}

/** 删除 Vercel Blob 上的图片 */
export async function deleteImage(url) {
  if (!BLOB_TOKEN || !url) return;
  const res = await fetch(`${BLOB_BASE}/api/delete`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${BLOB_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ urls: [url] }),
  });
  if (!res.ok) {
    console.warn("Blob 删除失败:", res.status, await res.text());
  }
}

/** 无 token 时的回退：转为 data URL */
function fallbackDataUrl(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

/** 判断是否为 Vercel Blob URL */
export function isBlobUrl(url) {
  return url?.startsWith("https://") && url.includes("blob.vercel-storage.com");
}