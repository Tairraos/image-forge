// 本地开发数据服务：把 ~/.image-forge 目录与 library.sqlite 通过 HTTP 提供给 Web 版。
// 与桌面版共享同一份数据：桌面端由 Rust 读写，Web Local 由这里（Node/better-sqlite3）读写。
// 契约对齐 src-tauri/src/history_db.rs：
//   - 任务记录 record_json 为 serde camelCase（TaskRecord），写入前必须归一化，
//     否则桌面端 parse_record 失败会导致整个图片库加载报错。
//   - settings → app_settings(key='settings')；模板 → app_templates（整表替换，position=下标）；
//     会话 → agent_sessions；任务 → tasks + task_outputs。
import { createReadStream, existsSync, unlinkSync } from 'node:fs';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, normalize, extname } from 'node:path';

const IMAGE_FORGE_DIR = join(homedir(), '.image-forge');
const DB_PATH = join(IMAGE_FORGE_DIR, 'library.sqlite');
const DEV_DATA_ORIGIN = '/image-forge-data';

const MIME_MAP = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.json': 'application/json',
};

function getMimeType(filePath) {
  return MIME_MAP[extname(filePath).toLowerCase()] || 'application/octet-stream';
}

// ── SQLite 连接 ──

let dbPromise = null;

function getDb() {
  if (!dbPromise) {
    dbPromise = (async () => {
      if (!existsSync(DB_PATH)) {
        throw new Error(`找不到数据库文件：${DB_PATH}（请先启动一次桌面版完成初始化）`);
      }
      const { default: Database } = await import('better-sqlite3');
      const db = new Database(DB_PATH);
      db.pragma('busy_timeout = 5000');
      db.pragma('foreign_keys = ON');
      return db;
    })();
  }
  return dbPromise;
}

function json(res, payload, statusCode = 200) {
  res.statusCode = statusCode;
  res.setHeader('Content-Type', 'application/json');
  res.end(JSON.stringify(payload));
}

async function readBody(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return Buffer.concat(chunks);
}

// ── 任务记录归一化：snake_case（Web 内部形状）→ camelCase（桌面 record_json 契约）──

const TASK_FIELD_MAP = {
  created_at: 'createdAt',
  updated_at: 'updatedAt',
  started_at: 'startedAt',
  completed_at: 'completedAt',
  provider_id: 'providerId',
  provider_name: 'providerName',
  reference_paths: 'referencePaths',
  task_group_id: 'taskGroupId',
  agent_session_id: 'agentSessionId',
  agent_plan: 'agentPlan',
};

const OUTPUT_FIELD_MAP = {
  file_name: 'fileName',
  mime_type: 'mimeType',
  output_format: 'outputFormat',
  revised_prompt: 'revisedPrompt',
};

function nowIso() {
  return new Date().toISOString();
}

/** 把 `/image-forge-data/...` 形式的路径还原为 ~/.image-forge 下的绝对磁盘路径 */
function toAbsPath(value) {
  const text = String(value || '');
  if (text.startsWith(`${DEV_DATA_ORIGIN}/`)) {
    return join(IMAGE_FORGE_DIR, text.slice(DEV_DATA_ORIGIN.length));
  }
  return text;
}

/** 归一化为桌面端 TaskRecord 契约；缺 required 字段时补空值，保证 Rust 端 serde 一定能解析 */
function normalizeTaskRecord(input) {
  if (!input || typeof input !== 'object' || !String(input.id || '').trim()) {
    throw new Error('任务记录缺少 id');
  }
  const source = { ...input };
  for (const [snake, camel] of Object.entries(TASK_FIELD_MAP)) {
    if (snake in source && !(camel in source)) source[camel] = source[snake];
  }
  const params = { ...(source.params || {}) };
  for (const key of Object.keys(params)) {
    const camel = key.replace(/_([a-z])/g, (_, char) => char.toUpperCase());
    if (camel !== key && !(camel in params)) params[camel] = params[key];
  }
  const outputs = (Array.isArray(source.outputs) ? source.outputs : []).map((output) => {
    const item = { ...output };
    for (const [snake, camel] of Object.entries(OUTPUT_FIELD_MAP)) {
      if (snake in item && !(camel in item)) item[camel] = item[snake];
    }
    return {
      path: toAbsPath(item.path || ''),
      fileName: item.fileName || '',
      mimeType: item.mimeType || '',
      outputFormat: item.outputFormat || 'png',
      size: item.size || '',
      background: item.background || '',
      quality: item.quality || '',
      revisedPrompt: item.revisedPrompt || '',
      usage: item.usage ?? null,
    };
  });
  return {
    id: String(source.id),
    createdAt: source.createdAt || nowIso(),
    updatedAt: source.updatedAt || nowIso(),
    startedAt: source.startedAt ?? null,
    completedAt: source.completedAt || null,
    prompt: source.prompt || '',
    providerId: source.providerId || '',
    providerName: source.providerName || '',
    mode: source.mode || '',
    model: source.model || '',
    status: source.status || 'queued',
    params,
    referencePaths: (source.referencePaths || []).map(toAbsPath),
    outputs,
    attempts: Number(source.attempts) || 0,
    error: source.error || null,
    origin: source.origin || '',
    agentSessionId: source.agentSessionId || '',
    taskGroupId: source.taskGroupId || '',
    agentPlan: source.agentPlan ?? null,
  };
}

