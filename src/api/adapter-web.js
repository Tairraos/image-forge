// Web 版适配器 — 阶段 4 实现（生图 + 队列）。
// 所有函数签名与 adapter-tauri.js 保持一致，桌面版代码无需改动。

import * as db from './db.js';
import * as queue from './queue.js';
import * as agent from './agent.js';
import { uploadImage, isLocalDev } from './blob.js';

// ── 本地存储键 ──
const KEYS = {
  settings: 'if_settings',
  templates: 'if_templates',
  agentSessions: 'if_agent_sessions',
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
    version: import.meta.env.VITE_APP_VERSION || '1.0.0-web',
    buildTime: '',
  };
}

export async function runtimeLogs() {
  return 'Web 版不支持运行日志';
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
    title: '',
    createdAt: now,
    updatedAt: now,
    modelProviderId: providerId || '',
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
  const localRecords = await db.getCompletedLibraryRecords();
  let records = localRecords;
  if (isLocalDev()) {
    // 本地开发：合并展示桌面版 ~/.image-forge/library.sqlite 的任务（图片路径已被
    // dev server 改写为 /image-forge-data/ URL），同一任务以桌面版记录为准。
    try {
      const res = await fetch('/image-forge-data/__library');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const remote = await res.json();
      const seen = new Set();
      records = [...(remote.tasks || []), ...localRecords].filter((task) => {
        if (!task?.id || seen.has(task.id)) return false;
        seen.add(task.id);
        return true;
      });
    } catch (error) {
      console.warn('读取 ~/.image-forge 图片库失败，仅显示浏览器内任务:', error);
    }
  }
  return db.buildLibraryPage(records, month, query);
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
  if (!provider) throw new Error('找不到生图 API 配置');

  const request = {
    model: provider.imageModel || '',
    prompt: plan.prompt || content,
    ratio: plan.ratio || '1:1',
    resolution: plan.resolution || '1K',
    count: plan.count || 1,
    output_format: 'png',
    quality: plan.quality || '',
    background: plan.background || '',
    reference_paths: (plan.referenceIds || [])
      .map((id) => {
        const att = (attachments || []).find((a) => a.id === id);
        return att?.path || '';
      })
      .filter(Boolean),
    origin: 'agent-direct',
    agent_session_id: sessionId,
    task_group_id: `web-tg-${Date.now()}`,
  };

  const task = queue.enqueueTask(request, provider);
  return {
    id: task.task_group_id,
    sessionId,
    status: 'queued',
    taskIds: [task.id],
    titles: [plan.title || '直接绘画'],
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
    try {
      cb(event, payload);
    } catch {
      /* ignore listener errors */
    }
  }
}

