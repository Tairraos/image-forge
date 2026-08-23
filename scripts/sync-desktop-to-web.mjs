// 同步桌面版 ~/.image-forge 数据到浏览器 Web 版。
// 读取 settings.json、prompt-templates.json、Agent 会话和 SQLite 图片库。
//
// 用法：
//   node scripts/sync-desktop-to-web.mjs          # 输出控制台代码
//   node scripts/sync-desktop-to-web.mjs --serve  # 启动 HTTP 服务

import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { homedir } from "node:os";
import { join } from "node:path";
import Database from "better-sqlite3";

const DATA_DIR = join(homedir(), ".image-forge");
const SETTINGS_FILE = join(DATA_DIR, "settings.json");
const TEMPLATES_FILE = join(DATA_DIR, "prompt-templates.json");
const AGENT_DIR = join(DATA_DIR, "agent", "sessions");
const SQLITE_FILE = join(DATA_DIR, "library.sqlite");

async function readJSON(path) {
  try {
    return JSON.parse(await readFile(path, "utf-8"));
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
  } catch { /* 无会话 */ }
  return sessions;
}

function readTaskHistory() {
  try {
    const db = new Database(SQLITE_FILE, { readonly: true });
    const rows = db.prepare("SELECT record_json FROM tasks ORDER BY created_at DESC").all();
    db.close();
    return rows.map((row) => JSON.parse(row.record_json));
  } catch {
    return [];
  }
}

async function collectData() {
  const [settings, templates, sessions] = await Promise.all([
    readJSON(SETTINGS_FILE),
    readJSON(TEMPLATES_FILE),
    readAgentSessions(),
  ]);
  const tasks = readTaskHistory();

  return {
    settings: settings || { providers: [] },
    templates: templates || [],
    agentSessions: sessions,
    tasks,
    taskCount: tasks.length,
  };
}

