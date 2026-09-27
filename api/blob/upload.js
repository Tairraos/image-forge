// Vercel Function：为浏览器签发短时上传令牌，并接收上传完成回调。
// read-write token 只存在于服务端环境变量（Vercel 项目设置里的 BLOB_READ_WRITE_TOKEN），
// 不再编译进前端 bundle；浏览器用 @vercel/blob/client 凭短时令牌直传 Blob，
// 大文件不受 Serverless Function 约 4.5MB 的请求体限制。
import { handleUpload } from '@vercel/blob/client';

export default async function handler(req, res) {
  try {
    const body = await readJsonBody(req);
    const result = await handleUpload({
      request: req,
      body,
      token: process.env.BLOB_READ_WRITE_TOKEN,
      onBeforeGenerateToken: async () => {
        // 尽力同源校验：浏览器 POST 会带 Origin 头，跨站调用在此被拒绝；
        // 不带 Origin 的非浏览器请求无法用此机制区分，属已知边界。
        const origin = String(req.headers.origin || '');
        const host = String(req.headers.host || '');
        if (origin && new URL(origin).host !== host) {
          throw new Error('拒绝跨站上传请求');
        }
        return { addRandomSuffix: true };
      },
      onUploadCompleted: async ({ blob }) => {
        console.log(`[blob] 上传完成: ${blob.url} (${blob.size} bytes)`);
      },
    });
    res.status(200).json(result);
  } catch (error) {
    res.status(400).json({ error: String(error?.message || error) });
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
