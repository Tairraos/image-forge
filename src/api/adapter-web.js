// Web 版适配器 — 阶段 4 实现（生图 + 队列）。
// 所有函数签名与 adapter-tauri.js 保持一致，桌面版代码无需改动。

import * as db from "./db.js";
import * as queue from "./queue.js";
import * as agent from "./agent.js";
import { uploadImage, downloadImage, deleteImage } from "./blob.js";

// ── 本地存储键 ──
const KEYS = {
  settings: "if_settings",
  templates: "if_templates",
  agentSessions: "if_agent_sessions",
};

function readJSON(key, fallback = null) {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : fallback;
  } catch {
    return fallback;
  }
}

function writeJSON(key, value) {
  localStorage.setItem(key, JSON.stringify(value));
}

// ── 应用状态 ──

export async function loadAppState() {
  const settings = readJSON(KEYS.settings) || { providers: [] };
  const history = await db.getAllTasks();
  const templates = readJSON(KEYS.templates) || [];
  // 恢复遗留的 running 任务
  await queue.recoverTasks();
  return {
    settings,
    history,
    templates,
    queue: queue.snapshot(),
  };
}

export async function aboutInfo() {
  return {
    version: import.meta.env.VITE_APP_VERSION || "1.0.0-web",
    buildTime: "",
  };
}

export async function runtimeLogs() {
  return "Web 版不支持运行日志";
}

// ── 设置 ──

export async function saveSettings(settings) {
  writeJSON(KEYS.settings, settings);
  return settings;
}

// ── Agent 会话 ──

let sessionIdCounter = Date.now();

function nextSessionId() {
  return `web-session-${++sessionIdCounter}`;
}

function readSessions() {
  return readJSON(KEYS.agentSessions, []);
}

function writeSessions(sessions) {
  writeJSON(KEYS.agentSessions, sessions);
}

export async function listAgentSessions() {
  return readSessions();
}

export async function createAgentSession(providerId) {
  const now = new Date().toISOString();
  const session = {
    id: nextSessionId(),
    title: "",
    createdAt: now,
    updatedAt: now,
    modelProviderId: providerId || "",
    messages: [],
  };
  const sessions = readSessions();
  sessions.push(session);
  writeSessions(sessions);
  return session;
}

export async function getAgentSession(sessionId) {
  const sessions = readSessions();
  return sessions.find((s) => s.id === sessionId) || null;
}

export async function deleteAgentSession(sessionId) {
  const sessions = readSessions().filter((s) => s.id !== sessionId);
  writeSessions(sessions);
}

export async function renameAgentSession(sessionId, title) {
  const sessions = readSessions();
  const session = sessions.find((s) => s.id === sessionId);
  if (session) {
    session.title = title;
    session.updatedAt = new Date().toISOString();
  }
  writeSessions(sessions);
  return session || null;
}

// ── 图片库 ──

export async function agentLibrary(month, query) {
  return db.queryAgentLibrary(month, query);
}

// ── 任务 ──

export async function deleteTask(taskId) {
  await db.deleteTask(taskId);
}

export async function getTaskStatus(taskGroupId, taskId) {
  // 按任务组 ID 查询
  const all = await db.getAllTasks();
  return all.filter((t) => t.task_group_id === taskGroupId || t.id === taskId);
}

// ── 队列 ──

export async function queueSnapshot() {
  return queue.snapshot();
}

/** 设置队列变化回调，供 UI 层监听 */
export function onQueueChange(callback) {
  queue.onQueueChange(callback);
}

// ── 生图（直接绘画模式） ──

export async function createAgentDirectImageTask(sessionId, content, attachments, plan) {
  const settings = readJSON(KEYS.settings) || { providers: [] };
  const provider = (settings.providers || []).find((p) => p.id === plan.providerId);
  if (!provider) throw new Error("找不到生图 API 配置");

  const request = {
    model: provider.imageModel || "",
    prompt: plan.prompt || content,
    ratio: plan.ratio || "1:1",
    resolution: plan.resolution || "1K",
    count: plan.count || 1,
    output_format: "png",
    quality: plan.quality || "",
    background: plan.background || "",
    reference_paths: (plan.referenceIds || []).map((id) => {
      const att = (attachments || []).find((a) => a.id === id);
      return att?.path || "";
    }).filter(Boolean),
    origin: "agent",
    agent_session_id: sessionId,
    task_group_id: `web-tg-${Date.now()}`,
  };

  const task = queue.enqueueTask(request, provider);
  return {
    id: task.task_group_id,
    sessionId,
    status: "queued",
    taskIds: [task.id],
    titles: [plan.title || "直接绘画"],
  };
}

// ── Agent 消息 ──

let agentEventListeners = [];

/** 注册 Agent 事件监听（对应 Tauri 的 listenEvent("agent-progress") 和 ("agent-task-group")） */
export function onAgentEvent(callback) {
  agentEventListeners.push(callback);
  return () => {
    agentEventListeners = agentEventListeners.filter((cb) => cb !== callback);
  };
}

function emitAgentEvent(event, payload) {
  for (const cb of agentEventListeners) {
    try { cb(event, payload); } catch {}
  }
}