export async function sendAgentMessage(sessionId, providerId, content, attachments) {
  const settings = readJSON(KEYS.settings) || { providers: [] };
  const provider =
    (settings.providers || []).find((p) => p.id === providerId && p.modelType === 'chat') ||
    (settings.providers || []).find((p) => p.modelType === 'chat');
  if (!provider) throw new Error('还没有配置对话模型');

  const sessions = readSessions();
  let session = sessions.find((s) => s.id === sessionId);
  if (!session) throw new Error('找不到 Agent 会话');

  // 添加用户消息
  const now = new Date().toISOString();
  const userMsg = {
    id: `web-msg-${Date.now()}`,
    role: 'user',
    content,
    createdAt: now,
    attachments: (attachments || []).map((a) => ({
      id: a.id,
      path: a.path || '',
      fileName: a.fileName || 'image.png',
      mimeType: a.mimeType || 'image/png',
      dataUrl: a.dataUrl || '',
    })),
  };
  session.messages = [...(session.messages || []), userMsg];

  try {
    session = await agent.runAgentTurn(provider, session, content, attachments, (event) => {
      emitAgentEvent('agent-progress', event);
    });
  } catch (error) {
    const errorMsg = {
      id: `web-msg-${Date.now()}`,
      role: 'assistant',
      content: '',
      error: error.message || String(error),
      createdAt: new Date().toISOString(),
    };
    session.messages = [...(session.messages || []), errorMsg];
    emitAgentEvent('agent-progress', {
      phase: 'error',
      message: error.message || 'Agent 调用失败',
      sessionId,
    });
  }

  // 检查是否有任务组创建
  const lastMsg = session.messages?.[session.messages.length - 1];
  if (lastMsg?.taskGroup?.id) {
    emitAgentEvent('agent-task-group', {
      ...lastMsg.taskGroup,
      sessionId,
      // 与桌面版事件保持一致：tasks 是任务 id 数组（App.vue 读取 group.tasks.length）
      tasks: lastMsg.taskGroup.taskIds || [],
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

// 与桌面 chat.rs 的 TEMPLATE_SYSTEM_PROMPT 保持一致
const TEMPLATE_FILL_SYSTEM_PROMPT =
  '你是提示词模板填充助手。用户会提供一个包含若干 {占位描述} 的生图提示词模板。请根据花括号里的描述语义，把每一处花括号连同里面的描述替换为具体、自然、适合生图的中文内容。不要保留花括号，不要改变花括号外的其它文字，不要输出解释、Markdown 或代码块，只输出填充后的完整文本。';

export async function fillPromptTemplate(sessionId, providerId, template) {
  void sessionId;
  const content = String(template || '').trim();
  if (!content) throw new Error('模板内容不能为空');
  const settings = readJSON(KEYS.settings) || { providers: [] };
  const provider =
    (settings.providers || []).find((p) => p.id === providerId && p.modelType === 'chat') ||
    (settings.providers || []).find((p) => p.modelType === 'chat');
  if (!provider) throw new Error('请选择可用的对话模型');
  if (!(provider.apiKey || '').trim()) {
    throw new Error(`对话模型「${provider.name}」还没有填写 API Key`);
  }
  const baseUrl = (provider.baseUrl || '').replace(/\/+$/, '');
  const res = await fetch(`${baseUrl}/chat/completions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${(provider.apiKey || '').trim()}`,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      model: provider.imageModel || 'gpt-4o',
      temperature: 0.2,
      stream: false,
      messages: [
        { role: 'system', content: TEMPLATE_FILL_SYSTEM_PROMPT },
        { role: 'user', content },
      ],
    }),
  });
  if (!res.ok) {
    let msg = await res.text();
    try {
      msg = JSON.parse(msg).error?.message || msg;
    } catch {
      /* ignore parse error */
    }
    throw new Error(`模板填充失败: HTTP ${res.status} ${msg}`);
  }
  const json = await res.json();
  return json.choices?.[0]?.message?.content?.trim() || '';
}

export async function cancelAgentTaskGroup(taskGroupId) {
  queue.cancelTaskGroup(taskGroupId);
}

export async function retryAgentTaskGroup(taskGroupId) {
  queue.retryTaskGroup(taskGroupId);
}

export async function redrawTask(taskId) {
  const tasks = await db.getAllTasks();
  const source = tasks.find((t) => t.id === taskId);
  if (!source) throw new Error('找不到要重画的任务');
  const settings = readJSON(KEYS.settings) || { providers: [] };
  const provider =
    (settings.providers || []).find((p) => p.id === source.provider_id) ||
    (settings.providers || []).find((p) => p.modelType !== 'chat') ||
    settings.providers?.[0];
  if (!provider) throw new Error('没有可用的生图 API 配置');
  const request = {
    prompt: source.prompt || '',
    ratio: source.params?.ratio || '1:1',
    resolution: source.params?.resolution || '1K',
    count: source.params?.count || 1,
    output_format: source.params?.output_format || 'png',
    quality: source.params?.quality || '',
    background: source.params?.background || '',
    size: source.params?.size || '',
    reference_paths: source.reference_paths || [],
    origin: source.origin || 'agent',
    agent_session_id: source.agent_session_id || '',
    task_group_id: source.task_group_id || `web-tg-${Date.now()}`,
  };
  const task = queue.enqueueTask(request, provider);
  return {
    id: source.task_group_id || task.task_group_id,
    status: 'queued',
    taskIds: [task.id],
  };
}

export async function referenceFromPath(path) {
  // 本地文件路径转为开发服务器 HTTP URL
  const url = toLocalFileUrl(path);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`读取图片失败: ${res.status}`);
  const blob = await res.blob();
  const dataUrl = await blobToDataUrl(blob);
  const fileName = path.split('/').pop() || 'image.png';
  return {
    path,
    fileName,
    mimeType: blob.type || 'image/png',
    dataUrl,
  };
}