function generateConsoleScript(data) {
  return `
// === 从桌面版 ~/.image-forge 导入数据到浏览器 ===
// 复制以下全部代码，粘贴到浏览器控制台（F12 → Console），回车执行

(async function importData() {
  const data = ${JSON.stringify(data)};
  
  if (data.settings?.providers?.length) {
    localStorage.setItem("if_settings", JSON.stringify(data.settings));
    console.log("✅ 已导入设置（" + data.settings.providers.length + " 个 API 源）");
  }
  if (data.templates?.length) {
    localStorage.setItem("if_templates", JSON.stringify(data.templates));
    console.log("✅ 已导入模板（" + data.templates.length + " 个）");
  }
  if (data.agentSessions?.length) {
    localStorage.setItem("if_agent_sessions", JSON.stringify(data.agentSessions));
    console.log("✅ 已导入 Agent 会话（" + data.agentSessions.length + " 个）");
  }
  if (data.tasks?.length) {
    // 通过 IndexedDB 批量导入图片库
    const db = await new Promise((resolve, reject) => {
      const req = indexedDB.open("ImageForge", 1);
      req.onupgradeneeded = (e) => {
        const db = e.target.result;
        if (!db.objectStoreNames.contains("tasks")) {
          db.createObjectStore("tasks", { keyPath: "id" });
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    const tx = db.transaction("tasks", "readwrite");
    const store = tx.objectStore("tasks");
    for (const task of data.tasks) {
      const row = {
        id: task.id,
        created_at: task.created_at || task.createdAt || "",
        library_date: task.library_date || (task.completed_at || task.created_at || "").slice(0, 10),
        status: task.status || "completed",
        origin: task.origin || (task.agent_session_id || task.task_group_id ? "agent" : "drawing"),
        task_group_id: task.task_group_id || "",
        prompt: task.prompt || "",
        model: task.model || "",
        record_json: JSON.stringify(task),
      };
      store.put(row);
    }
    await new Promise((resolve, reject) => {
      tx.oncomplete = resolve;
      tx.onerror = () => reject(tx.error);
    });
    db.close();
    console.log("✅ 已导入图片库（" + data.tasks.length + " 条记录）");
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
    button { padding: 10px 24px; font-size: 15px; cursor: pointer; border: 0; border-radius: 8px; background: #7c5ce8; color: #fff; margin-right: 8px; }
    pre { background: #f5f5f5; padding: 12px; border-radius: 6px; font-size: 12px; overflow: auto; max-height: 300px; }
    .ok { color: #2a8; }
    .err { color: #c2415b; }
    .info { color: #666; font-size: 13px; margin: 8px 0; }
  </style>
</head>
<body>
  <h1>Image Forge — 桌面数据同步到浏览器</h1>
  <p class="info">将导入：${data.settings?.providers?.length || 0} 个 API 源、${data.templates?.length || 0} 个模板、${data.agentSessions?.length || 0} 个会话、${data.taskCount || 0} 条图片库记录</p>
  <button onclick="doImport()">导入数据</button>
  <pre id="result"></pre>
  <script>
    const data = ${json};
    async function doImport() {
      const el = document.getElementById("result");
      const lines = [];
      try {
        if (data.settings?.providers?.length) {
          localStorage.setItem("if_settings", JSON.stringify(data.settings));
          lines.push("✅ 已导入设置（" + data.settings.providers.length + " 个 API 源）");
        }
        if (data.templates?.length) {
          localStorage.setItem("if_templates", JSON.stringify(data.templates));
          lines.push("✅ 已导入模板（" + data.templates.length + " 个）");
        }
        if (data.agentSessions?.length) {
          localStorage.setItem("if_agent_sessions", JSON.stringify(data.agentSessions));
          lines.push("✅ 已导入 Agent 会话（" + data.agentSessions.length + " 个）");
        }
        if (data.tasks?.length) {
          const db = await new Promise((resolve, reject) => {
            const req = indexedDB.open("ImageForge", 1);
            req.onupgradeneeded = (e) => {
              const db = e.target.result;
              if (!db.objectStoreNames.contains("tasks")) {
                db.createObjectStore("tasks", { keyPath: "id" });
              }
            };
            req.onsuccess = () => resolve(req.result);
            req.onerror = () => reject(req.error);
          });
          const tx = db.transaction("tasks", "readwrite");
          const store = tx.objectStore("tasks");
          for (const task of data.tasks) {
            store.put({
              id: task.id,
              created_at: task.created_at || task.createdAt || "",
              library_date: task.library_date || (task.completed_at || task.created_at || "").slice(0, 10),
              status: task.status || "completed",
              origin: task.origin || (task.agent_session_id || task.task_group_id ? "agent" : "drawing"),
              task_group_id: task.task_group_id || "",
              prompt: task.prompt || "",
              model: task.model || "",
              record_json: JSON.stringify(task),
            });
          }
          await new Promise((resolve, reject) => {
            tx.oncomplete = resolve;
            tx.onerror = () => reject(tx.error);
          });
          db.close();
          lines.push("✅ 已导入图片库（" + data.tasks.length + " 条记录）");
        }
        lines.push("🎉 导入完成！刷新页面生效。");
        el.className = "ok";
      } catch (err) {
        lines.push("❌ 导入失败：" + err.message);
        el.className = "err";
      }
      el.textContent = lines.join("\\n");
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
      console.log(`  3. 将导入 ${data.taskCount} 条图片库记录`);
      console.log("  4. 关闭此页面，重新启动 Vite：pnpm dev\n");
      console.log("  按 Ctrl+C 停止。\n");
    });
    return;
  }

  const script = generateConsoleScript(data);
  console.log(script);
  console.log(`\n// 将导入 ${data.taskCount} 条图片库记录`);
  console.log("// 复制以上代码，粘贴到浏览器控制台（F12 → Console），回车执行\n");
}

main().catch((error) => {
  console.error("同步失败:", error.message);
  process.exit(1);
});