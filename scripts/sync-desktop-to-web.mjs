// 双向同步：SQLite ↔ 浏览器 IndexedDB/localStorage
// 合并策略：以 updated_at 或 created_at 较新的记录为准
//
// 用法：
//   node scripts/sync-desktop-to-web.mjs --serve  # 启动双向同步服务

import { createServer } from 'node:https';
import { homedir } from 'node:os';
import { join, extname } from 'node:path';
import { createReadStream, readFileSync, existsSync, readdirSync } from 'node:fs';
import { stat } from 'node:fs/promises';
import Database from 'better-sqlite3';

const DATA_DIR = join(homedir(), '.image-forge');
const SQLITE_FILE = join(DATA_DIR, 'library.sqlite');
const PORT = 443;
const HOST = 'image.xiaole.qzz.io';
const CERT_DIR = join(DATA_DIR, 'certs');

const MIME_MAP = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.json': 'application/json',
};

// ── JSON → SQLite 迁移 ──

function migrateJsonToSQLite(db) {
  const now = new Date().toISOString();

  // settings.json
  try {
    const settingsPath = join(DATA_DIR, 'settings.json');
    if (existsSync(settingsPath)) {
      const text = readFileSync(settingsPath, 'utf-8');
      const existing = db.prepare("SELECT value FROM app_settings WHERE key = 'settings'").get();
      if (!existing) {
        db.prepare("INSERT INTO app_settings (key, value) VALUES ('settings', ?)").run(text);
        console.log('  ✅ 已迁移 settings.json → SQLite');
      }
    }
  } catch { /* ignore */ }

  // prompt-templates.json
  try {
    const tplPath = join(DATA_DIR, 'prompt-templates.json');
    if (existsSync(tplPath)) {
      const existing = db.prepare('SELECT COUNT(*) as cnt FROM app_templates').get();
      if (existing.cnt === 0) {
        const text = readFileSync(tplPath, 'utf-8');
        const templates = JSON.parse(text);
        const insert = db.prepare(
          'INSERT INTO app_templates (id, position, created_at, updated_at, record_json) VALUES (?, ?, ?, ?, ?)'
        );
        for (let i = 0; i < templates.length; i++) {
          const t = templates[i];
          insert.run(
            t.id || `tpl-${i}`,
            i,
            t.createdAt || now,
            t.updatedAt || now,
            JSON.stringify(t)
          );
        }
        console.log(`  ✅ 已迁移 prompt-templates.json → SQLite（${templates.length} 个模板）`);
      }
    }
  } catch { /* ignore */ }

  // agent/sessions/*.json
  try {
    const sessionsDir = join(DATA_DIR, 'agent', 'sessions');
    if (existsSync(sessionsDir)) {
      const existing = db.prepare('SELECT COUNT(*) as cnt FROM agent_sessions').get();
      if (existing.cnt === 0) {
        const files = readdirSync(sessionsDir).filter((f) => f.endsWith('.json'));
        const upsert = db.prepare(
          'INSERT INTO agent_sessions (id, created_at, updated_at, title, model_provider_id, status, record_json) VALUES (?, ?, ?, ?, ?, ?, ?)'
        );
        for (const file of files) {
          const text = readFileSync(join(sessionsDir, file), 'utf-8');
          const s = JSON.parse(text);
          upsert.run(
            s.id,
            s.created_at || s.createdAt || now,
            s.updated_at || s.updatedAt || now,
            s.title || '',
            s.model_provider_id || s.modelProviderId || '',
            s.status || 'idle',
            text
          );
        }
        console.log(`  ✅ 已迁移 agent/sessions/ → SQLite（${files.length} 个会话）`);
      }
    }
  } catch { /* ignore */ }

  // queue.json
  try {
    const queuePath = join(DATA_DIR, 'queue.json');
    if (existsSync(queuePath)) {
      const existing = db.prepare('SELECT COUNT(*) as cnt FROM app_queue').get();
      if (existing.cnt === 0) {
        const text = readFileSync(queuePath, 'utf-8');
        const queue = JSON.parse(text);
        const waiting = queue.waiting || [];
        const running = queue.running || [];
        const insert = db.prepare(
          'INSERT INTO app_queue (id, position, status, provider_id, record_json) VALUES (?, ?, ?, ?, ?)'
        );
        for (let i = 0; i < waiting.length; i++) {
          insert.run(waiting[i], i, 'waiting', '', JSON.stringify({ taskId: waiting[i] }));
        }
        for (let i = 0; i < running.length; i++) {
          const r = running[i];
          insert.run(
            r.task_id,
            waiting.length + i,
            'running',
            r.provider_id || '',
            JSON.stringify(r)
          );
        }
        console.log(`  ✅ 已迁移 queue.json → SQLite`);
      }
    }
  } catch { /* ignore */ }
}

