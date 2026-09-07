// Web 版共享数据存取层：settings / 模板 / Agent 会话。
// - 本地开发（isLocalDev）：读写 dev server 的 /image-forge-data/__* 端点，
//   直接落到与桌面版共享的 ~/.image-forge/library.sqlite。
// - 远端/兜底：沿用浏览器 localStorage（Vercel 部署场景，本轮不做迁移）。

import { isLocalDev } from './blob.js';

const KEYS = {
  settings: 'if_settings',
  templates: 'if_templates',
  sessions: 'if_agent_sessions',
};

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
  localStorage.setItem(key, JSON.stringify(value));
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

export async function readSessions() {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__sessions`);
    return Array.isArray(data?.sessions) ? data.sessions : [];
  }
  return readLocal(KEYS.sessions, []);
}

/** 单个会话 upsert（本地开发直接写 SQLite；兜底模式整表读改写） */
export async function writeSession(session) {
  if (isLocalDev()) {
    const data = await requestJson(`${DATA_ORIGIN}/__sessions`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(session),
    });
    return data?.session ?? session;
  }
  const sessions = readLocal(KEYS.sessions, []);
  const idx = sessions.findIndex((item) => item.id === session.id);
  if (idx >= 0) {
    sessions[idx] = session;
  } else {
    sessions.push(session);
  }
  writeLocal(KEYS.sessions, sessions);
  return session;
}

const sessionUpdates = new Map();

// ponytail: 串行化本页的会话读改写；跨窗口并发需由存储端提供事务。
export function updateSession(sessionId, update) {
  const pending = (sessionUpdates.get(sessionId) || Promise.resolve())
    .catch(() => {})
    .then(async () => {
      const session = (await readSessions()).find((item) => item.id === sessionId) || null;
      const updated = await update(session);
      return updated ? writeSession(updated) : null;
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
  writeLocal(
    KEYS.sessions,
    readLocal(KEYS.sessions, []).filter((item) => item.id !== sessionId)
  );
}
