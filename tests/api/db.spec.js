// db.spec.js — IndexedDB 适配器层测试
// 通过 mock Dexie 验证 CRUD 和图片库查询逻辑，不依赖真实 IndexedDB。

import { describe, expect, it, vi, beforeEach } from 'vitest';

// 在导入 db.js 前 mock Dexie
const mockTasks = {
  put: vi.fn().mockResolvedValue(undefined),
  get: vi.fn().mockResolvedValue(null),
  delete: vi.fn().mockResolvedValue(undefined),
  clear: vi.fn().mockResolvedValue(undefined),
  orderBy: vi.fn(),
  filter: vi.fn(),
  where: vi.fn(),
};

const mockDb = {
  tasks: mockTasks,
  transaction: vi.fn((_mode, _table, fn) => fn()),
};

vi.mock('dexie', () => ({
  default: class {
    version() {
      return { stores: () => {} };
    }
  },
}));

// 必须在导入 db.js 前注入 mock 实例，因为 db.js 在模块顶层 new 了 Dexie
const { db } = await vi.importActual('../../src/api/db.js');
Object.assign(db, mockDb);

const {
  upsertTask,
  getTask,
  deleteTask,
  getAllTasks,
  queryAgentLibrary,
  buildLibraryPage,
  replaceAllTasks,
} = await import('../../src/api/db.js');

function makeRecord(overrides = {}) {
  return {
    id: 'task-1',
    created_at: '2026-08-20T10:00:00.000Z',
    completed_at: '2026-08-20T10:05:00.000Z',
    status: 'completed',
    prompt: '一只猫',
    model: 'flux-pro',
    origin: 'drawing',
    task_group_id: '',
    outputs: [{ path: 'https://blob.vercel-storage.com/cat.png', file_name: 'cat.png' }],
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('upsertTask', () => {
  it('把记录写入 tasks 表，library_date 从 completed_at 提取', async () => {
    const record = makeRecord();
    await upsertTask(record);

    expect(mockTasks.put).toHaveBeenCalledTimes(1);
    const row = mockTasks.put.mock.calls[0][0];
    expect(row.id).toBe('task-1');
    expect(row.library_date).toBe('2026-08-20');
    expect(row.status).toBe('completed');
    expect(row.origin).toBe('drawing');
    expect(JSON.parse(row.record_json).prompt).toBe('一只猫');
  });

  it('无 completed_at 时 library_date 回退 created_at', async () => {
    const record = makeRecord({ completed_at: '' });
    await upsertTask(record);

    const row = mockTasks.put.mock.calls[0][0];
    expect(row.library_date).toBe('2026-08-20');
  });

  it('无日期时 library_date 为 1970-01-01', async () => {
    const record = makeRecord({ completed_at: '', created_at: '' });
    await upsertTask(record);

    const row = mockTasks.put.mock.calls[0][0];
    expect(row.library_date).toBe('1970-01-01');
  });

  it('agent 会话的任务 origin 标记为 agent', async () => {
    const record = makeRecord({ agent_session_id: 'sess-1', origin: '' });
    await upsertTask(record);

    const row = mockTasks.put.mock.calls[0][0];
    expect(row.origin).toBe('agent');
  });

  it('task_group_id 存在的任务 origin 标记为 agent', async () => {
    const record = makeRecord({ task_group_id: 'tg-1', origin: '' });
    await upsertTask(record);

    const row = mockTasks.put.mock.calls[0][0];
    expect(row.origin).toBe('agent');
  });
});

describe('getTask', () => {
  it('按 ID 返回解析后的记录', async () => {
    const record = makeRecord();
    mockTasks.get.mockResolvedValue({
      id: 'task-1',
      record_json: JSON.stringify(record),
    });

    const result = await getTask('task-1');
    expect(result.id).toBe('task-1');
    expect(result.prompt).toBe('一只猫');
  });

  it('不存在时返回 null', async () => {
    mockTasks.get.mockResolvedValue(undefined);
    const result = await getTask('nonexistent');
    expect(result).toBeNull();
  });
});

describe('deleteTask', () => {
  it('调用 db.tasks.delete', async () => {
    await deleteTask('task-1');
    expect(mockTasks.delete).toHaveBeenCalledWith('task-1');
  });
});

describe('getAllTasks', () => {
  it('按 created_at 倒序返回所有任务', async () => {
    const records = [makeRecord({ id: 'a' }), makeRecord({ id: 'b' })];
    const chain = {
      reverse: vi.fn().mockReturnThis(),
      limit: vi.fn().mockReturnThis(),
      toArray: vi
        .fn()
        .mockResolvedValue(records.map((r) => ({ id: r.id, record_json: JSON.stringify(r) }))),
    };
    mockTasks.orderBy.mockReturnValue(chain);

    const result = await getAllTasks();
    expect(result).toHaveLength(2);
    expect(result[0].id).toBe('a');
  });

  it('limit 参数生效', async () => {
    const chain = {
      reverse: vi.fn().mockReturnThis(),
      limit: vi.fn().mockReturnThis(),
      toArray: vi.fn().mockResolvedValue([]),
    };
    mockTasks.orderBy.mockReturnValue(chain);

    await getAllTasks(10);
    expect(chain.limit).toHaveBeenCalledWith(10);
  });
});

describe('queryAgentLibrary', () => {
  const completedTask = (id, recordJson) => ({
    id,
    status: 'completed',
    created_at: '2026-08-20T10:00:00.000Z',
    library_date: '2026-08-20',
    record_json: typeof recordJson === 'string' ? recordJson : JSON.stringify(recordJson),
  });

  it('按月筛选：仅返回有 outputs 的 completed 任务', async () => {
    const chain = {
      startsWith: vi.fn().mockReturnThis(),
      filter: vi.fn().mockReturnThis(),
      toArray: vi.fn(),
    };
    mockTasks.where.mockReturnValue(chain);

    // 第一次调用（按月筛选）返回一条
    chain.toArray.mockResolvedValueOnce([completedTask('task-1', makeRecord({ id: 'task-1' }))]);

    // 第二次调用（月份统计）返回同一条
    const filterChain = {
      toArray: vi.fn().mockResolvedValue([completedTask('task-1', makeRecord({ id: 'task-1' }))]),
    };
    mockTasks.filter.mockReturnValue(filterChain);

    const result = await queryAgentLibrary('2026-08', '');

    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].id).toBe('task-1');
    expect(result.months).toHaveLength(1);
    expect(result.months[0].date).toBe('2026-08');
    expect(result.months[0].imageCount).toBe(1);
    expect(result.total_images).toBe(1);
  });

  it('关键词搜索：跨月匹配 prompt / model / id', async () => {
    const allTasks = [
      completedTask('task-1', makeRecord({ id: 'task-1', prompt: '一只猫', outputs: [] })),
      completedTask(
        'task-2',
        makeRecord({ id: 'task-2', prompt: '一只狗', outputs: [{ path: '/x.png' }] })
      ),
      completedTask(
        'task-3',
        makeRecord({ id: 'task-3', prompt: '猫在睡觉', outputs: [{ path: '/y.png' }] })
      ),
    ];
    const filterChain = { toArray: vi.fn().mockResolvedValue(allTasks) };
    mockTasks.filter.mockReturnValue(filterChain);

    const result = await queryAgentLibrary('', '猫');

    // task-1 无 outputs，task-2 的 prompt/model/id 都不含"猫"，
    // 只有 task-3 的 prompt 含"猫"且有 outputs
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].id).toBe('task-3');
  });

  it('无匹配时返回空列表', async () => {
    const chain = {
      startsWith: vi.fn().mockReturnThis(),
      filter: vi.fn().mockReturnThis(),
      toArray: vi.fn().mockResolvedValue([]),
    };
    mockTasks.where.mockReturnValue(chain);

    const filterChain = { toArray: vi.fn().mockResolvedValue([]) };
    mockTasks.filter.mockReturnValue(filterChain);

    const result = await queryAgentLibrary('2026-08', '');
    expect(result.tasks).toHaveLength(0);
    expect(result.months).toHaveLength(0);
    expect(result.total_images).toBe(0);
  });
});