function ensureTables(db) {
  db.exec(`
    CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS app_templates (id TEXT PRIMARY KEY, position INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, record_json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS agent_sessions (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', model_provider_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'idle', record_json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS app_queue (id TEXT PRIMARY KEY, position INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'waiting', provider_id TEXT NOT NULL DEFAULT '', record_json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT NOT NULL, library_date TEXT NOT NULL, prompt TEXT NOT NULL, model TEXT NOT NULL, provider_name TEXT NOT NULL, origin TEXT NOT NULL, task_group_id TEXT NOT NULL, status TEXT NOT NULL, record_json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS task_outputs (task_id TEXT NOT NULL, position INTEGER NOT NULL, path TEXT NOT NULL, PRIMARY KEY (task_id, position));
  `);
  migrateJsonToSQLite(db);
}

// ── SQLite 读取 ──

function readSQLite() {
  const db = new Database(SQLITE_FILE);
  ensureTables(db);
  const data = {
    settings: null,
    templates: [],
    agentSessions: [],
    tasks: [],
  };

  try {
    const settingsRow = db.prepare("SELECT value FROM app_settings WHERE key = 'settings'").get();
    if (settingsRow) data.settings = JSON.parse(settingsRow.value);
  } catch { /* ignore */ }

  try {
    const tpls = db.prepare('SELECT record_json FROM app_templates ORDER BY position ASC').all();
    data.templates = tpls.map((r) => JSON.parse(r.record_json));
  } catch { /* ignore */ }

  try {
    const sessions = db
      .prepare('SELECT record_json FROM agent_sessions ORDER BY updated_at DESC')
      .all();
    data.agentSessions = sessions.map((r) => JSON.parse(r.record_json));
  } catch { /* ignore */ }

  try {
    const tasks = db.prepare('SELECT record_json FROM tasks ORDER BY created_at DESC').all();
    data.tasks = tasks.map((r) => JSON.parse(r.record_json));
  } catch { /* ignore */ }

  db.close();
  return data;
}

// ── SQLite 写入 ──

