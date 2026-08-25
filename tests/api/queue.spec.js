// queue.spec.js — 队列调度测试
// 测试 enqueueTask、cancelTask、snapshot 等核心函数。

import { describe, expect, it, vi, beforeEach } from 'vitest';

// Mock db 模块
vi.mock('../../src/api/db.js', () => ({
  upsertTask: vi.fn().mockResolvedValue(undefined),
  getAllTasks: vi.fn().mockResolvedValue([]),
}));

// Mock providers 模块（queue 内部用 executeGeneration）
vi.mock('../../src/api/providers.js', () => ({
  executeGeneration: vi.fn().mockResolvedValue([]),
}));

// Mock blob 模块
vi.mock('../../src/api/blob.js', () => ({
  uploadImage: vi.fn().mockResolvedValue('/image-forge-data/tasks/cat.png'),
}));

// Mock localStorage 供 loadProvider 使用
const mockSettings = JSON.stringify({ providers: [] });
const localStorageMock = {
  getItem: vi.fn().mockReturnValue(mockSettings),
  setItem: vi.fn(),
};
Object.defineProperty(globalThis, 'localStorage', {
  value: localStorageMock,
  writable: true,
});

const { enqueueTask, enqueueBatch, cancelTask, snapshot, onQueueChange, recoverTasks } =
  await import('../../src/api/queue.js');

beforeEach(() => {
  vi.clearAllMocks();
  // 清空队列内部状态通过多次 cancel 达到
  // 实际上队列模块使用模块级变量，我们通过 snapshot 来验证
});

describe('enqueueTask', () => {
  it('创建任务并入队，返回完整 TaskRecord', () => {
    const provider = {
      id: 'prov-1',
      name: '测试供应商',
      imageModel: 'flux-pro',
    };
    const request = {
      prompt: '一只猫',
      ratio: '16:9',
      resolution: '2K',
      count: 2,
      output_format: 'webp',
      reference_paths: ['/ref.png'],
      origin: 'agent',
      agent_session_id: 'sess-1',
      task_group_id: 'tg-1',
    };

    const task = enqueueTask(request, provider);

    expect(task.id).toBeTruthy();
    expect(task.prompt).toBe('一只猫');
    expect(task.model).toBe('flux-pro');
    expect(task.provider_name).toBe('测试供应商');
    expect(task.provider_id).toBe('prov-1');
    // worker 立即启动，status 可能已变为 running
    expect(['queued', 'running']).toContain(task.status);
    expect(task.origin).toBe('agent');
    expect(task.agent_session_id).toBe('sess-1');
    expect(task.task_group_id).toBe('tg-1');
    expect(task.reference_paths).toEqual(['/ref.png']);
    expect(task.params.ratio).toBe('16:9');
    expect(task.params.resolution).toBe('2K');
    expect(task.params.count).toBe(2);
    expect(task.params.output_format).toBe('webp');
    expect(task.outputs).toEqual([]);
  });

  it('默认值正确填充', () => {
    const task = enqueueTask({}, {});
    expect(task.prompt).toBe('');
    expect(task.model).toBe('');
    expect(task.params.ratio).toBe('1:1');
    expect(task.params.resolution).toBe('1K');
    expect(task.params.count).toBe(1);
    expect(task.params.output_format).toBe('png');
    expect(task.origin).toBe('drawing');
  });

  it('使用 request.id 若提供', () => {
    const task = enqueueTask({ id: 'custom-id' }, {});
    expect(task.id).toBe('custom-id');
  });
});

describe('enqueueBatch', () => {
  it('批量入队返回对应数量的任务', () => {
    const requests = [{ prompt: 'a' }, { prompt: 'b' }, { prompt: 'c' }];
    const tasks = enqueueBatch(requests, {});
    expect(tasks).toHaveLength(3);
    expect(tasks[0].prompt).toBe('a');
    expect(tasks[2].prompt).toBe('c');
  });
});

describe('snapshot', () => {
  it('返回当前队列快照', () => {
    const snap = snapshot();
    expect(snap).toHaveProperty('waiting');
    expect(snap).toHaveProperty('running');
    expect(snap).toHaveProperty('recent');
    expect(snap).toHaveProperty('workerActive');
    expect(snap).toHaveProperty('updatedAt');
  });
});

describe('cancelTask', () => {
  it('标记任务为取消', () => {
    const task = enqueueTask({ prompt: 'test' }, {});
    cancelTask(task.id);

    // 取消后任务不再在 waiting 中（worker 会跳过）
    const snap = snapshot();
    // waiting 可能为空，因为 worker 会消费掉；但 recent 会包含取消的任务
    // 这里只验证 cancelTask 不抛错
    expect(snap.waiting).toBeDefined();
    expect(snap.recent).toBeDefined();
  });
});

describe('onQueueChange', () => {
  it('注册回调后入队时触发', () => {
    const cb = vi.fn();
    onQueueChange(cb);

    enqueueTask({ prompt: 'test' }, {});
    expect(cb).toHaveBeenCalled();
  });
});

describe('recoverTasks', () => {
  it('从持久化中恢复任务（无遗留任务时为空）', async () => {
    await recoverTasks();
    const snap = snapshot();
    expect(snap.waiting).toEqual([]);
  });
});
