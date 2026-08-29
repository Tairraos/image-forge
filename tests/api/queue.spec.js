// queue.spec.js — 队列调度测试
// 测试 enqueueTask、cancelTask、retryTask、recoverTasks 与 worker 执行流程。

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

const dbMock = await import('../../src/api/db.js');
const providersMock = await import('../../src/api/providers.js');
const blobMock = await import('../../src/api/blob.js');
const {
  enqueueTask,
  enqueueBatch,
  cancelTask,
  cancelTaskGroup,
  retryTask,
  retryTaskGroup,
  snapshot,
  onQueueChange,
  recoverTasks,
} = await import('../../src/api/queue.js');

// 配好生图 provider 的设置，供 worker 的 loadProvider 读取
const enabledProviderSettings = JSON.stringify({
  providers: [{ id: 'prov-1', name: '测试供应商', imageModel: 'flux-pro' }],
});

// 等待队列排空：worker 空闲且没有等待/运行中的任务
async function waitForIdle() {
  for (let i = 0; i < 200; i += 1) {
    const snap = snapshot();
    if (!snap.workerActive && snap.waiting.length === 0 && snap.running.length === 0) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  throw new Error('队列未在限时内排空');
}

beforeEach(() => {
  vi.clearAllMocks();
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

describe('worker 执行流程', () => {
  beforeEach(async () => {
    vi.resetAllMocks();
    dbMock.upsertTask.mockResolvedValue(undefined);
    dbMock.getAllTasks.mockResolvedValue([]);
    providersMock.executeGeneration.mockResolvedValue([]);
    blobMock.uploadImage.mockResolvedValue('/image-forge-data/tasks/out-01.png');
    localStorageMock.getItem.mockReturnValue(enabledProviderSettings);
    await waitForIdle();
  });

  it('执行成功：上传图片、记录 outputs 与 usage', async () => {
    providersMock.executeGeneration.mockResolvedValue([
      {
        bytes: new Uint8Array([1, 2, 3]),
        output_format: 'png',
        size: '1024x1024',
        revised_prompt: '改良提示词',
        usage: { cost: '0.01' },
      },
    ]);

    const task = enqueueTask(
      { prompt: '画一只柴犬', count: 1 },
      { id: 'prov-1', name: '测试供应商', imageModel: 'flux-pro' }
    );
    await waitForIdle();

    expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    const [provider, request] = providersMock.executeGeneration.mock.calls[0];
    expect(provider.id).toBe('prov-1');
    expect(request.prompt).toBe('画一只柴犬');
    expect(request.ratio).toBe('1:1');

    expect(blobMock.uploadImage).toHaveBeenCalledTimes(1);
    const [fileName, blob, datePath] = blobMock.uploadImage.mock.calls[0];
    expect(fileName).toContain(task.id);
    expect(fileName.endsWith('-01.png')).toBe(true);
    expect(blob).toBeInstanceOf(Blob);
    expect(datePath).toMatch(/^tasks\/\d{4}-\d{2}-\d{2}$/);

    expect(task.status).toBe('completed');
    expect(task.completed_at).toBeTruthy();
    expect(task.outputs).toHaveLength(1);
    expect(task.outputs[0]).toMatchObject({
      path: '/image-forge-data/tasks/out-01.png',
      output_format: 'png',
      size: '1024x1024',
      revised_prompt: '改良提示词',
    });
    expect(task.usage).toEqual({ cost: '0.01' });
    expect(dbMock.upsertTask).toHaveBeenCalledWith(task);
  });

  it('执行失败：任务标记 failed 并保留错误信息', async () => {
    providersMock.executeGeneration.mockRejectedValue(new Error('配额不足'));

    const task = enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toBe('配额不足');
    expect(task.outputs).toEqual([]);
  });

  it('找不到 API 配置时任务失败', async () => {
    localStorageMock.getItem.mockReturnValue(JSON.stringify({ providers: [] }));

    const task = enqueueTask({ prompt: 'x' }, { id: 'missing' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toContain('找不到 API 配置');
  });

  it('设置 JSON 损坏时按找不到配置处理', async () => {
    localStorageMock.getItem.mockReturnValue('broken-json{');

    const task = enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toContain('找不到 API 配置');
  });

  it('localStorage 无设置时任务失败', async () => {
    localStorageMock.getItem.mockReturnValue(null);

    const task = enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toContain('找不到 API 配置');
  });

  it('取消仍在排队的任务，worker 跳过执行', async () => {
    let releaseFirst;
    providersMock.executeGeneration.mockImplementation(
      () =>
        new Promise((resolve) => {
          releaseFirst = resolve;
        })
    );

    const first = enqueueTask({ prompt: '占用 worker' }, { id: 'prov-1' });
    const queued = enqueueTask({ prompt: '待取消' }, { id: 'prov-1' });

    await vi.waitFor(() => {
      expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    });
    cancelTask(queued.id);

    const snap = snapshot();
    expect(snap.waiting).toHaveLength(0);
    expect(snap.recent.map((t) => t.id)).toContain(queued.id);
    expect(queued.status).toBe('cancelled');

    releaseFirst([]);
    await waitForIdle();
    expect(first.status).toBe('completed');
    expect(queued.status).toBe('cancelled');
  });

  it('取消任务组内所有排队任务', async () => {
    let releaseFirst;
    providersMock.executeGeneration.mockImplementation(
      () =>
        new Promise((resolve) => {
          releaseFirst = resolve;
        })
    );

    const first = enqueueTask({ prompt: '占用' }, { id: 'prov-1' });
    await vi.waitFor(() => {
      expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    });
    const group = enqueueBatch(
      [
        { prompt: 'g1', task_group_id: 'tg-1' },
        { prompt: 'g2', task_group_id: 'tg-1' },
      ],
      { id: 'prov-1' }
    );

    cancelTaskGroup('tg-1');
    releaseFirst([]);
    await waitForIdle();

    expect(first.status).toBe('completed');
    expect(group.map((t) => t.status)).toEqual(['cancelled', 'cancelled']);
  });

  it('取消运行中的任务后再重试，出队时直接标记取消', async () => {
    let rejectGeneration;
    providersMock.executeGeneration.mockImplementation(
      () =>
        new Promise((_, reject) => {
          rejectGeneration = reject;
        })
    );

    const task = enqueueTask({ prompt: 'cancel-run' }, { id: 'prov-1' });
    await vi.waitFor(() => {
      expect(snapshot().running.map((t) => t.id)).toContain(task.id);
    });

    // 运行中取消：任务不中断，但 ID 进入 cancelSet
    cancelTask(task.id);
    rejectGeneration(new Error('已取消'));
    await waitForIdle();
    expect(task.status).toBe('failed');

    // 重试后出队时命中 cancelSet → 直接取消，不再调用生成
    const retried = retryTask(task.id);
    expect(retried).not.toBeNull();
    await waitForIdle();

    expect(task.status).toBe('cancelled');
    expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
  });

  it('重试失败任务并成功完成', async () => {
    providersMock.executeGeneration
      .mockRejectedValueOnce(new Error('网络错误'))
      .mockResolvedValueOnce([]);

    const task = enqueueTask({ prompt: 'r' }, { id: 'prov-1' });
    await waitForIdle();
    expect(task.status).toBe('failed');

    const retried = retryTask(task.id);
    expect(retried).not.toBeNull();
    // worker 同步启动，返回时状态可能已是 running；error 必定被清空
    expect(retried.error).toBe('');

    await waitForIdle();
    expect(task.status).toBe('completed');
  });

  it('重试不存在的任务返回 null', () => {
    expect(retryTask('no-such-task')).toBeNull();
  });

  it('已完成或排队中的任务不可重试', async () => {
    const task = enqueueTask({ prompt: 'ok' }, { id: 'prov-1' });
    await waitForIdle();
    expect(task.status).toBe('completed');
    expect(retryTask(task.id)).toBeNull();
  });

  it('重试任务组内所有失败任务', async () => {
    providersMock.executeGeneration.mockRejectedValue(new Error('批量失败'));
    const tasks = enqueueBatch(
      [
        { prompt: 'a', task_group_id: 'tg-9' },
        { prompt: 'b', task_group_id: 'tg-9' },
        { prompt: 'c' },
      ],
      { id: 'prov-1' }
    );
    await waitForIdle();
    expect(tasks.every((t) => t.status === 'failed')).toBe(true);

    providersMock.executeGeneration.mockResolvedValue([]);
    retryTaskGroup('tg-9');
    await waitForIdle();

    const groupTasks = tasks.filter((t) => t.task_group_id === 'tg-9');
    expect(groupTasks.every((t) => t.status === 'completed')).toBe(true);
    expect(tasks[2].status).toBe('failed');
  });
});

describe('recoverTasks 恢复遗留任务', () => {
  beforeEach(async () => {
    vi.resetAllMocks();
    dbMock.upsertTask.mockResolvedValue(undefined);
    dbMock.getAllTasks.mockResolvedValue([]);
    providersMock.executeGeneration.mockResolvedValue([]);
    blobMock.uploadImage.mockResolvedValue('/image-forge-data/tasks/out-01.png');
    localStorageMock.getItem.mockReturnValue(enabledProviderSettings);
    await waitForIdle();
  });

  it('把遗留 running 任务恢复为 queued 并重新执行', async () => {
    const legacyTask = {
      id: 'legacy-1',
      status: 'running',
      prompt: '遗留任务',
      provider_id: 'prov-1',
      params: {},
      reference_paths: [],
      outputs: [],
    };
    const doneTask = { id: 'done-1', status: 'completed', prompt: '已完成', outputs: [] };
    dbMock.getAllTasks.mockResolvedValueOnce([legacyTask, doneTask]);

    await recoverTasks();

    // 恢复后任务进入队列（worker 可能已开始执行）
    const snap = snapshot();
    expect([...snap.waiting, ...snap.running].map((t) => t.id)).toContain('legacy-1');
    expect(snap.recent.map((t) => t.id)).toContain('done-1');

    await waitForIdle();
    expect(legacyTask.status).toBe('completed');
    expect(dbMock.upsertTask).toHaveBeenCalledWith(legacyTask);
  });

  it('recent 只保留最近 50 条已完成任务', async () => {
    const many = Array.from({ length: 60 }, (_, index) => ({
      id: `done-${index}`,
      status: 'completed',
      outputs: [],
    }));
    dbMock.getAllTasks.mockResolvedValueOnce(many);

    await recoverTasks();

    expect(snapshot().recent).toHaveLength(50);
  });
});
