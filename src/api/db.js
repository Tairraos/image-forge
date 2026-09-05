// Web 版任务数据层，对应桌面版的 SQLite history_db.rs。
// - 本地开发（isLocalDev）：读写 dev server 的 /image-forge-data/__tasks，
//   直接落到与桌面版共享的 ~/.image-forge/library.sqlite（record_json 为桌面端 camelCase 契约）。
// - 远端/兜底：Dexie（IndexedDB），库名 ImageForge。
import Dexie from 'dexie';
import { isLocalDev } from './blob.js';

class ImageForgeDB extends Dexie {
  constructor() {
    super('ImageForge');

    this.version(1).stores({
      // 任务主表：完整 TaskRecord 存为 record_json，索引字段拆出来供查询
      tasks: 'id, created_at, library_date, status, origin, task_group_id, *prompt, *model',
    });
  }
}

const db = new ImageForgeDB();

const DATA_ORIGIN = '/image-forge-data';

// ── 记录形状转换（HTTP 后端与桌面端 camelCase 契约对接；Web 内部统一 snake_case）──

const TASK_FIELD_MAP = {
  createdAt: 'created_at',
  updatedAt: 'updated_at',
  startedAt: 'started_at',
  completedAt: 'completed_at',
  providerId: 'provider_id',
  providerName: 'provider_name',
  referencePaths: 'reference_paths',
  taskGroupId: 'task_group_id',
  agentSessionId: 'agent_session_id',
  agentPlan: 'agent_plan',
};

const OUTPUT_FIELD_MAP = {
  fileName: 'file_name',
  mimeType: 'mime_type',
  outputFormat: 'output_format',
  revisedPrompt: 'revised_prompt',
};

function toSnakeTaskRecord(record) {
  if (!record || typeof record !== 'object') return record;
  const task = { ...record };
  for (const [camel, snake] of Object.entries(TASK_FIELD_MAP)) {
    if (camel in task && !(snake in task)) task[snake] = task[camel];
  }
  if (task.params && typeof task.params === 'object') {
    const params = { ...task.params };
    for (const key of Object.keys(params)) {
      const snake = key.replace(/[A-Z]/g, (char) => `_${char.toLowerCase()}`);
      if (snake !== key && !(snake in params)) params[snake] = params[key];
    }
    task.params = params;
  }
  if (Array.isArray(task.outputs)) {
    task.outputs = task.outputs.map((output) => {
      const item = { ...output };
      for (const [camel, snake] of Object.entries(OUTPUT_FIELD_MAP)) {
        if (camel in item && !(snake in item)) item[snake] = item[camel];
      }
      return item;
    });
  }
  return task;
}

async function requestJson(url, options) {
  const res = await fetch(url, ...(options ? [options] : []));
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`任务数据请求失败: HTTP ${res.status} ${text}`);
  }
  if (res.status === 204) return null;
  return res.json();
}

async function upsertTaskViaHttp(record) {
  await requestJson(`${DATA_ORIGIN}/__tasks`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(record),
  });
}

async function getAllTasksViaHttp(limit) {
  const data = await requestJson(`${DATA_ORIGIN}/__tasks${limit ? `?limit=${limit}` : ''}`);
  return (Array.isArray(data?.tasks) ? data.tasks : []).map(toSnakeTaskRecord);
}

async function deleteTaskViaHttp(id) {
  await requestJson(`${DATA_ORIGIN}/__tasks/${encodeURIComponent(id)}`, { method: 'DELETE' });
}

// ── 任务 CRUD ──

/** 插入或更新一条任务记录 */
export async function upsertTask(record) {
  if (isLocalDev()) {
    return upsertTaskViaHttp(record);
  }
  const now = new Date().toISOString();
  const row = {
    id: record.id,
    created_at: record.created_at || now,
    library_date: libraryDate(record),
    status: record.status || 'queued',
    origin: normalizedOrigin(record),
    task_group_id: record.task_group_id || '',
    prompt: record.prompt || '',
    model: record.model || '',
    record_json: JSON.stringify(record),
  };
  await db.tasks.put(row);
}

/** 按 ID 获取单条任务 */
export async function getTask(id) {
  if (isLocalDev()) {
    return (await getAllTasksViaHttp()).find((record) => record.id === id) || null;
  }
  const row = await db.tasks.get(id);
  return row ? parseRecord(row.record_json) : null;
}