describe('replaceAllTasks', () => {
  it('清空后批量写入', async () => {
    const records = [makeRecord({ id: 'a' }), makeRecord({ id: 'b' })];
    await replaceAllTasks(records);

    expect(mockTasks.clear).toHaveBeenCalled();
    expect(mockTasks.put).toHaveBeenCalledTimes(2);
  });
});

describe('buildLibraryPage（桌面/Web 记录混用）', () => {
  // 桌面版 record_json：camelCase 任务字段 + camelCase outputs
  const desktopRecord = {
    id: 'desktop-1',
    createdAt: '2026-07-10T08:00:00Z',
    completedAt: '2026-07-10T08:05:00Z',
    status: 'completed',
    prompt: '桌面柴犬',
    model: 'gpt-image-2',
    providerName: 'OpenAI',
    origin: 'agent',
    agentSessionId: 'sess-1',
    taskGroupId: 'tg-1',
    referencePaths: ['/image-forge-data/references/abc.png'],
    outputs: [
      { path: '/image-forge-data/outputs/2026/07/a.png', fileName: 'a.png', size: '1024x1024' },
    ],
  };
  const webRecord = makeRecord({
    id: 'web-1',
    prompt: '一只狗',
    created_at: '2026-08-01T08:55:00.000Z',
    completed_at: '2026-08-01T09:00:00.000Z',
  });

  it('月份为空时返回全部月份（与桌面版 agent_library 一致）', () => {
    const page = buildLibraryPage([desktopRecord, webRecord], '', '');
    expect(page.tasks.map((task) => task.id)).toEqual(['web-1', 'desktop-1']);
    expect(page.months.map((month) => month.date)).toEqual(['2026-08', '2026-07']);
    expect(page.total_images).toBe(2);
  });

  it('按月筛选同时兼容两种字段形状', () => {
    expect(
      buildLibraryPage([desktopRecord, webRecord], '2026-07', '').tasks.map((t) => t.id)
    ).toEqual(['desktop-1']);
    expect(
      buildLibraryPage([desktopRecord, webRecord], '2026-08', '').tasks.map((t) => t.id)
    ).toEqual(['web-1']);
  });

  it('搜索兼容 providerName（camelCase）与 prompt', () => {
    expect(
      buildLibraryPage([desktopRecord, webRecord], '', 'openai').tasks.map((task) => task.id)
    ).toEqual(['desktop-1']);
    expect(
      buildLibraryPage([desktopRecord, webRecord], '', '狗').tasks.map((task) => task.id)
    ).toEqual(['web-1']);
  });

  it('无日期记录不进入月份统计', () => {
    const page = buildLibraryPage(
      [{ ...webRecord, id: 'no-date', completed_at: '', created_at: '' }],
      '',
      ''
    );
    expect(page.tasks).toHaveLength(1);
    expect(page.months).toEqual([]);
  });
});