function writeSQLite(data) {
  const db = new Database(SQLITE_FILE);
  ensureTables(db);
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
      db.prepare('DELETE FROM app_templates').run();
      const insert = db.prepare(
        'INSERT INTO app_templates (id, position, created_at, updated_at, record_json) VALUES (?, ?, ?, ?, ?)'
      );
      for (let i = 0; i < data.templates.length; i++) {
        const t = data.templates[i];
        insert.run(
          t.id || `tpl-${i}`,
          i,
          t.createdAt || now,
          t.updatedAt || now,
          JSON.stringify(t)
        );
      }
    }

    // 会话
    if (data.agentSessions?.length) {
      const upsert = db.prepare(
        'INSERT OR REPLACE INTO agent_sessions (id, created_at, updated_at, title, model_provider_id, status, record_json) VALUES (?, ?, ?, ?, ?, ?, ?)'
      );
      for (const s of data.agentSessions) {
        upsert.run(
          s.id,
          s.createdAt || now,
          s.updatedAt || now,
          s.title || '',
          s.modelProviderId || s.model_provider_id || '',
          s.status || 'idle',
          JSON.stringify(s)
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
      const deleteOutputs = db.prepare('DELETE FROM task_outputs WHERE task_id = ?');
      const insertOutput = db.prepare(
        'INSERT INTO task_outputs (task_id, position, path) VALUES (?, ?, ?)'
      );
      for (const t of data.tasks) {
        const up = t.updated_at || t.updatedAt || now;
        upsert.run(
          t.id,
          t.created_at || t.createdAt || now,
          up,
          t.completed_at || t.completedAt || '',
          t.library_date || (t.completed_at || t.created_at || '').slice(0, 10),
          t.prompt || '',
          t.model || '',
          t.provider_name || t.providerName || '',
          t.origin || (t.agent_session_id || t.task_group_id ? 'agent' : 'drawing'),
          t.task_group_id || t.taskGroupId || '',
          t.status || 'completed',
          JSON.stringify(t)
        );
        deleteOutputs.run(t.id);
        if (t.outputs) {
          for (let i = 0; i < t.outputs.length; i++) {
            insertOutput.run(t.id, i, t.outputs[i].path || '');
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
    if (!prev || timeFn(item) > timeFn(prev)) {
      map.set(key, item);
    }
  }
  return Array.from(map.values());
}

function mergeData(desktop, browser) {
  const timeFn = (item) =>
    item.updated_at || item.updatedAt || item.created_at || item.createdAt || '';
  const idFn = (item) => item.id;

  return {
    settings: browser.settings || desktop.settings || null,
    templates: mergeByKey(
      [...(desktop.templates || []), ...(browser.templates || [])],
      [],
      idFn,
      timeFn
    ).length
      ? mergeByKey(desktop.templates || [], browser.templates || [], idFn, timeFn)
      : [],
    agentSessions: mergeByKey(
      desktop.agentSessions || [],
      browser.agentSessions || [],
      idFn,
      timeFn
    ),
    tasks: mergeByKey(desktop.tasks || [], browser.tasks || [], idFn, timeFn),
  };
}

// ── 文件服务 ──

function serveFile(req, res) {
  const urlPath = decodeURIComponent(req.url.split('?')[0]);
  const relative = urlPath.startsWith('/image-forge-data/')
    ? urlPath.slice('/image-forge-data'.length)
    : null;
  if (!relative) return false;

  const filePath = join(DATA_DIR, relative);
  if (!filePath.startsWith(DATA_DIR)) {
    res.writeHead(403);
    res.end('Forbidden');
    return true;
  }

  stat(filePath)
    .then((info) => {
      if (!info.isFile()) {
        res.writeHead(404);
        res.end();
        return;
      }
      const ct = MIME_MAP[extname(filePath).toLowerCase()] || 'application/octet-stream';
      res.writeHead(200, {
        'Content-Type': ct,
        'Content-Length': info.size,
        'Cache-Control': 'public, max-age=3600',
      });
      createReadStream(filePath).pipe(res);
    })
    .catch(() => {
      res.writeHead(404);
      res.end();
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
    .btn-row { display: flex; gap: 12px; margin-bottom: 16px; }
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

  <div class="btn-row">
    <button class="btn-primary" onclick="doSync('web-to-app')">Web → App</button>
    <button class="btn-primary" onclick="doSync('app-to-web')">App → Web</button>
  </div>
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
    async function doSync(direction) {
      const el = document.getElementById("result");
      const label = direction === "web-to-app" ? "Web → App" : "App → Web";
      el.innerHTML = '<div class="status">⏳ 正在同步 ' + label + '...</div>';

      try {
        // 1. 读取浏览器数据
        const browserData = {};
        ["if_settings", "if_templates", "if_agent_sessions"].forEach((key) => {
          const raw = localStorage.getItem(key);
          if (raw) browserData[key] = JSON.parse(raw);
        });

        const idb = await new Promise((r, j) => { const req = indexedDB.open("ImageForge"); req.onsuccess = () => r(req.result); req.onerror = () => j(req.error); });
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

        // 2. 发送到服务器
        const res = await fetch("/sync-merge", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            direction,
            settings: browserData.if_settings || null,
            templates: browserData.if_templates || [],
            agentSessions: browserData.if_agent_sessions || [],
            tasks: browserData.tasks || [],
          }),
        });
        if (!res.ok) throw new Error("服务器返回 " + res.status);
        const merged = await res.json();

        // 3. 写回浏览器（仅 app-to-web 方向）
        if (direction === "app-to-web") {
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
            const db = await new Promise((r, j) => { const req = indexedDB.open("ImageForge"); req.onsuccess = () => r(req.result); req.onerror = () => j(req.error); });
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
            await new Promise((r, j) => { tx.oncomplete = r; tx.onerror = () => j(tx.error); });
            db.close();
          }
        }

        // 4. 显示结果
        document.getElementById("s-settings").textContent = merged.settings?.providers?.length || 0;
        document.getElementById("s-templates").textContent = merged.templates?.length || 0;
        document.getElementById("s-sessions").textContent = merged.agentSessions?.length || 0;
        document.getElementById("s-tasks").textContent = merged.tasks?.length || 0;
        document.getElementById("summary").style.display = "block";
        el.innerHTML = '<div class="status status-ok">✅ ' + label + ' 同步完成！</div>';
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
  if (req.method !== 'POST') {
    res.writeHead(405);
    res.end('Method Not Allowed');
    return;
  }
  let body = '';
  for await (const chunk of req) body += chunk;

  let bodyData = {};
  try {
    bodyData = JSON.parse(body);
  } catch { /* ignore */ }

  const browserData = {
    settings: bodyData.settings || null,
    templates: bodyData.templates || [],
    agentSessions: bodyData.agentSessions || [],
    tasks: bodyData.tasks || [],
  };
  const direction = bodyData.direction || 'app-to-web';

  const desktop = readSQLite();
  const merged = mergeData(desktop, browserData);

  if (direction === 'web-to-app') {
    // Web → App：合并后写回 SQLite
    try {
      writeSQLite(merged);
    } catch (e) {
      console.error('写入 SQLite 失败:', e);
    }
  }

  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(merged));
}

const server = createServer(
  {
    key: readFileSync(join(CERT_DIR, `${HOST}-key.pem`)),
    cert: readFileSync(join(CERT_DIR, `${HOST}.pem`)),
  },
  async (req, res) => {
    if (req.url === '/sync-merge') return handleMerge(req, res);
    if (serveFile(req, res)) return;

    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
    res.end(syncPage());
  }
);

server.listen(PORT, () => {
  console.log(`\n  双向同步服务：https://${HOST}/\n`);
  console.log(`  1. 确保 Vite 已停止（Ctrl+C）`);
  console.log(`  2. 浏览器打开 https://${HOST}，点击「开始同步」`);
  console.log(`  3. 桌面版 SQLite 和浏览器数据自动合并`);
  console.log(`  4. 关闭页面，重启 Vite：sudo pnpm dev\n`);
  console.log(`  按 Ctrl+C 停止。\n`);
});
