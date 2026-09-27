// 本地开发数据服务的访问令牌。
// 在 .env 配置 VITE_DEV_DATA_TOKEN 后，dev server 的敏感端点（/__* 读写与文件写入/删除）
// 要求请求携带 x-if-data-token 头；这里在应用启动时为同源 /image-forge-data 请求自动附加，
// 静态图片 GET 不要求令牌，<img> 渲染不受影响。未配置令牌时本模块不做任何事。

const TOKEN = import.meta.env.VITE_DEV_DATA_TOKEN || '';
const DEV_DATA_MARKER = '/image-forge-data';

if (TOKEN && typeof window !== 'undefined') {
  const originalFetch = window.fetch.bind(window);
  window.fetch = (input, init = {}) => {
    const url = typeof input === 'string' ? input : (input && input.url) || '';
    if (url.includes(DEV_DATA_MARKER)) {
      const headers = new Headers(
        init.headers || (input instanceof Request ? input.headers : undefined)
      );
      if (!headers.has('x-if-data-token')) {
        headers.set('x-if-data-token', TOKEN);
      }
      return originalFetch(input, { ...init, headers });
    }
    return originalFetch(input, init);
  };
}