// 与 history_db.rs library_date 一致：completedAt 优先，本地时区取 YYYY-MM-DD
function libraryDateOf(record) {
  const value = record.completedAt || record.createdAt || '';
  const date = new Date(value);
  if (!Number.isNaN(date.getTime())) {
    const pad = (n) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }
  return String(value).slice(0, 10) || '1970-01-01';
}

// 与 history_db.rs normalized_origin 一致
function normalizedOrigin(record) {
  if (record.origin === 'agent' || record.agentSessionId || record.taskGroupId) return 'agent';
  return 'drawing';
}

// ── 各数据的读写实现 ──

function readSettings(db) {
  const row = db.prepare("SELECT value FROM app_settings WHERE key = 'settings'").get();
  return row ? JSON.parse(row.value) : null;
}

function writeSettings(db, value) {
  if (!value || typeof value !== 'object' || !Array.isArray(value.providers)) {
    throw new Error('settings 必须是包含 providers 数组的对象');
  }
  db.prepare("INSERT OR REPLACE INTO app_settings (key, value) VALUES ('settings', ?)").run(
    JSON.stringify(value)
  );
  return value;
}

function readTemplates(db) {
  return db
    .prepare('SELECT record_json FROM app_templates ORDER BY position ASC')
    .all()
    .map((row) => JSON.parse(row.record_json));
}

// 与 Rust write_templates 一致：整表替换，position = 数组下标
function writeTemplates(db, records) {
  if (!Array.isArray(records)) throw new Error('templates 必须是数组');
  const now = new Date().toISOString();
  const transaction = db.transaction((list) => {
    db.prepare('DELETE FROM app_templates').run();
    const insert = db.prepare(
      'INSERT INTO app_templates (id, position, created_at, updated_at, record_json) VALUES (?, ?, ?, ?, ?)'
    );
    list.forEach((record, index) => {
      const template = {
        ...record,
        referencePaths: (record.referencePaths || []).map(toAbsPath),
        effectImagePath: toAbsPath(record.effectImagePath || ''),
      };
      const id = String(template.id || '');
      if (!id) return;
      insert.run(id, index, now, now, JSON.stringify(template));
    });
  });
  transaction(records);
  return readTemplates(db);
}

// 与 Rust upsert_agent_session 一致：列字段从 record_json 提取
function upsertSession(db, session) {
  if (!session || !String(session.id || '').trim()) throw new Error('会话记录缺少 id');
  const now = new Date().toISOString();
  const createdAt = session.createdAt || now;
  const updatedAt = session.updatedAt || now;
  db.prepare(
    'INSERT OR REPLACE INTO agent_sessions (id, created_at, updated_at, title, model_provider_id, status, record_json) VALUES (?, ?, ?, ?, ?, ?, ?)'
  ).run(
    session.id,
    createdAt,
    updatedAt,
    session.title || '',
    session.modelProviderId || '',
    session.status || 'idle',
    JSON.stringify(session)
  );
  return session;
}

