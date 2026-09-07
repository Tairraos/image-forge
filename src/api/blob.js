// Vercel Blob 图片存储层。
// 桌面版用本地文件系统，Web 版用 Vercel Blob；
// 没有配置 VITE_BLOB_READ_WRITE_TOKEN 时，本地开发会把图片写入 ~/.image-forge 目录，
// 通过 vite dev server 的 /image-forge-data 路径提供给浏览器，避免任何图片字节进入 localStorage。
// API 参考：https://vercel.com/docs/storage/vercel-blob

const BLOB_TOKEN = import.meta.env.VITE_BLOB_READ_WRITE_TOKEN || '';
// .env 里常驻的占位符（如 vercel_blob_read_write_token_here）视为未配置，
// 否则本地开发会被误判成 Blob 模式，上传与桌面数据合并全部失效
const hasBlobToken = Boolean(BLOB_TOKEN) && !/_here$/i.test(BLOB_TOKEN);
const BLOB_BASE = 'https://blob.vercel-storage.com';
const DEV_DATA_ORIGIN = '/image-forge-data';
const DEV_PORTS = new Set(['1421', String(import.meta.env.VITE_DEV_PORT || '')].filter(Boolean));

/** 当前是否处于本地开发模式：未配置（有效的）Vercel token、且浏览器在 vite dev server 同源下 */
export function isLocalDev() {
  if (hasBlobToken) return false;
  if (typeof window === 'undefined') return false;
  // 生产构建里没有 dev server 代理可用，直接走远端逻辑（此时通常 BLOB_TOKEN 必填）
  if (!DEV_PORTS.has(window.location?.port)) return false;
  return window.location.protocol === 'http:' || window.location.protocol === 'https:';
}

/** 上传图片：
 *  - 配了 VITE_BLOB_READ_WRITE_TOKEN：上传到 Vercel Blob，返回远端 URL
 *  - 本地开发：写入 ~/.image-forge/<relPath>，返回可被 vite 代理读取的 HTTP URL
 *  - 其他情况：把图片放进 localStorage 的元数据键（仅作极端回退，绝不存进 if_settings / if_templates）
 */
export async function uploadImage(fileName, blob, relPath, signal) {
  if (hasBlobToken) {
    const form = new FormData();
    form.append('file', blob, fileName);
    const res = await fetch(`${BLOB_BASE}/api/upload?filename=${encodeURIComponent(fileName)}`, {
      signal,
      method: 'POST',
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
  if (isLocalDev()) {
    return uploadToLocalFs(fileName, blob, relPath, signal);
  }
  throw new Error('Web 版未配置 VITE_BLOB_READ_WRITE_TOKEN，且当前不在本地开发环境，无法上传图片');
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

/** 删除远端 / 本地图片：Vercel Blob 走 API；本地开发直接尝试删除文件 */
export async function deleteImage(url) {
  if (!url) return;
  if (hasBlobToken && url.startsWith('https://')) {
    const res = await fetch(`${BLOB_BASE}/api/delete`, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${BLOB_TOKEN}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ urls: [url] }),
    });
    if (!res.ok) {
      console.warn('Blob 删除失败:', res.status, await res.text());
    }
    return;
  }
  if (isLocalDev() && url.startsWith(DEV_DATA_ORIGIN)) {
    try {
      const path = decodeURIComponent(url.slice(DEV_DATA_ORIGIN.length));
      await fetch(encodeURI(DEV_DATA_ORIGIN + path), { method: 'DELETE' }).catch(() => {});
    } catch {
      // 静默忽略
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
