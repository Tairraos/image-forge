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

/**
 * Agent 内嵌图片库查询。
 * 无关键词时按月份筛选，有关键词时跨月搜索。
 * 同时返回所有有图片的月份列表。
 */
export async function queryAgentLibrary(month, query) {
  const q = (query || '').trim();
  const now = new Date();
  const defaultMonth = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
  const targetMonth = month || defaultMonth;

  let tasks;
  if (q) {
    // 关键词搜索：匹配 prompt / model / id
    const lower = q.toLowerCase();
    const all = await db.tasks.filter((row) => row.status === 'completed').toArray();
    tasks = all.filter((row) => {
      const json = row.record_json || '';
      const record = parseRecord(json);
      // 有输出图才显示
      if (!record.outputs?.length) return false;
      return (
        (record.prompt || '').toLowerCase().includes(lower) ||
        (record.model || '').toLowerCase().includes(lower) ||
        (record.id || '').toLowerCase().includes(lower)
      );
    });
  } else {
    // 按月筛选
    const prefix = targetMonth;
    tasks = await db.tasks
      .where('library_date')
      .startsWith(prefix)
      .filter((row) => row.status === 'completed')
      .toArray();
    tasks = tasks.filter((row) => {
      const record = parseRecord(row.record_json);
      return record.outputs?.length > 0;
    });
  }

  // 按创建时间倒序
  tasks.sort((a, b) => (b.created_at || '').localeCompare(a.created_at || ''));

  // 月份统计
  const allCompleted = await db.tasks.filter((row) => row.status === 'completed').toArray();
  const monthMap = new Map();
  for (const row of allCompleted) {
    const record = parseRecord(row.record_json);
    if (!record.outputs?.length) continue;
    const m = (row.library_date || '').slice(0, 7);
    if (!m) continue;
    monthMap.set(m, (monthMap.get(m) || 0) + record.outputs.length);
  }
  const months = Array.from(monthMap, ([date, imageCount]) => ({ date, imageCount })).sort((a, b) =>
    b.date.localeCompare(a.date)
  );

  const totalImages = tasks.reduce((sum, row) => {
    const record = parseRecord(row.record_json);
    return sum + (record.outputs?.length || 0);
  }, 0);

  return {
    tasks: tasks.map((row) => parseRecord(row.record_json)),
    months,
    total_images: totalImages,
  };
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

function libraryDate(record) {
  const value = record.completed_at || record.created_at || '';
  // 取日期部分 YYYY-MM-DD
  const match = /^\d{4}-\d{2}-\d{2}/.exec(value);
  return match ? match[0] : '1970-01-01';
}

function normalizedOrigin(record) {
  if (record.origin === 'agent' || record.agent_session_id || record.task_group_id) {
    return 'agent';
  }
  return 'drawing';
}

export { db };
