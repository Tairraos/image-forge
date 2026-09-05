// Web 版 IndexedDB 数据库层，对应桌面版的 SQLite history_db.rs。
// 使用 Dexie.js 封装，兼容 IndexedDB 的异步查询。
import Dexie from 'dexie';

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

// ── 任务 CRUD ──

/** 插入或更新一条任务记录 */
export async function upsertTask(record) {
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
  const row = await db.tasks.get(id);
  return row ? parseRecord(row.record_json) : null;
}

/** 删除任务 */
export async function deleteTask(id) {
  await db.tasks.delete(id);
}

/** 获取所有任务（按时间倒序） */
export async function getAllTasks(limit) {
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
 * 本地开发时桌面版任务由 adapter-web 合并进来，见 agentLibrary。
 */
export async function queryAgentLibrary(month, query) {
  return buildLibraryPage(await getCompletedLibraryRecords(), month, query);
}

// ── 批量操作 ──

/** 批量替换全部任务 */
export async function replaceAllTasks(records) {
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
