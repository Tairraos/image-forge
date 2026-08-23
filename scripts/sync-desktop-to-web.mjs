// 同步桌面版 ~/.image-forge 数据到浏览器 Web 版。
// 读取桌面版的 settings.json 和 prompt-templates.json，
// 生成可在浏览器控制台执行的代码，一键导入到 localStorage。
//
// 用法：
//   node scripts/sync-desktop-to-web.mjs          # 输出控制台代码
//   node scripts/sync-desktop-to-web.mjs --serve  # 启动 HTTP 服务，浏览器自动导入

import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { homedir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

const DATA_DIR = join(homedir(), ".image-forge");
const SETTINGS_FILE = join(DATA_DIR, "settings.json");
const TEMPLATES_FILE = join(DATA_DIR, "prompt-templates.json");
const AGENT_DIR = join(DATA_DIR, "agent", "sessions");

async function readJSON(path) {
  try {
    const raw = await readFile(path, "utf-8");
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

async function readAgentSessions() {
  const sessions = [];
  try {
    const { readdir } = await import("node:fs/promises");
    const files = await readdir(AGENT_DIR);
    for (const file of files) {
      if (!file.endsWith(".json")) continue;
      const data = await readJSON(join(AGENT_DIR, file));
      if (data) sessions.push(data);
    }
  } catch {
    // 目录不存在或无会话
  }
  return sessions;
}

async function collectData() {
  const [settings, templates, sessions] = await Promise.all([
    readJSON(SETTINGS_FILE),
    readJSON(TEMPLATES_FILE),
    readAgentSessions(),
  ]);

  const data = {
    settings: settings || { providers: [] },
    templates: templates || [],
    agentSessions: sessions,
  };

  return data;
}

function generateConsoleScript(data) {
  return `
// === 从桌面版 ~/.image-forge 导入数据到浏览器 ===
// 复制以下全部代码，粘贴到浏览器控制台，回车执行

(function importData() {
  const data = ${JSON.stringify(data, null, 2)};
  
  if (data.settings) {
    localStorage.setItem("if_settings", JSON.stringify(data.settings));
    console.log("✅ 已导入设置（" + (data.settings.providers?.length || 0) + " 个 API 源）");
  }
  if (data.templates) {
    localStorage.setItem("if_templates", JSON.stringify(data.templates));
    console.log("✅ 已导入模板（" + data.templates.length + " 个）");
  }
  if (data.agentSessions?.length) {
    localStorage.setItem("if_agent_sessions", JSON.stringify(data.agentSessions));
    console.log("✅ 已导入 Agent 会话（" + data.agentSessions.length + " 个）");
  }
  console.log("🎉 导入完成！刷新页面生效。");
})();
`.trim();
}

function generateHTMLPage(data) {
  const json = JSON.stringify(data);
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <title>Image Forge 数据同步</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 600px; margin: 60px auto; padding: 20px; color: #333; }
    h1 { font-size: 20px; }
    button { padding: 10px 24px; font-size: 15px; cursor: pointer; border: 0; border-radius: 8px; background: #7c5ce8; color: #fff; }
    pre { background: #f5f5f5; padding: 12px; border-radius: 6px; font-size: 12px; overflow: auto; max-height: 300px; }
    .ok { color: #2a8; }
  </style>
</head>
<body>
  <h1>Image Forge — 桌面数据同步到浏览器</h1>
  <p>点击按钮将桌面版 <code>~/.image-forge</code> 的数据导入浏览器 localStorage：</p>
  <button onclick="doImport()">导入数据</button>
  <pre id="result"></pre>
  <script>
    const data = ${json};
    function doImport() {
      const el = document.getElementById("result");
      const lines = [];
      if (data.settings) {
        localStorage.setItem("if_settings", JSON.stringify(data.settings));
        lines.push("✅ 已导入设置（" + (data.settings.providers?.length || 0) + " 个 API 源）");
      }
      if (data.templates) {
        localStorage.setItem("if_templates", JSON.stringify(data.templates));
        lines.push("✅ 已导入模板（" + data.templates.length + " 个）");
      }
      if (data.agentSessions?.length) {
        localStorage.setItem("if_agent_sessions", JSON.stringify(data.agentSessions));
        lines.push("✅ 已导入 Agent 会话（" + data.agentSessions.length + " 个）");
      }
      lines.push("🎉 导入完成！打开 Web 版页面即可看到数据。");
      el.textContent = lines.join("\\n");
      el.className = "ok";
    }
  </script>
</body>
</html>`;
}

async function main() {
  const data = await collectData();

  if (process.argv.includes("--serve")) {
    const html = generateHTMLPage(data);
    const server = createServer((_req, res) => {
      res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      res.end(html);
    });
    const port = 1421;
    server.listen(port, () => {
      console.log(`\n  数据同步页面：http://localhost:${port}\n`);
      console.log("  1. 确保 Vite 开发服务器已停止（Ctrl+C）");
      console.log("  2. 在浏览器中打开此地址，点击「导入数据」");
      console.log("  3. 关闭此页面，重新启动 Vite：pnpm dev");
      console.log("\n  按 Ctrl+C 停止同步服务。\n");
    });
    return;
  }

  // 默认：输出控制台代码
  const script = generateConsoleScript(data);
  console.log(script);
  console.log("\n// 复制以上代码，粘贴到浏览器控制台（F12 → Console），回车执行\n");
}

main().catch((error) => {
  console.error("同步失败:", error.message);
  process.exit(1);
});