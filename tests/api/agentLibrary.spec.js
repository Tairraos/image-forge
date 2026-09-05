// agentLibrary.spec.js — Web 版图片库在本地开发下合并桌面版 ~/.image-forge 数据
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { isLocalDevMock, fetchMock } = vi.hoisted(() => ({
  isLocalDevMock: vi.fn(),
  fetchMock: vi.fn(),
}));

vi.mock('dexie', () => ({
  default: class {
    version() {
      return { stores: () => {} };
    }
  },
}));

vi.mock('../../src/api/blob.js', () => ({
  uploadImage: vi.fn(),
  isLocalDev: isLocalDevMock,
}));

const mockTasks = {
  filter: vi.fn(),
};

const localStorageMock = (() => {
  let store = {};
  return {
    getItem: vi.fn((key) => (key in store ? store[key] : null)),
    setItem: vi.fn((key, value) => {
      store[key] = String(value);
    }),
    removeItem: vi.fn((key) => {
      delete store[key];
    }),
    clear: vi.fn(() => {
      store = {};
    }),
  };
})();
Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock, writable: true });

const { db } = await vi.importActual('../../src/api/db.js');
Object.assign(db, { tasks: mockTasks });

const { agentLibrary } = await import('../../src/api/adapter-web.js');

// 桌面版记录（record_json 解析后，camelCase；路径已被 dev server 改写为 /image-forge-data）
const desktopTask = {
  id: 'dt-1',
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
  outputs: [{ path: '/image-forge-data/outputs/2026/07/a-1.png', fileName: 'a-1.png' }],
};

const webTask = {
  id: 'web-1',
  created_at: '2026-08-01T08:55:00.000Z',
  completed_at: '2026-08-01T09:00:00.000Z',
  status: 'completed',
  prompt: 'web 猫',
  model: 'flux-pro',
  origin: 'drawing',
  task_group_id: '',
  outputs: [{ path: 'https://blob.vercel-storage.com/x.png', file_name: 'x.png' }],
};

function completedRows(records) {
  return {
    toArray: vi
      .fn()
      .mockResolvedValue(
        records.map((record) => ({ status: 'completed', record_json: JSON.stringify(record) }))
      ),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorageMock.clear();
  vi.stubGlobal('fetch', fetchMock);
});

describe('agentLibrary（本地开发合并桌面版数据）', () => {
  it('dev 模式合并桌面任务，同一任务以桌面版记录为准', async () => {
    isLocalDevMock.mockReturnValue(true);
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ tasks: [desktopTask] }),
    });
    mockTasks.filter.mockReturnValue(
      completedRows([webTask, { ...desktopTask, prompt: '旧同步副本' }])
    );

    const result = await agentLibrary('', '');

    expect(fetchMock).toHaveBeenCalledWith('/image-forge-data/__library');
    expect(result.tasks.map((task) => task.id)).toEqual(['web-1', 'dt-1']);
    const desktop = result.tasks.find((task) => task.id === 'dt-1');
    expect(desktop.prompt).toBe('桌面柴犬');
    expect(result.months.map((month) => month.date)).toEqual(['2026-08', '2026-07']);
  });

  it('dev 模式按月筛选合并结果', async () => {
    isLocalDevMock.mockReturnValue(true);
    fetchMock.mockResolvedValue({ ok: true, json: async () => ({ tasks: [desktopTask] }) });
    mockTasks.filter.mockReturnValue(completedRows([webTask]));

    const result = await agentLibrary('2026-07', '');

    expect(result.tasks.map((task) => task.id)).toEqual(['dt-1']);
    // 月份统计始终是全局的，保证月份导航可用
    expect(result.months.map((month) => month.date)).toEqual(['2026-08', '2026-07']);
  });

  it('非 dev 模式只读浏览器内任务，不发请求', async () => {
    isLocalDevMock.mockReturnValue(false);
    mockTasks.filter.mockReturnValue(completedRows([webTask]));

    const result = await agentLibrary('', '');

    expect(fetchMock).not.toHaveBeenCalled();
    expect(result.tasks.map((task) => task.id)).toEqual(['web-1']);
  });

  it('dev 端点失败时回退浏览器内任务', async () => {
    isLocalDevMock.mockReturnValue(true);
    fetchMock.mockResolvedValue({ ok: false, status: 500 });
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    mockTasks.filter.mockReturnValue(completedRows([webTask]));

    const result = await agentLibrary('', '');

    expect(result.tasks.map((task) => task.id)).toEqual(['web-1']);
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });
});
