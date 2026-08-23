// 双向同步：SQLite ↔ 浏览器 IndexedDB/localStorage
// 合并策略：以 updated_at 或 created_at 较新的记录为准
//
// 用法：
//   node scripts/sync-desktop-to-web.mjs --serve  # 启动双向同步服务

import { readFile } from "node:fs/promises";
import { createServer } from "node:https";
import { homedir } from "node:os";
import { join, extname } from "node:path";
import { createReadStream, readFileSync } from "node:fs";
import { stat } from "node:fs/promises";
import Database from "better-sqlite3";

const DATA_DIR = join(homedir(), ".image-forge");
const SQLITE_FILE = join(DATA_DIR, "library.sqlite");
const PORT = 443;
const HOST = "image.xiaole.qzz.io";
const CERT_DIR = join(DATA_DIR, "certs");

const MIME_MAP = {
  ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
  ".webp": "image/webp", ".gif": "image/gif", ".svg": "image/svg+xml",
  ".json": "application/json",
};

// ── SQLite 读取 ──

function readSQLite() {
  const db = new Database(SQLITE_FILE, { readonly: true });
  const data = {
    settings: null,
    templates: [],
    agentSessions: [],
    tasks: [],
  };

  try {
    const settingsRow = db.prepare("SELECT value FROM app_settings WHERE key = 'settings'").get();
    if (settingsRow) data.settings = JSON.parse(settingsRow.value);
  } catch {}

  try {
    const tpls = db.prepare("SELECT record_json FROM app_templates ORDER BY position ASC").all();
    data.templates = tpls.map((r) => JSON.parse(r.record_json));
  } catch {}

  try {
    const sessions = db.prepare("SELECT record_json FROM agent_sessions ORDER BY updated_at DESC").all();
    data.agentSessions = sessions.map((r) => JSON.parse(r.record_json));
  } catch {}

  try {
    const tasks = db.prepare("SELECT record_json FROM tasks ORDER BY created_at DESC").all();
    data.tasks = tasks.map((r) => JSON.parse(r.record_json));
  } catch {}

  db.close();
  return data;
}

// ── SQLite 写入 ──

function writeSQLite(data) {
  const db = new Database(SQLITE_FILE);
  const tx = db.transaction(() => {
    const now = new Date().toISOString();

    // 设置
    if (data.settings) {
      db.prepare("INSERT OR REPLACE INTO app_settings (key, value) VALUES ('settings', ?)").run(
        JSON.stringify(data.settings)
      );
    }

    // 模板
    if (data.templates?.length) {
      db.prepare("DELETE FROM app_templates").run();
      const insert = db.prepare(
        "INSERT INTO app_templates (id, position, created_at, updated_at, record_json) VALUES (?, ?, ?, ?, ?)"
      );
      for (let i = 0; i < data.templates.length; i++) {
        const t = data.templates[i];
        insert.run(t.id || `tpl-${i}`, i, t.createdAt || now, t.updatedAt || now, JSON.stringify(t));
      }
    }

    // 会话
    if (data.agentSessions?.length) {
      const upsert = db.prepare(
        "INSERT OR REPLACE INTO agent_sessions (id, created_at, updated_at, title, model_provider_id, status, record_json) VALUES (?, ?, ?, ?, ?, ?, ?)"
      );
      for (const s of data.agentSessions) {
        upsert.run(
          s.id, s.createdAt || now, s.updatedAt || now,
          s.title || "", s.modelProviderId || s.model_provider_id || "",
          s.status || "idle", JSON.stringify(s)
        );
      }
    }

    // 任务
    if (data.tasks?.length) {
      const upsert = db.prepare(`
        INSERT INTO tasks (id, created_at, updated_at, completed_at, library_date, prompt, model, provider_name, origin, task_group_id, status, record_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          updated_at=excluded.updated_at, completed_at=excluded.completed_at,
          library_date=excluded.library_date, prompt=excluded.prompt, model=excluded.model,
          status=excluded.status, record_json=excluded.record_json
      `);
      const deleteOutputs = db.prepare("DELETE FROM task_outputs WHERE task_id = ?");
      const insertOutput = db.prepare("INSERT INTO task_outputs (task_id, position, path) VALUES (?, ?, ?)");
      for (const t of data.tasks) {
        const up = t.updated_at || t.updatedAt || now;
        upsert.run(
          t.id, t.created_at || t.createdAt || now, up,
          t.completed_at || t.completedAt || "",
          t.library_date || (t.completed_at || t.created_at || "").slice(0, 10),
          t.prompt || "", t.model || "", t.provider_name || t.providerName || "",
          t.origin || (t.agent_session_id || t.task_group_id ? "agent" : "drawing"),
          t.task_group_id || t.taskGroupId || "", t.status || "completed",
          JSON.stringify(t)
        );
        deleteOutputs.run(t.id);
        if (t.outputs) {
          for (let i = 0; i < t.outputs.length; i++) {
            insertOutput.run(t.id, i, t.outputs[i].path || "");
          }
        }
      }
    }
  });
  tx();
  db.close();
}

