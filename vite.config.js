import vue from "@vitejs/plugin-vue";
import { defineConfig, loadEnv } from "vite";
import { createReadStream, readFileSync, unlinkSync } from "node:fs";
import { mkdir, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join, normalize, extname } from "node:path";

const IMAGE_FORGE_DIR = join(homedir(), ".image-forge");

const MIME_MAP = {
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".gif": "image/gif",
  ".svg": "image/svg+xml",
  ".json": "application/json",
};

function getMimeType(filePath) {
  return MIME_MAP[extname(filePath).toLowerCase()] || "application/octet-stream";
}

/** 开发时把 ~/.image-forge 目录下的文件通过 HTTP 提供给浏览器 */
function serveImageForgeData() {
  return {
    name: "serve-image-forge-data",
    configureServer(server) {
      server.middlewares.use("/image-forge-data", async (req, res) => {
        const urlPath = decodeURIComponent(req.url?.split("?")[0] || "/");
        const filePath = normalize(join(IMAGE_FORGE_DIR, urlPath));
        // 安全检查：确保路径在 .image-forge 目录内
        if (!filePath.startsWith(IMAGE_FORGE_DIR)) {
          res.statusCode = 403;
          res.end("Forbidden");
          return;
        }
        if (req.method === "POST" || req.method === "PUT") {
          // 写文件：浏览器把 Blob / ArrayBuffer 直接放进 body，Content-Type 决定保存格式
          const chunks = [];
          req.on("data", (chunk) => chunks.push(chunk));
          req.on("end", async () => {
            try {
              const body = Buffer.concat(chunks);
              if (!body.length) {
                res.statusCode = 400;
                res.end("Empty body");
                return;
              }
              await mkdir(join(filePath, ".."), { recursive: true });
              await writeFile(filePath, body);
              res.setHeader("Content-Type", "application/json");
              res.end(
                JSON.stringify({
                  path: join(IMAGE_FORGE_DIR, urlPath),
                  url: `/image-forge-data${urlPath}`,
                  size: body.length,
                }),
              );
            } catch (err) {
              console.error("[image-forge-data] 写入失败:", err);
              res.statusCode = 500;
              res.end(String(err?.message || err));
            }
          });
          return;
        }
        if (req.method === "DELETE") {
          try {
            unlinkSync(filePath);
            res.statusCode = 204;
            res.end();
          } catch (err) {
            if (err?.code === "ENOENT") {
              res.statusCode = 404;
              res.end("Not Found");
            } else {
              res.statusCode = 500;
              res.end(String(err?.message || err));
            }
          }
          return;
        }
        try {
          const info = await stat(filePath);
          if (!info.isFile()) {
            res.statusCode = 404;
            res.end("Not Found");
            return;
          }
          const contentType = getMimeType(filePath);
          res.setHeader("Content-Type", contentType);
          res.setHeader("Content-Length", info.size);
          res.setHeader("Cache-Control", "public, max-age=3600");
          createReadStream(filePath).pipe(res);
        } catch {
          res.statusCode = 404;
          res.end("Not Found");
        }
      });
    },
  };
}

/** 启动时打印开发域名 */
function devHostBanner() {
  let host = "127.0.0.1";
  let port = 1421;
  return {
    name: "dev-host-banner",
    config(_, { command, mode }) {
      if (command !== "serve") return;
      const env = loadEnv(mode, process.cwd(), "");
      if (env.VITE_DEV_HOST) host = env.VITE_DEV_HOST;
      if (env.VITE_DEV_PORT) port = parseInt(env.VITE_DEV_PORT, 10) || 1421;
    },
    configureServer(server) {
      const protocol = server.config.server.https ? "https" : "http";
      const url = `${protocol}://${host}:${port}`;
      server.httpServer?.once("listening", () => {
        console.log("");
        console.log("  \x1b[36m🌐 开发域名：%s\x1b[0m", url);
        if (port === 443) {
          console.log("  \x1b[33m⚠  端口 443 需要 sudo 权限启动\x1b[0m");
        }
        console.log("");
      });
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const host = env.VITE_DEV_HOST || "127.0.0.1";
  const port = parseInt(env.VITE_DEV_PORT, 10) || 1421;
  const useHttps = env.VITE_DEV_HTTPS === "true";
  const allowedHosts = [host, "127.0.0.1", "localhost"];
  if (host !== "127.0.0.1" && host !== "localhost") {
    allowedHosts.push(host);
  }

  let https = false;
  if (useHttps) {
    const certDir = join(IMAGE_FORGE_DIR, "certs");
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
      environment: "jsdom",
      setupFiles: ["./tests/setup.js"],
    },
    server: {
      host: "0.0.0.0",
      port,
      strictPort: true,
      allowedHosts,
      https,
    },
    clearScreen: false,
  };
});