/** 删除任务 */
export async function deleteTask(id) {
  if (isLocalDev()) {
    return deleteTaskViaHttp(id);
  }
  await db.tasks.delete(id);
}

/** 获取所有任务（按时间倒序） */
export async function getAllTasks(limit) {
  if (isLocalDev()) {
    return getAllTasksViaHttp(limit);
  }
  let collection = db.tasks.orderBy('created_at').reverse();
  if (limit) collection = collection.limit(limit);
  const rows = await collection.toArray();
  return rows.map((row) => parseRecord(row.record_json));
}

// ── 图片库查询 ──

/** 兼容桌面版（camelCase）与 Web 版（snake_case）两种记录形状的时间戳 */
function recordTime(record) {
  return record.completed_at || record.completedAt || record.created_at || record.createdAt || '';
}

function libraryDate(record) {
  // 取日期部分 YYYY-MM-DD
  const match = /^\d{4}-\d{2}-\d{2}/.exec(recordTime(record));
  return match ? match[0] : '1970-01-01';
}

/** 全部有输出图的 completed 任务（解析后的记录，兼容两种字段形状） */
export async function getCompletedLibraryRecords() {
  if (isLocalDev()) {
    return (await getAllTasksViaHttp()).filter(
      (record) => record.status === 'completed' && record.outputs?.length > 0
    );
  }
  const rows = await db.tasks.filter((row) => row.status === 'completed').toArray();
  return rows
    .map((row) => parseRecord(row.record_json))
    .filter((record) => record.outputs?.length > 0);
}

/**
 * 纯查询：对任意一组任务记录做图片库筛选与统计，桌面版/Web 版记录混用均可。
 * month 为空表示不过滤（与桌面版 agent_library 一致，默认显示全部月份）；
 * query 匹配 prompt / model / providerName / id，跨月搜索。
 */
export function buildLibraryPage(records, month, query) {
  const q = (query || '').trim().toLowerCase();
  const targetMonth = (month || '').trim();
  const withOutputs = records.filter((record) => record.outputs?.length > 0);

  let tasks;
  if (q) {
    tasks = withOutputs.filter((record) =>
      [record.prompt, record.model, record.provider_name || record.providerName, record.id].some(
        (value) =>
          String(value || '')
            .toLowerCase()
            .includes(q)
      )
    );
  } else if (targetMonth) {
    tasks = withOutputs.filter((record) => libraryDate(record).startsWith(targetMonth));
  } else {
    tasks = [...withOutputs];
  }

  tasks.sort((a, b) => String(recordTime(b)).localeCompare(String(recordTime(a))));

  const monthMap = new Map();
  for (const record of withOutputs) {
    const monthKey = libraryDate(record).slice(0, 7);
    if (!monthKey || monthKey === '1970-01') continue;
    monthMap.set(monthKey, (monthMap.get(monthKey) || 0) + record.outputs.length);
  }
  const months = Array.from(monthMap, ([date, imageCount]) => ({ date, imageCount })).sort((a, b) =>
    b.date.localeCompare(a.date)
  );

  return {
    tasks,
    months,
    total_images: tasks.reduce((sum, record) => sum + (record.outputs?.length || 0), 0),
  };
}

/**
 * Agent 内嵌图片库查询（浏览器 IndexedDB 内的任务）。
 * 本地开发时任务直接来自共享 SQLite，桌面任务天然可见。
 */
export async function queryAgentLibrary(month, query) {
  return buildLibraryPage(await getCompletedLibraryRecords(), month, query);
}

// ── 批量操作 ──

/** 批量替换全部任务。共享 SQLite 模式下只做逐条合并（绝不清空桌面任务）。 */
export async function replaceAllTasks(records) {
  if (isLocalDev()) {
    for (const record of records) {
      await upsertTaskViaHttp(record);
    }
    return;
  }
  await db.transaction('rw', db.tasks, async () => {
    await db.tasks.clear();
    for (const record of records) {
      await upsertTask(record);
    }
  });
}

// ── 辅助函数 ──

function parseRecord(json) {
  try {
    return JSON.parse(json);
  } catch {
    return { id: '', outputs: [], prompt: '' };
  }
}

function normalizedOrigin(record) {
  if (record.origin === 'agent' || record.agent_session_id || record.task_group_id) {
    return 'agent';
  }
  return 'drawing';
}

export { db };