// ── 合并逻辑 ──

function mergeByKey(existing, incoming, keyFn, timeFn) {
  const map = new Map();
  for (const item of existing) map.set(keyFn(item), item);
  for (const item of incoming) {
    const key = keyFn(item);
    const prev = map.get(key);
    if (!prev || (timeFn(item) > timeFn(prev))) {
      map.set(key, item);
    }
  }
  return Array.from(map.values());
}

function mergeData(desktop, browser) {
  const timeFn = (item) => item.updated_at || item.updatedAt || item.created_at || item.createdAt || "";
  const idFn = (item) => item.id;

  return {
    settings: browser.settings || desktop.settings || null,
    templates: mergeByKey(
      [...(desktop.templates || []), ...(browser.templates || [])],
      [],
      idFn,
      timeFn
    ).length ? mergeByKey(desktop.templates || [], browser.templates || [], idFn, timeFn) : [],
    agentSessions: mergeByKey(
      desktop.agentSessions || [], browser.agentSessions || [], idFn, timeFn
    ),
    tasks: mergeByKey(
      desktop.tasks || [], browser.tasks || [], idFn, timeFn
    ),
  };
}

// ── 文件服务 ──

function serveFile(req, res) {
  const urlPath = decodeURIComponent(req.url.split("?")[0]);
  const relative = urlPath.startsWith("/image-forge-data/")
    ? urlPath.slice("/image-forge-data".length)
    : null;
  if (!relative) return false;

  const filePath = join(DATA_DIR, relative);
  if (!filePath.startsWith(DATA_DIR)) {
    res.writeHead(403); res.end("Forbidden"); return true;
  }

  stat(filePath).then((info) => {
    if (!info.isFile()) { res.writeHead(404); res.end(); return; }
    const ct = MIME_MAP[extname(filePath).toLowerCase()] || "application/octet-stream";
    res.writeHead(200, { "Content-Type": ct, "Content-Length": info.size, "Cache-Control": "public, max-age=3600" });
    createReadStream(filePath).pipe(res);
  }).catch(() => {
    res.writeHead(404); res.end();
  });
  return true;
}

// ── 同步页面 ──

