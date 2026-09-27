// Vercel Blob 图片存储层。
// 桌面版用本地文件系统，Web 版用 Vercel Blob；
// 上传/删除都经由同源 /api/blob/* 服务端完成：read-write token 只存在于 Vercel
// 服务端环境变量（BLOB_READ_WRITE_TOKEN），不再编译进前端 bundle；
// 浏览器通过 @vercel/blob/client 的短时令牌直传 Blob，大文件不受函数 4.5MB 请求体限制。
// 本地开发会把图片写入 ~/.image-forge 目录，
// 通过 vite dev server 的 /image-forge-data 路径提供给浏览器，避免任何图片字节进入 localStorage。
// API 参考：https://vercel.com/docs/vercel-blob

import { upload as blobClientUpload } from '@vercel/blob/client';

const BLOB_UPLOAD_URL = '/api/blob/upload';
const BLOB_DELETE_URL = '/api/blob/delete';
const DEV_DATA_ORIGIN = '/image-forge-data';
const DEV_PORTS = new Set(['1421', String(import.meta.env.VITE_DEV_PORT || '')].filter(Boolean));

/** 当前是否处于本地开发模式：浏览器在 vite dev server 同源下 */
export function isLocalDev() {
  if (typeof window === 'undefined') return false;
  // 生产构建里没有 dev server 代理可用，直接走远端逻辑（此时需已部署 /api/blob/* 服务端）
  if (!DEV_PORTS.has(window.location?.port)) return false;
  return window.location.protocol === 'http:' || window.location.protocol === 'https:';
}

/** 上传图片：
 *  - 本地开发：写入 ~/.image-forge/<relPath>，返回可被 vite 代理读取的 HTTP URL
 *  - 部署的 Web 版：经服务端签发短时令牌后直传 Vercel Blob，返回远端 URL
 */
export async function uploadImage(fileName, blob, relPath, signal) {
  if (isLocalDev()) {
    return uploadToLocalFs(fileName, blob, relPath, signal);
  }
  const safeName = sanitizeFileName(fileName || `image-${Date.now()}.png`);
  try {
    const result = await blobClientUpload(safeName, blob, {
      access: 'public',
      handleUploadUrl: BLOB_UPLOAD_URL,
      contentType: blob.type || undefined,
      abortSignal: signal,
    });
    return result.url;
  } catch (error) {
    throw new Error(
      `Blob 上传失败（请确认已部署 /api/blob/upload 并在 Vercel 配置 BLOB_READ_WRITE_TOKEN）: ${
        error?.message || error
      }`,
      { cause: error }
    );
  }
}

/** 把图片写入本地 ~/.image-forge 目录，返回可被 vite 代理的 URL */
async function uploadToLocalFs(fileName, blob, relPath, signal) {
  const safeName = sanitizeFileName(fileName || `image-${Date.now()}.png`);
  // 强制以 ~/.image-forge 为根，禁止越界
  const subPath = sanitizeRelPath(relPath) || 'tasks/uploads';
  const encoded = `${subPath}/${safeName}`.split('/').map(encodeURIComponent).join('/');
  const res = await fetch(`${DEV_DATA_ORIGIN}/${encoded}`, {
    signal,
    method: 'POST',
    headers: { 'Content-Type': blob.type || 'application/octet-stream' },
    body: blob,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`写入 ~/.image-forge 失败: ${res.status} ${text}`);
  }
  const data = await res.json().catch(() => ({}));
  // 返回 vite 代理的 HTTP URL，模板/任务记录里只放这个 URL，不放任何图片字节
  return data.url || `${DEV_DATA_ORIGIN}/${encoded}`;
}

/** 从 URL 下载图片为 Blob */
export async function downloadImage(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`图片下载失败: ${res.status}`);
  return res.blob();
}

/** 删除远端 / 本地图片：Vercel Blob 走服务端 API；本地开发直接尝试删除文件 */
export async function deleteImage(url) {
  if (!url) return;
  if (isLocalDev() && url.startsWith(DEV_DATA_ORIGIN)) {
    try {
      const path = decodeURIComponent(url.slice(DEV_DATA_ORIGIN.length));
      await fetch(encodeURI(DEV_DATA_ORIGIN + path), { method: 'DELETE' }).catch(() => {});
    } catch {
      // 静默忽略
    }
    return;
  }
  if (isBlobUrl(url)) {
    const res = await fetch(BLOB_DELETE_URL, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ urls: [url] }),
    });
    if (!res.ok) {
      console.warn('Blob 删除失败:', res.status, await res.text());
    }
  }
}

/** 解析 ~/.image-forge 下的 URL 为绝对磁盘路径（仅供 dev server 内部使用） */
export function devUrlToAbsPath(url) {
  if (!url || !url.startsWith(DEV_DATA_ORIGIN)) return null;
  return null;
}

/** 判断是否为 Vercel Blob URL */
export function isBlobUrl(url) {
  if (!url) return false;
  return url.startsWith('https://') && url.includes('blob.vercel-storage.com');
}

function sanitizeFileName(name) {
  // 仅保留文件名部分，剥掉任何路径分隔符
  const base =
    String(name || '')
      .split(/[\\/]/)
      .pop() || `image-${Date.now()}.png`;
  return base.replace(/[^\w.-]+/g, '_');
}

function sanitizeRelPath(input) {
  if (!input) return '';
  return String(input)
    .replace(/^\/+/, '')
    .split(/[\\/]+/)
    .filter((seg) => seg && seg !== '..' && seg !== '.')
    .join('/');
}