export async function sendAgentMessage(sessionId, providerId, content, attachments) {
  const settings = readJSON(KEYS.settings) || { providers: [] };
  const provider = (settings.providers || []).find(
    (p) => p.id === providerId && p.modelType === "chat"
  ) || (settings.providers || []).find((p) => p.modelType === "chat");
  if (!provider) throw new Error("还没有配置对话模型");

  const sessions = readSessions();
  let session = sessions.find((s) => s.id === sessionId);
  if (!session) throw new Error("找不到 Agent 会话");

  // 添加用户消息
  const now = new Date().toISOString();
  const userMsg = {
    id: `web-msg-${Date.now()}`,
    role: "user",
    content,
    createdAt: now,
    attachments: (attachments || []).map((a) => ({
      id: a.id,
      path: a.path || "",
      fileName: a.fileName || "image.png",
      mimeType: a.mimeType || "image/png",
      dataUrl: a.dataUrl || "",
    })),
  };
  session.messages = [...(session.messages || []), userMsg];

  try {
    session = await agent.runAgentTurn(provider, session, content, attachments, (event) => {
      emitAgentEvent("agent-progress", event);
    });
  } catch (error) {
    const errorMsg = {
      id: `web-msg-${Date.now()}`,
      role: "assistant",
      content: "",
      error: error.message || String(error),
      createdAt: new Date().toISOString(),
    };
    session.messages = [...(session.messages || []), errorMsg];
    emitAgentEvent("agent-progress", {
      phase: "error",
      message: error.message || "Agent 调用失败",
      sessionId,
    });
  }

  // 检查是否有任务组创建
  const lastMsg = session.messages?.[session.messages.length - 1];
  if (lastMsg?.taskGroup?.id) {
    emitAgentEvent("agent-task-group", {
      ...lastMsg.taskGroup,
      sessionId,
      tasks: lastMsg.taskGroup.taskIds?.length || 0,
    });
  }

  // 保存会话
  const allSessions = readSessions();
  const idx = allSessions.findIndex((s) => s.id === session.id);
  if (idx >= 0) {
    allSessions[idx] = session;
  } else {
    allSessions.push(session);
  }
  writeSessions(allSessions);

  return session;
}

export async function cancelAgentTurn() {
  // Web 版可通过 AbortController 实现，当前占位
}

export async function cancelAgentTaskGroup(taskGroupId) {
  queue.cancelTaskGroup(taskGroupId);
}

export async function retryAgentTaskGroup(taskGroupId) {
  queue.retryTaskGroup(taskGroupId);
}

export async function referenceFromPath(path) {
  // Web 版：通过 fetch 读取本地 Blob URL 或远程 URL
  const res = await fetch(path);
  if (!res.ok) throw new Error(`读取图片失败: ${res.status}`);
  const blob = await res.blob();
  const dataUrl = await blobToDataUrl(blob);
  const fileName = path.split("/").pop() || "image.png";
  return {
    path,
    fileName,
    mimeType: blob.type || "image/png",
    dataUrl,
  };
}

export async function referenceFromClipboard() {
  try {
    const items = await navigator.clipboard.read();
    for (const item of items) {
      const imageType = item.types.find((t) => t.startsWith("image/"));
      if (imageType) {
        const blob = await item.getType(imageType);
        const dataUrl = await blobToDataUrl(blob);
        const path = `clipboard-${Date.now()}.png`;
        return { path, fileName: "clipboard.png", mimeType: imageType, dataUrl };
      }
    }
  } catch {
    // 剪贴板权限未授予或无可读图片
  }
  return null;
}

function blobToDataUrl(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

// ── 输出 ──

export async function downloadOutput(path) {
  // Web 版：触发浏览器下载
  const res = await fetch(path);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = path.split("/").pop() || "image.png";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return path;
}

export async function revealPath() {
  // Web 版不支持 Finder 定位
}

// ── 模板（阶段 7 实现，先占位） ──

export async function saveTemplate() {
  throw new Error("Web 版模板系统尚未实现");
}
export async function exportTemplates() {
  throw new Error("Web 版模板导出尚未实现");
}
export async function importTemplates() {
  throw new Error("Web 版模板导入尚未实现");
}
export async function deleteTemplate() {
  throw new Error("Web 版模板系统尚未实现");
}
export async function moveTemplate() {
  throw new Error("Web 版模板系统尚未实现");
}

// ── 清理 ──

export async function scanCleanupCandidates() {
  return [];
}
export async function cleanupDataFiles() {
  return [];
}

// ── 工具 ──

export async function readClipboardText() {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return "";
  }
}

export async function listProviderModels(provider) {
  const baseUrl = (provider.baseUrl || "").replace(/\/+$/, "");
  const apiKey = provider.apiKey || "";
  if (!baseUrl || !apiKey) throw new Error("缺少 API 地址或 Key");

  const res = await fetch(`${baseUrl}/models`, {
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`获取模型列表失败: ${res.status} ${text}`);
  }
  const data = await res.json();
  return (data.data || data.models || data || [])
    .map((m) => m.id || m.name || m)
    .filter((id) => typeof id === "string");
}