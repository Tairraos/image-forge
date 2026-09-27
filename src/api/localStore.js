// Web 版共享数据存取层：settings / 模板 / Agent 会话。
// - 本地开发（isLocalDev）：读写 dev server 的 /image-forge-data/__* 端点，
//   直接落到与桌面版共享的 ~/.image-forge/library.sqlite。
// - 远端/兜底：浏览器 localStorage。会话按"单会话单键"存储（旧版单键全量重写
//   会随会话增长把每次写消息放大成整表序列化，且极易触发配额上限）。

import { isLocalDev } from './blob.js';

const KEYS = {
  settings: 'if_settings',
  templates: 'if_templates',
  // 旧版整表键，仅用于一次性迁移读取
  sessions: 'if_agent_sessions',
};
const SESSION_INDEX_KEY = 'if_agent_session_ids';
const SESSION_KEY_PREFIX = 'if_agent_session:';

const DATA_ORIGIN = '/image-forge-data';

async function requestJson(url, options) {
  const res = await fetch(url, ...(options ? [options] : []));
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`数据请求失败: HTTP ${res.status} ${text}`);
  }
  if (res.status === 204) return null;
  return res.json();
}

function readLocal(key, fallback) {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : fallback;
  } catch {
    return fallback;
  }
}

function writeLocal(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (error) {
    if (error?.name === 'QuotaExceededError' || error?.code === 22) {
      throw new Error('浏览器本地存储已满（配额超限）：请清理旧会话，或改用桌面版/本地开发模式', {
        cause: error,
      });
    }
    throw error;
  }
}

// ── 设置 ──

export async function readSettings() {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__settings`);
    return data?.settings || null;
  }
  return readLocal(KEYS.settings, null);
}

export async function writeSettings(settings) {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__settings`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });
    return data?.settings ?? settings;
  }
  writeLocal(KEYS.settings, settings);
  return settings;
}

// ── 模板 ──

export async function readTemplates() {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__templates`);
    return Array.isArray(data?.templates) ? data.templates : [];
  }
  return readLocal(KEYS.templates, []);
}

export async function writeTemplates(templates) {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__templates`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ templates }),
    });
    return Array.isArray(data?.templates) ? data.templates : templates;
  }
  writeLocal(KEYS.templates, templates);
  return templates;
}

// ── Agent 会话 ──

let legacySessionsMigrated = null;

/** 旧版把全部会话塞在一个 localStorage 键里：一次性迁移为按会话分键 */
function migrateLegacySessions() {
  if (legacySessionsMigrated) return legacySessionsMigrated;
  legacySessionsMigrated = (async () => {
    const legacy = readLocal(KEYS.sessions, null);
    if (Array.isArray(legacy) && legacy.length) {
      const ids = [];
      for (const session of legacy) {
        if (session?.id) {
          writeLocal(SESSION_KEY_PREFIX + session.id, session);
          ids.push(session.id);
        }
      }
      writeLocal(SESSION_INDEX_KEY, ids);
    }
    try {
      localStorage.removeItem(KEYS.sessions);
    } catch {
      // 清理失败不影响后续读写
    }
  })();
  return legacySessionsMigrated;
}

function readSessionIds() {
  return readLocal(SESSION_INDEX_KEY, []);
}

function readFallbackSession(sessionId) {
  return readLocal(SESSION_KEY_PREFIX + sessionId, null);
}

export async function readSessions() {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__sessions`);
    return Array.isArray(data?.sessions) ? data.sessions : [];
  }
  await migrateLegacySessions();
  return readSessionIds()
    .map((id) => readFallbackSession(id))
    .filter(Boolean);
}

/** 单个会话 upsert（本地开发直接写 SQLite；兜底模式按会话分键写，不整表重写） */
export async function writeSession(session) {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__sessions`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(session),
    });
    return data?.session ?? session;
  }
  await migrateLegacySessions();
  writeLocal(SESSION_KEY_PREFIX + session.id, session);
  const ids = readSessionIds();
  if (!ids.includes(session.id)) {
    ids.push(session.id);
    writeLocal(SESSION_INDEX_KEY, ids);
  }
  return session;
}

const sessionUpdates = new Map();

// 串行化本页的会话读改写；支持 Web Locks 的浏览器同时用跨标签页互斥锁，
// 防止两个标签页同时改同一会话互相覆盖。
export function updateSession(sessionId, update) {
  const pending = (sessionUpdates.get(sessionId) || Promise.resolve())
    .catch(() => {})
    .then(async () => {
      const run = async () => {
        let session;
        if (isLocalDev()) {
          session = (await readSessions()).find((item) => item.id === sessionId) || null;
        } else {
          await migrateLegacySessions();
          session = readFallbackSession(sessionId);
        }
        const updated = await update(session);
        return updated ? writeSession(updated) : null;
      };
      if (typeof navigator !== 'undefined' && navigator?.locks?.request) {
        return navigator.locks.request(`if-session:${sessionId}`, run);
      }
      return run();
    })
    .finally(() => {
      if (sessionUpdates.get(sessionId) === pending) sessionUpdates.delete(sessionId);
    });
  sessionUpdates.set(sessionId, pending);
  return pending;
}

export async function removeSession(sessionId) {
  if (isLocalDev()) {
    await requestJson(`${DATA_ORIGIN}/__sessions/${encodeURIComponent(sessionId)}`, {
      method: 'DELETE',
    });
    return;
  }
  await migrateLegacySessions();
  writeLocal(
    SESSION_INDEX_KEY,
    readSessionIds().filter((id) => id !== sessionId)
  );
  try {
    localStorage.removeItem(SESSION_KEY_PREFIX + sessionId);
  } catch {
    // 清理失败不影响后续读写
  }
}