function readTasks(db, limit) {
  const sql =
    'SELECT record_json FROM tasks ORDER BY created_at DESC' +
    (limit ? ` LIMIT ${Number(limit) | 0}` : '');
  return db
    .prepare(sql)
    .all()
    .map((row) => {
      try {
        return JSON.parse(row.record_json);
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

// 与 Rust upsert_in_transaction 一致：tasks upsert + task_outputs 重建，单事务
function upsertTask(db, record) {
  const normalized = normalizeTaskRecord(record);
  const transaction = db.transaction(() => {
    db.prepare(
      `INSERT INTO tasks (id, created_at, updated_at, completed_at, library_date, prompt, model, provider_name, origin, task_group_id, status, record_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           created_at=excluded.created_at, updated_at=excluded.updated_at,
           completed_at=excluded.completed_at, library_date=excluded.library_date,
           prompt=excluded.prompt, model=excluded.model, provider_name=excluded.provider_name,
           origin=excluded.origin, task_group_id=excluded.task_group_id,
           status=excluded.status, record_json=excluded.record_json`
    ).run(
      normalized.id,
      normalized.createdAt,
      normalized.updatedAt,
      normalized.completedAt || '',
      libraryDateOf(normalized),
      normalized.prompt,
      normalized.model,
      normalized.providerName,
      normalizedOrigin(normalized),
      normalized.taskGroupId,
      normalized.status,
      JSON.stringify(normalized)
    );
    db.prepare('DELETE FROM task_outputs WHERE task_id = ?').run(normalized.id);
    const insertOutput = db.prepare(
      'INSERT INTO task_outputs (task_id, position, path) VALUES (?, ?, ?)'
    );
    normalized.outputs.forEach((output, index) => {
      insertOutput.run(normalized.id, index, output.path);
    });
  });
  transaction();
  return normalized;
}

// ── 中间件 ──

/** 开发时把 ~/.image-forge 目录与 library.sqlite 通过 HTTP 提供给浏览器 */
export function serveImageForgeData() {
  return {
    name: 'serve-image-forge-data',
    configureServer(server) {
      server.middlewares.use(DEV_DATA_ORIGIN, async (req, res) => {
        const urlPath = decodeURIComponent(req.url?.split('?')[0] || '/');
        const query = new URL(req.url ?? '/', 'http://localhost').searchParams;
        const filePath = normalize(join(IMAGE_FORGE_DIR, urlPath));
        // 安全检查：确保路径在 .image-forge 目录内
        if (!filePath.startsWith(IMAGE_FORGE_DIR)) {
          res.statusCode = 403;
          res.end('Forbidden');
          return;
        }

        try {
          // ── 数据端点（读写共享的 library.sqlite）──
          if (urlPath === '/__settings' && req.method === 'GET') {
            json(res, { settings: readSettings(await getDb()) });
            return;
          }
          if (urlPath === '/__settings' && (req.method === 'PUT' || req.method === 'POST')) {
            const body = JSON.parse((await readBody(req)).toString('utf8') || '{}');
            json(res, { settings: writeSettings(await getDb(), body) });
            return;
          }
          if (urlPath === '/__templates' && req.method === 'GET') {
            json(res, { templates: readTemplates(await getDb()) });
            return;
          }
          if (urlPath === '/__templates' && (req.method === 'PUT' || req.method === 'POST')) {
            const body = JSON.parse((await readBody(req)).toString('utf8') || '[]');
            json(res, { templates: writeTemplates(await getDb(), body.templates ?? body) });
            return;
          }
          if (urlPath === '/__sessions' && req.method === 'GET') {
            const sessions = (await getDb())
              .prepare('SELECT record_json FROM agent_sessions ORDER BY updated_at DESC')
              .all()
              .map((row) => {
                try {
                  return JSON.parse(row.record_json);
                } catch {
                  return null;
                }
              })
              .filter(Boolean);
            json(res, { sessions });
            return;
          }
          if (urlPath === '/__sessions' && (req.method === 'PUT' || req.method === 'POST')) {
            const body = JSON.parse((await readBody(req)).toString('utf8') || '{}');
            json(res, { session: upsertSession(await getDb(), body) });
            return;
          }
          if (urlPath.startsWith('/__sessions/') && req.method === 'DELETE') {
            const sessionId = urlPath.slice('/__sessions/'.length);
            (await getDb()).prepare('DELETE FROM agent_sessions WHERE id = ?').run(sessionId);
            res.statusCode = 204;
            res.end();
            return;
          }
          if (urlPath === '/__tasks' && req.method === 'GET') {
            json(res, { tasks: readTasks(await getDb(), query.get('limit')) });
            return;
          }
          if (urlPath === '/__tasks' && (req.method === 'PUT' || req.method === 'POST')) {
            const body = JSON.parse((await readBody(req)).toString('utf8') || '{}');
            json(res, { task: upsertTask(await getDb(), body.task ?? body) });
            return;
          }
          if (urlPath.startsWith('/__tasks/') && req.method === 'DELETE') {
            const taskId = urlPath.slice('/__tasks/'.length);
            (await getDb()).prepare('DELETE FROM tasks WHERE id = ?').run(taskId);
            res.statusCode = 204;
            res.end();
            return;
          }
          if (urlPath === '/__library' && req.method === 'GET') {
            // 兼容旧端点：图片库 = 共享 SQLite 中有输出图的 completed 任务
            const tasks = readTasks(await getDb()).filter(
              (record) => record.status === 'completed' && record.outputs?.length > 0
            );
            json(res, { tasks });
            return;
          }

          // ── 文件端点（图片等静态资源）──
          if (req.method === 'POST' || req.method === 'PUT') {
            const body = await readBody(req);
            if (!body.length) {
              res.statusCode = 400;
              res.end('Empty body');
              return;
            }
            await mkdir(join(filePath, '..'), { recursive: true });
            await writeFile(filePath, body);
            json(res, {
              path: join(IMAGE_FORGE_DIR, urlPath),
              url: `${DEV_DATA_ORIGIN}${urlPath}`,
              size: body.length,
            });
            return;
          }
          if (req.method === 'DELETE') {
            try {
              unlinkSync(filePath);
              res.statusCode = 204;
              res.end();
            } catch (err) {
              if (err?.code === 'ENOENT') {
                res.statusCode = 404;
                res.end('Not Found');
              } else {
                res.statusCode = 500;
                res.end(String(err?.message || err));
              }
            }
            return;
          }
          const info = await stat(filePath);
          if (!info.isFile()) {
            res.statusCode = 404;
            res.end('Not Found');
            return;
          }
          res.setHeader('Content-Type', getMimeType(filePath));
          res.setHeader('Content-Length', info.size);
          res.setHeader('Cache-Control', 'public, max-age=3600');
          createReadStream(filePath).pipe(res);
        } catch (err) {
          console.error('[image-forge-data] 处理失败:', err?.message || err);
          json(res, { error: String(err?.message || err) }, 500);
        }
      });
    },
  };
}
