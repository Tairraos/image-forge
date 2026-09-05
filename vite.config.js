import vue from '@vitejs/plugin-vue';
import { defineConfig, loadEnv } from 'vite';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { serveImageForgeData } from './dev-server/data-server.mjs';

const IMAGE_FORGE_DIR = join(homedir(), '.image-forge');

/** 启动时打印开发域名 */
function devHostBanner() {
  let host = '127.0.0.1';
  let port = 1421;
  return {
    name: 'dev-host-banner',
    config(_, { command, mode }) {
      if (command !== 'serve') return;
      const env = loadEnv(mode, process.cwd(), '');
      if (env.VITE_DEV_HOST) host = env.VITE_DEV_HOST;
      if (env.VITE_DEV_PORT) port = parseInt(env.VITE_DEV_PORT, 10) || 1421;
    },
    configureServer(server) {
      const protocol = server.config.server.https ? 'https' : 'http';
      const url = `${protocol}://${host}:${port}`;
      server.httpServer?.once('listening', () => {
        console.log('');
        console.log('  \x1b[36m🌐 开发域名：%s\x1b[0m', url);
        if (port === 443) {
          console.log('  \x1b[33m⚠  端口 443 需要 sudo 权限启动\x1b[0m');
        }
        console.log('');
      });
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const host = env.VITE_DEV_HOST || '127.0.0.1';
  const port = parseInt(env.VITE_DEV_PORT, 10) || 1421;
  const useHttps = env.VITE_DEV_HTTPS === 'true';
  const allowedHosts = [host, '127.0.0.1', 'localhost'];
  if (host !== '127.0.0.1' && host !== 'localhost') {
    allowedHosts.push(host);
  }

  let https = false;
  if (useHttps) {
    const certDir = join(IMAGE_FORGE_DIR, 'certs');
    const keyPath = join(certDir, `${host}-key.pem`);
    const certPath = join(certDir, `${host}.pem`);
    try {
      https = {
        key: readFileSync(keyPath),
        cert: readFileSync(certPath),
      };
    } catch {
      console.warn(`\x1b[33m⚠  HTTPS 证书未找到，回退到 HTTP。请先运行：`);
      console.warn(`   mkcert -key-file ${keyPath} -cert-file ${certPath} ${host}\x1b[0m\n`);
    }
  }

  return {
    plugins: [vue(), serveImageForgeData(), devHostBanner()],
    test: {
      environment: 'jsdom',
      setupFiles: ['./tests/setup.js'],
      coverage: {
        provider: 'v8',
        reporter: ['text', 'text-summary', 'lcov'],
        include: ['src/api/**/*.js', 'src/lib/**/*.js'],
        exclude: ['src/api/index.js', 'src/api/adapter-tauri.js'],
      },
    },
    server: {
      host: '0.0.0.0',
      port,
      strictPort: true,
      allowedHosts,
      https,
    },
    clearScreen: false,
  };
});