/** 本地 .image-forge 路径转为开发服务器 URL */
function toLocalFileUrl(path) {
  if (!path) return path;
  const idx = path.indexOf('/.image-forge/');
  if (idx >= 0) {
    return '/image-forge-data' + path.slice(idx + '/.image-forge'.length);
  }
  return path;
}

export async function referenceFromClipboard() {
  try {
    const items = await navigator.clipboard.read();
    for (const item of items) {
      const imageType = item.types.find((t) => t.startsWith('image/'));
      if (imageType) {
        const blob = await item.getType(imageType);
        // 把剪贴板图片直接写入 ~/.image-forge/references/，避免任何 data URL 持久化到会话/模板
        const fileName = `clipboard-${Date.now()}.${imageType.split('/')[1] || 'png'}`;
        const imageUrl = await uploadImage(fileName, blob, 'references');
        return {
          path: imageUrl,
          fileName,
          mimeType: imageType,
          dataUrl: imageUrl,
        };
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
  const a = document.createElement('a');
  a.href = url;
  a.download = path.split('/').pop() || 'image.png';
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

// ── 模板包（与桌面版 template_bundle.rs 同一格式契约，ZIP 可互相导入导出）──

const BUNDLE_FORMAT = 'image-forge-template-bundle';
const BUNDLE_VERSION = 1;
const BUNDLE_MANIFEST_NAME = 'manifest.json';
const BUNDLE_MARKDOWN_NAME = 'ImageForge-templates.md';
const BUNDLE_MAX_ARCHIVE_BYTES = 256 * 1024 * 1024;
const BUNDLE_MAX_ENTRY_COUNT = 2000;
const BUNDLE_MAX_READ_BYTES = 1024 * 1024 * 1024;
const BUNDLE_MAX_MANIFEST_BYTES = 10 * 1024 * 1024;
const BUNDLE_MAX_MARKDOWN_BYTES = 20 * 1024 * 1024;
const BUNDLE_MAX_IMAGE_BYTES = 100 * 1024 * 1024;
const BUNDLE_MAX_TEMPLATE_COUNT = 10000;
const BUNDLE_MAX_REFERENCES_PER_TEMPLATE = 64;

function bytesStartWith(bytes, prefix) {
  if (bytes.length < prefix.length) return false;
  return prefix.every((value, index) => bytes[index] === value);
}

// 与桌面版 utils::image_mime_type 相同的字节嗅探规则
function detectImageMime(path, bytes) {
  let mime = '';
  if (bytesStartWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
    mime = 'image/png';
  } else if (bytesStartWith(bytes, [0xff, 0xd8, 0xff])) {
    mime = 'image/jpeg';
  } else if (
    bytesStartWith(bytes, [0x52, 0x49, 0x46, 0x46]) &&
    bytes.length > 12 &&
    bytesStartWith(bytes.slice(8, 12), [0x57, 0x45, 0x42, 0x50])
  ) {
    mime = 'image/webp';
  } else if (
    bytesStartWith(bytes, [0x47, 0x49, 0x46, 0x38, 0x37, 0x61]) ||
    bytesStartWith(bytes, [0x47, 0x49, 0x46, 0x38, 0x39, 0x61])
  ) {
    mime = 'image/gif';
  }
  if (!mime) {
    const ext = path.split('.').pop()?.toLowerCase() || '';
    const guessed = {
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      png: 'image/png',
      webp: 'image/webp',
      gif: 'image/gif',
    }[ext];
    mime = guessed || '';
  }
  if (!mime.startsWith('image/')) throw new Error('只支持图像文件');
  return mime;
}

// 与桌面版 extension_for_mime 相同的扩展名映射
function extensionForMime(mime) {
  if (mime === 'image/jpeg') return 'jpg';
  if (mime === 'image/webp') return 'webp';
  if (mime === 'image/gif') return 'gif';
  return 'png';
}

function validateBundleImagePath(value) {
  const unsafe = !value.startsWith('images/') || value.includes('\\');
  const segments = value.split('/');
  if (unsafe || segments.some((segment) => !segment || segment === '.' || segment === '..')) {
    throw new Error(`模板包包含不安全的参考图路径：${value}`);
  }
}

function validateBundleTemplates(templates) {
  if (!templates.length) throw new Error('模板包中没有模板');
  if (templates.length > BUNDLE_MAX_TEMPLATE_COUNT) throw new Error('模板包中的模板数量过多');
  for (const template of templates) {
    if (!String(template.content || '').trim()) throw new Error('模板包包含空提示词');
    if (template.references.length > BUNDLE_MAX_REFERENCES_PER_TEMPLATE) {
      throw new Error('单个模板的参考图数量超过 64 张');
    }
    for (const reference of template.references) validateBundleImagePath(reference);
    if (template.effectImage) validateBundleImagePath(template.effectImage);
  }
}

// 与桌面版 build_templates_markdown 同一结构，供人工检视模板内容
function buildTemplatesMarkdown(templates, exportedAt) {
  let markdown = `# Image Forge 提示词模板\n\n> 导出时间：${exportedAt}\n> 模板数量：${templates.length}\n\n`;
  for (const template of templates) {
    markdown += `---\n\n## 模板 ${template.sourceId} · ${template.title}\n\n${template.content}\n\n`;
    if (template.references.length) {
      markdown += '### 参考图\n\n';
      template.references.forEach((archivePath, index) => {
        markdown += `![模板 ${template.sourceId} 参考图 ${index + 1}](${archivePath})\n\n`;
      });
    }
    if (template.effectImage) {
      markdown += `### 效果图\n\n![模板 ${template.sourceId} 效果图](${template.effectImage})\n\n`;
    }
  }
  return markdown;
}

// 导出模板包：manifest.json + ImageForge-templates.md + images/<sha256>.<ext>，
// 与桌面版导出的 ZIP 结构完全一致。参考图按内容 SHA-256 去重。
export async function exportTemplates(destination) {
  const templates = readJSON(KEYS.templates, []);
  if (!templates.length) throw new Error('没有可导出的模板');

  const JSZip = (await import('jszip')).default;
  const zip = new JSZip();

  const archivePaths = new Map();
  const uniqueImages = new Map();
  const collectImage = async (rawPath) => {
    const source = String(rawPath || '').trim();
    if (!source) return '';
    const cached = archivePaths.get(source);
    if (cached !== undefined) return cached;
    const res = await fetch(source);
    if (!res.ok) throw new Error(`找不到模板参考图：${source}`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    const archivePath = `images/${await sha256(bytes)}.${extensionForMime(detectImageMime(source, bytes))}`;
    archivePaths.set(source, archivePath);
    if (!uniqueImages.has(archivePath)) uniqueImages.set(archivePath, bytes);
    return archivePath;
  };

  const bundleTemplates = [];
  for (const template of templates) {
    const references = [];
    for (const rawPath of template.referencePaths || []) {
      const archivePath = await collectImage(rawPath);
      if (archivePath && !references.includes(archivePath)) references.push(archivePath);
    }
    bundleTemplates.push({
      sourceId: String(template.id || ''),
      title: template.title || '',
      content: template.content || '',
      references,
      effectImage: await collectImage(template.effectImagePath),
    });
  }
  validateBundleTemplates(bundleTemplates);

  const manifest = {
    format: BUNDLE_FORMAT,
    version: BUNDLE_VERSION,
    exportedAt: new Date().toISOString(),
    templates: bundleTemplates,
  };
  zip.file(BUNDLE_MANIFEST_NAME, JSON.stringify(manifest, null, 2));
  zip.file(BUNDLE_MARKDOWN_NAME, buildTemplatesMarkdown(bundleTemplates, manifest.exportedAt));
  for (const [archivePath, bytes] of uniqueImages) {
    zip.file(archivePath, bytes);
  }

  const zipBlob = await zip.generateAsync({ type: 'blob', compression: 'DEFLATE' });
  const url = URL.createObjectURL(zipBlob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = 'ImageForge-templates.zip';
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  URL.revokeObjectURL(url);
  return destination || 'ImageForge-templates.zip';
}

async function readBundleEntry(zip, name, maxBytes) {
  const entry = zip.file(name);
  if (!entry) throw new Error(`模板包缺少文件：${name}`);
  const bytes = new Uint8Array(await entry.async('arraybuffer'));
  if (bytes.length > maxBytes) throw new Error(`模板包文件超过大小限制：${name}`);
  return bytes;
}

// 旧版 Markdown ZIP（无 manifest）与桌面版 parse_legacy_markdown 同一解析规则
function parseLegacyMarkdown(markdown) {
  const marker = '\n---\n\n## 模板 ';
  const referenceMarker = '\n\n### 参考图\n\n';
  const templates = [];
  for (const section of markdown.split(marker).slice(1)) {
    const splitIndex = section.indexOf('\n\n');
    if (splitIndex < 0) throw new Error('旧版模板 Markdown 结构无效');
    const sourceId = section.slice(0, splitIndex).trim();
    const body = section.slice(splitIndex + 2);
    const referenceIndex = body.lastIndexOf(referenceMarker);
    const content = referenceIndex < 0 ? body : body.slice(0, referenceIndex);
    const referenceText =
      referenceIndex < 0 ? '' : body.slice(referenceIndex + referenceMarker.length);
    const references = referenceText
      .split('\n')
      .map((line) => {
        const trimmed = line.trim();
        const start = trimmed.lastIndexOf('](');
        if (start < 0 || !trimmed.endsWith(')')) return '';
        return trimmed.slice(start + 2, -1);
      })
      .filter(Boolean);
    templates.push({ sourceId, title: '', content: content.trim(), references, effectImage: '' });
  }
  return templates;
}

// 旧版 Web 导出包没有 format 标识，参考图按原路径名在 images/ 里尽力找回
function mapLegacyWebImages(zip, bundleTemplates, rawTemplates) {
  const resolveImage = (path) => {
    if (!path) return '';
    const base = String(path).split(/[\\/]/).pop() || '';
    return base && zip.file(`images/${base}`) ? `images/${base}` : '';
  };
  for (let index = 0; index < bundleTemplates.length; index += 1) {
    const raw = rawTemplates[index] || {};
    bundleTemplates[index].references = [
      ...new Set((raw.referencePaths || []).map(resolveImage).filter(Boolean)),
    ];
    bundleTemplates[index].effectImage = resolveImage(raw.effectImagePath);
  }
}

// 图片按内容 SHA-256 转存到共享参考图资源库（与桌面版 persist_reference_bytes 同一寻址规则），
// 带 manifest 的包会校验文件名哈希与内容一致
async function persistBundleImages(zip, templates, requireHashNames) {
  const wanted = new Set();
  for (const template of templates) {
    for (const path of [...template.references, template.effectImage]) {
      if (path) wanted.add(path);
    }
  }
  const persisted = new Map();
  let totalBytes = 0;
  for (const archivePath of wanted) {
    const bytes = await readBundleEntry(zip, archivePath, BUNDLE_MAX_IMAGE_BYTES);
    totalBytes += bytes.length;
    if (totalBytes > BUNDLE_MAX_READ_BYTES) throw new Error('模板包解压后超过 1 GB，无法导入');
    const mime = detectImageMime(archivePath, bytes);
    const hash = await sha256(bytes);
    if (requireHashNames) {
      const stem = archivePath.slice('images/'.length).split('.')[0] || '';
      if (stem !== hash) throw new Error(`参考图完整性校验失败：${archivePath}`);
    }
    const fileName = `${hash}.${extensionForMime(mime)}`;
    try {
      persisted.set(
        archivePath,
        await uploadImage(fileName, new Blob([bytes], { type: mime }), 'references')
      );
    } catch (error) {
      throw new Error(`保存模板参考图失败（${archivePath}）: ${error?.message || error}`, {
        cause: error,
      });
    }
  }
  for (const template of templates) {
    template.referencePaths = template.references.map((path) => {
      const stored = persisted.get(path);
      if (!stored) throw new Error(`模板包缺少参考图：${path}`);
      return stored;
    });
    if (template.effectImage) {
      const stored = persisted.get(template.effectImage);
      if (!stored) throw new Error(`模板包缺少效果图：${template.effectImage}`);
      template.effectImagePath = stored;
    } else {
      template.effectImagePath = '';
    }
  }
}

// 与桌面 merge_imported_templates 相同：按 (标题, 内容, 参考图集合, 效果图) 签名去重，
// 重复模板跳过，新模板重新分配本地 ID 和时间戳
function templateSignature(template) {
  const references = [...new Set(template.referencePaths)].sort();
  return [
    template.title.trim(),
    template.content.trim(),
    JSON.stringify(references),
    template.effectImagePath.trim(),
  ].join('\n');
}

function mergeImportedTemplates(bundleTemplates) {
  const existing = readJSON(KEYS.templates, []);
  const signatures = new Set(existing.map(templateSignature));
  let importedCount = 0;
  let skippedCount = 0;
  const now = new Date().toISOString();
  for (const template of bundleTemplates) {
    const next = {
      id: '',
      title: (template.title || '').trim(),
      shortTitle: '',
      category: '常用',
      content: (template.content || '').trim(),
      referencePaths: template.referencePaths,
      effectImagePath: (template.effectImagePath || '').trim(),
      notes: '',
      tags: [],
      favorite: false,
      modelHint: '',
      createdAt: now,
      updatedAt: now,
    };
    const signature = templateSignature(next);
    if (signatures.has(signature)) {
      skippedCount += 1;
      continue;
    }
    signatures.add(signature);
    importedCount += 1;
    next.id = `tpl-${Date.now()}-${importedCount}`;
    existing.push(next);
  }
  writeJSON(KEYS.templates, existing);
  return { templates: existing, importedCount, skippedCount };
}

// 导入模板包：解析与桌面版同构的 manifest（兼容旧版 Markdown 包和旧版 Web 导出包）
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
  if (blob.size > BUNDLE_MAX_ARCHIVE_BYTES) throw new Error('模板包超过 256 MB，无法导入');

  const JSZip = (await import('jszip')).default;
  const zip = await JSZip.loadAsync(blob);

  let entryCount = 0;
  zip.forEach((relativePath, entry) => {
    if (!entry.dir) entryCount += 1;
  });
  if (entryCount > BUNDLE_MAX_ENTRY_COUNT) throw new Error('模板包文件数量过多');

  let bundleTemplates;
  let requireHashNames;
  if (zip.file(BUNDLE_MANIFEST_NAME)) {
    const manifestBytes = await readBundleEntry(
      zip,
      BUNDLE_MANIFEST_NAME,
      BUNDLE_MAX_MANIFEST_BYTES
    );
    let manifest;
    try {
      manifest = JSON.parse(new TextDecoder().decode(manifestBytes));
    } catch {
      throw new Error('模板包 manifest.json 无效');
    }
    if (manifest && manifest.format === BUNDLE_FORMAT) {
      if (manifest.version !== BUNDLE_VERSION) {
        throw new Error(`不支持模板包版本 ${manifest.version}，当前支持版本 ${BUNDLE_VERSION}`);
      }
      bundleTemplates = manifest.templates || [];
      requireHashNames = true;
    } else if (manifest && Array.isArray(manifest.templates)) {
      bundleTemplates = manifest.templates.map((tpl) => ({
        sourceId: String(tpl.id || ''),
        title: tpl.title || '',
        content: tpl.content || tpl.prompt || '',
        references: [],
        effectImage: '',
      }));
      mapLegacyWebImages(zip, bundleTemplates, manifest.templates);
      requireHashNames = false;
    } else {
      throw new Error('不是 Image Forge 模板包');
    }
  } else if (zip.file(BUNDLE_MARKDOWN_NAME)) {
    const markdownBytes = await readBundleEntry(
      zip,
      BUNDLE_MARKDOWN_NAME,
      BUNDLE_MAX_MARKDOWN_BYTES
    );
    bundleTemplates = parseLegacyMarkdown(new TextDecoder().decode(markdownBytes));
    requireHashNames = false;
  } else {
    throw new Error('模板包缺少 manifest.json 或 ImageForge-templates.md');
  }

  validateBundleTemplates(bundleTemplates);
  await persistBundleImages(zip, bundleTemplates, requireHashNames);
  return mergeImportedTemplates(bundleTemplates);
}

export async function exportDataBundle(categories) {
  const JSZip = (await import('jszip')).default;
  const zip = new JSZip();
  const now = new Date().toISOString();

  const settings = categories.includes('settings') ? readJSON(KEYS.settings, null) : null;
  const templates = categories.includes('templates') ? readJSON(KEYS.templates, []) : [];
  const sessions = categories.includes('sessions') ? readJSON(KEYS.agentSessions, []) : [];
  let tasks = [];
  if (categories.includes('tasks')) {
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
      for (const att of msg.attachments || []) {
        if (att.path) fileSet.add(att.path);
      }
      if (msg.taskGroup) {
        for (const task of msg.taskGroup.tasks || []) {
          for (const p of task.referencePaths || []) fileSet.add(p);
          for (const o of task.outputs || []) {
            if (o.path) fileSet.add(o.path);
          }
        }
      }
    }
  }
  for (const t of tasks) {
    for (const p of t.referencePaths || []) fileSet.add(p);
    for (const o of t.outputs || []) {
      if (o.path) fileSet.add(o.path);
    }
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
      const ext = (path.split('.').pop() || 'png').split('?')[0];
      const name = `files/${hash.slice(0, 16)}.${ext}`;
      if (added.has(hash)) continue;
      added.add(hash);
      zip.file(name, blob);
    } catch {
      /* 文件不可访问，跳过 */
    }
  }

  // 写入 manifest
  const manifest = {
    format: 'image-forge-data-bundle',
    version: 1,
    exportedAt: now,
    hasSettings: !!settings,
    settings,
    templates,
    sessions,
    tasks,
  };
  zip.file('manifest.json', JSON.stringify(manifest, null, 2));

  // 生成 ZIP 并触发下载
  const zipBlob = await zip.generateAsync({ type: 'blob', compression: 'DEFLATE' });
  const url = URL.createObjectURL(zipBlob);
  const a = document.createElement('a');
  a.href = url;
  const date = new Date().toISOString().slice(0, 16).replace('T', '-').replace(/:/g, '');
  a.download = `export-${date}.zip`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return `export-${date}.zip`;
}

async function sha256(buffer) {
  const hash = await crypto.subtle.digest('SHA-256', buffer);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

export async function importDataBundle(file) {
  // Web 版接受 File 对象（来自文件选择器）
  const JSZip = (await import('jszip')).default;
  const arrayBuffer = await file.arrayBuffer();
  const zip = await JSZip.loadAsync(arrayBuffer);

  const manifestFile = zip.file('manifest.json');
  if (!manifestFile) throw new Error('ZIP 缺少 manifest.json');
  const manifest = JSON.parse(await manifestFile.async('text'));
  if (manifest.format !== 'image-forge-data-bundle') throw new Error('不支持的格式');

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
      if (!ids.has(tpl.id)) {
        existing.push(tpl);
        ids.add(tpl.id);
      }
    }
    writeJSON(KEYS.templates, existing);
    result.templates = manifest.templates.length;
  }

  // 导入会话
  if (manifest.sessions?.length) {
    const existing = readJSON(KEYS.agentSessions, []);
    const ids = new Set(existing.map((s) => s.id));
    for (const s of manifest.sessions) {
      if (!ids.has(s.id)) {
        existing.push(s);
        ids.add(s.id);
      }
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
    return '';
  }
}

export async function listProviderModels(provider) {
  const baseUrl = (provider.baseUrl || '').replace(/\/+$/, '');
  const apiKey = provider.apiKey || '';
  if (!baseUrl || !apiKey) throw new Error('缺少 API 地址或 Key');

  const res = await fetch(`${baseUrl}/models`, {
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
    },
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`获取模型列表失败: ${res.status} ${text}`);
  }
  const data = await res.json();
  return (data.data || data.models || data || [])
    .map((m) => m.id || m.name || m)
    .filter((id) => typeof id === 'string');
}