function syncPage() {
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <title>Image Forge 双向同步</title>
  <style>
    * { box-sizing: border-box; }
    body { font-family: system-ui, sans-serif; max-width: 640px; margin: 40px auto; padding: 20px; color: #333; background: #faf8ff; }
    h1 { font-size: 20px; margin: 0 0 8px; }
    .sub { color: #666; font-size: 13px; margin: 0 0 20px; }
    button { padding: 10px 24px; font-size: 15px; border: 0; border-radius: 8px; cursor: pointer; }
    .btn-primary { background: #7c5ce8; color: #fff; }
    .btn-primary:hover { background: #6a4dd4; }
    .status { margin: 16px 0; padding: 12px; border-radius: 8px; font-size: 13px; line-height: 1.6; }
    .status-ok { background: #e8f7f0; color: #237257; }
    .status-err { background: #fff3f6; color: #c2415b; }
    .section { margin: 20px 0; padding: 16px; border: 1px solid #ded7f2; border-radius: 10px; background: #fff; }
    .section h2 { margin: 0 0 12px; font-size: 16px; }
    .row { display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid #f0ecff; }
    .row:last-child { border-bottom: 0; }
    .row span { font-size: 14px; }
    .row .count { color: #7c5ce8; font-weight: 700; }
    .row .count.zero { color: #999; }
  </style>
</head>
<body>
  <h1>Image Forge 双向同步</h1>
  <p class="sub">合并桌面版 SQLite 和浏览器数据，以较新记录为准</p>

  <button class="btn-primary" onclick="doSync()">开始同步</button>
  <div id="result"></div>

  <div id="summary" style="display:none">
    <div class="section">
      <h2>同步结果</h2>
      <div class="row"><span>API 源配置</span><span class="count" id="s-settings">-</span></div>
      <div class="row"><span>提示词模板</span><span class="count" id="s-templates">-</span></div>
      <div class="row"><span>Agent 会话</span><span class="count" id="s-sessions">-</span></div>
      <div class="row"><span>图片库任务</span><span class="count" id="s-tasks">-</span></div>
    </div>
  </div>

  <script>
    async function doSync() {
      const el = document.getElementById("result");
      el.innerHTML = '<div class="status">⏳ 正在同步...</div>';

      try {
        // 1. 读取浏览器数据
        const browserData = {};
        ["if_settings", "if_templates", "if_agent_sessions"].forEach((key) => {
          const raw = localStorage.getItem(key);
          if (raw) browserData[key] = JSON.parse(raw);
        });

        const idb = await new Promise((resolve, reject) => {
          const req = indexedDB.open("ImageForge", 1);
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
        });
        if (idb.objectStoreNames.contains("tasks")) {
          const tx = idb.transaction("tasks", "readonly");
          const rows = await new Promise((res) => {
            const req = tx.objectStore("tasks").getAll();
            req.onsuccess = () => res(req.result);
          });
          browserData.tasks = rows.map((r) => {
            try { return JSON.parse(r.record_json); } catch { return null; }
          }).filter(Boolean);
        }
        idb.close();

        // 2. 发送到服务器合并
        const res = await fetch("/sync-merge", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            settings: browserData.if_settings || null,
            templates: browserData.if_templates || [],
            agentSessions: browserData.if_agent_sessions || [],
            tasks: browserData.tasks || [],
          }),
        });
        if (!res.ok) throw new Error("服务器返回 " + res.status);
        const merged = await res.json();

        // 3. 写回浏览器
        if (merged.settings) {
          localStorage.setItem("if_settings", JSON.stringify(merged.settings));
        }
        if (merged.templates?.length) {
          localStorage.setItem("if_templates", JSON.stringify(merged.templates));
        }
        if (merged.agentSessions?.length) {
          localStorage.setItem("if_agent_sessions", JSON.stringify(merged.agentSessions));
        }
        if (merged.tasks?.length) {
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
          for (const t of merged.tasks) {
            store.put({
              id: t.id,
              created_at: t.created_at || t.createdAt || "",
              library_date: t.library_date || (t.completed_at || t.created_at || "").slice(0, 10),
              status: t.status || "completed",
              origin: t.origin || (t.agent_session_id || t.task_group_id ? "agent" : "drawing"),
              task_group_id: t.task_group_id || t.taskGroupId || "",
              prompt: t.prompt || "",
              model: t.model || "",
              record_json: JSON.stringify(t),
            });
          }
          await new Promise((resolve, reject) => {
            tx.oncomplete = resolve;
            tx.onerror = () => reject(tx.error);
          });
          db.close();
        }

        // 4. 显示结果
        document.getElementById("s-settings").textContent = merged.settings?.providers?.length || 0;
        document.getElementById("s-settings").className = "count";
        document.getElementById("s-templates").textContent = merged.templates?.length || 0;
        document.getElementById("s-templates").className = "count";
        document.getElementById("s-sessions").textContent = merged.agentSessions?.length || 0;
        document.getElementById("s-sessions").className = "count";
        document.getElementById("s-tasks").textContent = merged.tasks?.length || 0;
        document.getElementById("s-tasks").className = "count";
        document.getElementById("summary").style.display = "block";
        el.innerHTML = '<div class="status status-ok">✅ 双向同步完成！桌面版 SQLite 和浏览器数据已合并。</div>';
      } catch (err) {
        el.innerHTML = '<div class="status status-err">❌ 同步失败：' + err.message + '</div>';
      }
    }
  </script>
</body>
</html>`;
}

// ── 服务器 ──

async function handleMerge(req, res) {
  if (req.method !== "POST") {
    res.writeHead(405); res.end("Method Not Allowed"); return;
  }
  let body = "";
  for await (const chunk of req) body += chunk;

  let browserData = { settings: null, templates: [], agentSessions: [], tasks: [] };
  try { browserData = JSON.parse(body); } catch {}

  const desktop = readSQLite();
  const merged = mergeData(desktop, browserData);

  // 写回 SQLite
  try { writeSQLite(merged); } catch (e) { console.error("写入 SQLite 失败:", e); }

  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(JSON.stringify(merged));
}

const server = createServer({
  key: readFileSync(join(CERT_DIR, `${HOST}-key.pem`)),
  cert: readFileSync(join(CERT_DIR, `${HOST}.pem`)),
}, async (req, res) => {
  if (req.url === "/sync-merge") return handleMerge(req, res);
  if (serveFile(req, res)) return;

  res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
  res.end(syncPage());
});

server.listen(PORT, () => {
  console.log(`\n  双向同步服务：https://${HOST}/\n`);
  console.log(`  1. 确保 Vite 已停止（Ctrl+C）`);
  console.log(`  2. 浏览器打开 https://${HOST}，点击「开始同步」`);
  console.log(`  3. 桌面版 SQLite 和浏览器数据自动合并`);
  console.log(`  4. 关闭页面，重启 Vite：sudo pnpm dev\n`);
  console.log(`  按 Ctrl+C 停止。\n`);
});