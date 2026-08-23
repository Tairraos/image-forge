import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";
import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
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

export default defineConfig({
  plugins: [vue(), serveImageForgeData()],
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.js"],
  },
  server: {
    host: "127.0.0.1",
    port: 1421,
    strictPort: true,
  },
  clearScreen: false,
});
