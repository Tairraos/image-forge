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
  // 本地文件路径转为开发服务器 HTTP URL
  const url = toLocalFileUrl(path);
  const res = await fetch(url);
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

/** 本地 .image-forge 路径转为开发服务器 URL */
function toLocalFileUrl(path) {
  if (!path) return path;
  const idx = path.indexOf("/.image-forge/");
  if (idx >= 0) {
    return "/image-forge-data" + path.slice(idx + "/.image-forge".length);
  }
  return path;
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

// ── 模板 ──

export async function saveTemplate(template) {
  const templates = readJSON(KEYS.templates, []);
  const idx = templates.findIndex((t) => t.id === template.id);
  if (idx >= 0) {
    templates[idx] = template;
  } else {
    templates.push(template);
  }
  writeJSON(KEYS.templates, templates);
  return templates;
}

export async function exportTemplates(destination) {
  const templates = readJSON(KEYS.templates, []);
  if (!templates.length) throw new Error("没有可导出的模板");

  const JSZip = (await import("jszip")).default;
  const zip = new JSZip();

  // manifest.json
  const manifest = {
    version: 1,
    exportedAt: new Date().toISOString(),
    templates: templates.map((t) => ({
      id: t.id,
      title: t.title,
      prompt: t.prompt,
      referencePaths: t.referencePaths || [],
      effectImagePath: t.effectImagePath || "",
      usageCount: t.usageCount || 0,
    })),
  };
  zip.file("manifest.json", JSON.stringify(manifest, null, 2));

  // 添加参考图
  for (const tpl of templates) {
    for (const refPath of tpl.referencePaths || []) {
      try {
        const res = await fetch(refPath);
        if (res.ok) {
          const blob = await res.blob();
          const fileName = refPath.split("/").pop() || "image.png";
          zip.file(`images/${fileName}`, blob);
        }
      } catch {
        // 跳过不可用的参考图
      }
    }
    if (tpl.effectImagePath) {
      try {
        const res = await fetch(tpl.effectImagePath);
        if (res.ok) {
          const blob = await res.blob();
          const fileName = tpl.effectImagePath.split("/").pop() || "effect.png";
          zip.file(`images/${fileName}`, blob);
        }
      } catch {
        // 跳过
      }
    }
  }

  const blob = await zip.generateAsync({ type: "blob" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "ImageForge-templates.zip";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return destination || "ImageForge-templates.zip";
}

export async function importTemplates(archivePath) {
  // Web 版：archivePath 可能是 File 对象或 Blob URL
  let blob;
  if (archivePath instanceof File || archivePath instanceof Blob) {
    blob = archivePath;
  } else {
    const res = await fetch(archivePath);
    if (!res.ok) throw new Error(`读取模板包失败: ${res.status}`);
    blob = await res.blob();
  }

  const JSZip = (await import("jszip")).default;
  const zip = await JSZip.loadAsync(blob);

  const manifestFile = zip.file("manifest.json");
  if (!manifestFile) throw new Error("模板包缺少 manifest.json");

  const manifest = JSON.parse(await manifestFile.async("text"));
  const incoming = manifest.templates || [];
  const existing = readJSON(KEYS.templates, []);

  let imported = 0;
  let skipped = 0;
  const existingIds = new Set(existing.map((t) => t.id));

  for (const tpl of incoming) {
    if (existingIds.has(tpl.id)) {
      skipped++;
      continue;
    }
    existing.push({
      id: tpl.id || `tpl-${Date.now()}-${imported}`,
      title: tpl.title || "",
      prompt: tpl.prompt || "",
      referencePaths: tpl.referencePaths || [],
      effectImagePath: tpl.effectImagePath || "",
      usageCount: tpl.usageCount || 0,
    });
    imported++;
  }

  writeJSON(KEYS.templates, existing);
  return { templates: existing, importedCount: imported, skippedCount: skipped };
}

export async function exportDataBundle(categories) {
  const JSZip = (await import("jszip")).default;
  const zip = new JSZip();
  const now = new Date().toISOString();

  const settings = categories.includes("settings") ? readJSON(KEYS.settings, null) : null;
  const templates = categories.includes("templates") ? readJSON(KEYS.templates, []) : [];
  const sessions = categories.includes("sessions") ? readJSON(KEYS.agentSessions, []) : [];
  let tasks = [];
  if (categories.includes("tasks")) {
    tasks = await db.getAllTasks();
  }

  // 收集所有引用文件路径
  const fileSet = new Set();
  for (const tpl of templates) {
    for (const p of tpl.referencePaths || []) fileSet.add(p);
    if (tpl.effectImagePath) fileSet.add(tpl.effectImagePath);
  }
  for (const s of sessions) {
    for (const msg of s.messages || []) {
      for (const att of msg.attachments || []) { if (att.path) fileSet.add(att.path); }
      if (msg.taskGroup) {
        for (const task of msg.taskGroup.tasks || []) {
          for (const p of task.referencePaths || []) fileSet.add(p);
          for (const o of task.outputs || []) { if (o.path) fileSet.add(o.path); }
        }
      }
    }
  }
  for (const t of tasks) {
    for (const p of t.referencePaths || []) fileSet.add(p);
    for (const o of t.outputs || []) { if (o.path) fileSet.add(o.path); }
  }

  // 按内容哈希去重，添加到 ZIP
  const added = new Set();
  for (const path of fileSet) {
    try {
      const url = toLocalFileUrl(path);
      const res = await fetch(url);
      if (!res.ok) continue;
      const blob = await res.blob();
      const hash = await sha256(await blob.arrayBuffer());
      const ext = (path.split(".").pop() || "png").split("?")[0];
      const name = `files/${hash.slice(0, 16)}.${ext}`;
      if (added.has(hash)) continue;
      added.add(hash);
      zip.file(name, blob);
    } catch { /* 文件不可访问，跳过 */ }
  }

  // 写入 manifest
  const manifest = {
    format: "image-forge-data-bundle",
    version: 1,
    exportedAt: now,
    hasSettings: !!settings,
    settings,
    templates,
    sessions,
    tasks,
  };
  zip.file("manifest.json", JSON.stringify(manifest, null, 2));

  // 生成 ZIP 并触发下载
  const zipBlob = await zip.generateAsync({ type: "blob", compression: "DEFLATE" });
  const url = URL.createObjectURL(zipBlob);
  const a = document.createElement("a");
  a.href = url;
  const date = new Date().toISOString().slice(0, 16).replace("T", "-").replace(/:/g, "");
  a.download = `export-${date}.zip`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return `export-${date}.zip`;
}

async function sha256(buffer) {
  const hash = await crypto.subtle.digest("SHA-256", buffer);
  return Array.from(new Uint8Array(hash)).map((b) => b.toString(16).padStart(2, "0")).join("");
}

export async function importDataBundle(file) {
  // Web 版接受 File 对象（来自文件选择器）
  const JSZip = (await import("jszip")).default;
  const arrayBuffer = await file.arrayBuffer();
  const zip = await JSZip.loadAsync(arrayBuffer);

  const manifestFile = zip.file("manifest.json");
  if (!manifestFile) throw new Error("ZIP 缺少 manifest.json");
  const manifest = JSON.parse(await manifestFile.async("text"));
  if (manifest.format !== "image-forge-data-bundle") throw new Error("不支持的格式");

  const result = { settings: 0, templates: 0, sessions: 0, tasks: 0 };

  // 导入设置
  if (manifest.hasSettings && manifest.settings) {
    writeJSON(KEYS.settings, manifest.settings);
    result.settings = 1;
  }

  // 导入模板
  if (manifest.templates?.length) {
    const existing = readJSON(KEYS.templates, []);
    const ids = new Set(existing.map((t) => t.id));
    for (const tpl of manifest.templates) {
      if (!ids.has(tpl.id)) { existing.push(tpl); ids.add(tpl.id); }
    }
    writeJSON(KEYS.templates, existing);
    result.templates = manifest.templates.length;
  }

  // 导入会话
  if (manifest.sessions?.length) {
    const existing = readJSON(KEYS.agentSessions, []);
    const ids = new Set(existing.map((s) => s.id));
    for (const s of manifest.sessions) {
      if (!ids.has(s.id)) { existing.push(s); ids.add(s.id); }
    }
    writeJSON(KEYS.agentSessions, existing);
    result.sessions = manifest.sessions.length;
  }

  // 导入图片库
  if (manifest.tasks?.length) {
    const existing = await db.getAllTasks();
    const ids = new Set(existing.map((t) => t.id));
    let imported = 0;
    for (const t of manifest.tasks) {
      if (!ids.has(t.id)) {
        await db.upsertTask(t);
        imported++;
      }
    }
    result.tasks = imported;
  }

  return result;
}

export async function deleteTemplate(templateId) {
  const templates = readJSON(KEYS.templates, []).filter((t) => t.id !== templateId);
  writeJSON(KEYS.templates, templates);
  return templates;
}

export async function moveTemplate(templateId, targetTemplateId) {
  const templates = readJSON(KEYS.templates, []);
  const idxA = templates.findIndex((t) => t.id === templateId);
  const idxB = templates.findIndex((t) => t.id === targetTemplateId);
  if (idxA >= 0 && idxB >= 0) {
    [templates[idxA], templates[idxB]] = [templates[idxB], templates[idxA]];
  }
  writeJSON(KEYS.templates, templates);
  return templates;
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