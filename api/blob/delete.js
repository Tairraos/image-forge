// Vercel Function：服务端删除 Blob 对象。
// read-write token 只存在于服务端环境变量（Vercel 项目设置里的 BLOB_READ_WRITE_TOKEN）。
import { del } from '@vercel/blob';

export default async function handler(req, res) {
  if (req.method !== 'POST') {
    res.status(405).json({ error: 'Method Not Allowed' });
    return;
  }
  try {
    const body = (await readJsonBody(req)) || {};
    const urls = Array.isArray(body.urls)
      ? body.urls.filter((url) => typeof url === 'string' && url)
      : [];
    if (!urls.length) {
      res.status(400).json({ error: '缺少 urls 数组' });
      return;
    }
    await del(urls, { token: process.env.BLOB_READ_WRITE_TOKEN });
    res.status(200).json({ ok: true });
  } catch (error) {
    res.status(500).json({ error: String(error?.message || error) });
  }
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => {
      const text = Buffer.concat(chunks).toString('utf8');
      if (!text) {
        resolve(undefined);
        return;
      }
      try {
        resolve(JSON.parse(text));
      } catch {
        reject(new Error('请求体不是合法 JSON'));
      }
    });
    req.on('error', reject);
  });
}
