// agentLibrary.spec.js — Web 版图片库：本地开发直接读共享 SQLite（桌面任务天然可见）
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

// 共享 SQLite 里的桌面版记录（camelCase；输出为绝对磁盘路径）
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
  referencePaths: ['/Users/xiaole/.image-forge/references/abc.png'],
  outputs: [{ path: '/Users/xiaole/.image-forge/outputs/2026/07/a-1.png', fileName: 'a-1.png' }],
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

describe('agentLibrary（共享 SQLite 数据源）', () => {
  it('dev 模式直接读共享任务端点，桌面与 Web 任务统一返回', async () => {
    isLocalDevMock.mockReturnValue(true);
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ tasks: [webTask, desktopTask] }),
    });

    const result = await agentLibrary('', '');

    expect(fetchMock).toHaveBeenCalledWith('/image-forge-data/__tasks');
    expect(result.tasks.map((task) => task.id)).toEqual(['web-1', 'dt-1']);
    const desktop = result.tasks.find((task) => task.id === 'dt-1');
    expect(desktop.prompt).toBe('桌面柴犬');
    expect(result.months.map((month) => month.date)).toEqual(['2026-08', '2026-07']);
  });

  it('dev 模式按月筛选', async () => {
    isLocalDevMock.mockReturnValue(true);
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ tasks: [webTask, desktopTask] }),
    });

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

  it('dev 端点失败时直接抛错（dev server 即应用服务器，不做静默降级）', async () => {
    isLocalDevMock.mockReturnValue(true);
    fetchMock.mockResolvedValue({ ok: false, status: 500, text: async () => 'server error' });

    await expect(agentLibrary('', '')).rejects.toThrow('HTTP 500');
  });
});
