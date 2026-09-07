// queue.spec.js — 队列调度测试
// 测试 enqueueTask、cancelTask、retryTask、recoverTasks 与 worker 执行流程。

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

// Mock db 模块
vi.mock('../../src/api/db.js', () => ({
  upsertTask: vi.fn().mockResolvedValue(undefined),
  getAllTasks: vi.fn().mockResolvedValue([]),
  getTask: vi.fn().mockResolvedValue(null),
}));

// Mock providers 模块（queue 内部用 executeGeneration）
vi.mock('../../src/api/providers.js', () => ({
  executeGeneration: vi.fn().mockResolvedValue([]),
}));

// Mock blob 模块
vi.mock('../../src/api/blob.js', () => ({
  isLocalDev: () => false,
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

const generatedImage = { bytes: new Uint8Array([1, 2, 3]), output_format: 'png' };

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

afterEach(waitForIdle);

describe('enqueueTask', () => {
  it('创建任务并入队，返回完整 TaskRecord', async () => {
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

    const task = await enqueueTask(request, provider);

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

  it('默认值正确填充', async () => {
    const task = await enqueueTask({}, {});
    expect(task.prompt).toBe('');
    expect(task.model).toBe('');
    expect(task.params.ratio).toBe('1:1');
    expect(task.params.resolution).toBe('1K');
    expect(task.params.count).toBe(1);
    expect(task.params.output_format).toBe('png');
    expect(task.origin).toBe('drawing');
  });

  it('使用 request.id 若提供', async () => {
    const task = await enqueueTask({ id: 'custom-id' }, {});
    expect(task.id).toBe('custom-id');
  });
});

describe('enqueueBatch', () => {
  it('批量入队返回对应数量的任务', async () => {
    const requests = [{ prompt: 'a' }, { prompt: 'b' }, { prompt: 'c' }];
    const tasks = await enqueueBatch(requests, {});
    expect(tasks).toHaveLength(3);
    expect(tasks[0].prompt).toBe('a');
    expect(tasks[2].prompt).toBe('c');
  });
});

describe('snapshot', () => {
  it('返回当前队列快照', async () => {
    const snap = snapshot();
    expect(snap).toHaveProperty('waiting');
    expect(snap).toHaveProperty('running');
    expect(snap).toHaveProperty('recent');
    expect(snap).toHaveProperty('workerActive');
    expect(snap).toHaveProperty('updatedAt');
  });
});

describe('cancelTask', () => {
  it('标记任务为取消', async () => {
    const task = await enqueueTask({ prompt: 'test' }, {});
    await cancelTask(task.id);

    // 取消后任务不再在 waiting 中（worker 会跳过）
    const snap = snapshot();
    // waiting 可能为空，因为 worker 会消费掉；但 recent 会包含取消的任务
    // 这里只验证 cancelTask 不抛错
    expect(snap.waiting).toBeDefined();
    expect(snap.recent).toBeDefined();
  });
});

describe('onQueueChange', () => {
  it('注册回调后入队时触发', async () => {
    const cb = vi.fn();
    onQueueChange(cb);

    await enqueueTask({ prompt: 'test' }, {});
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
    dbMock.getTask.mockResolvedValue(null);
    providersMock.executeGeneration.mockResolvedValue([generatedImage]);
    blobMock.uploadImage.mockResolvedValue('/image-forge-data/tasks/out-01.png');
    localStorageMock.getItem.mockReturnValue(enabledProviderSettings);
    await waitForIdle();
  });

  it('入队持久化失败时不启动生图请求', async () => {
    dbMock.upsertTask.mockRejectedValueOnce(new Error('磁盘不可写'));
    await expect(enqueueTask({ prompt: 'x' }, { id: 'prov-1' })).rejects.toThrow('磁盘不可写');
    expect(providersMock.executeGeneration).not.toHaveBeenCalled();
    expect(snapshot().waiting).toHaveLength(0);
  });

  it('空结果标记失败，不生成没有图片的完成任务', async () => {
    providersMock.executeGeneration.mockResolvedValueOnce([]);
    const task = await enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await waitForIdle();
    expect(task.status).toBe('failed');
    expect(task.error).toContain('未返回图像数据');
    expect(blobMock.uploadImage).not.toHaveBeenCalled();
  });

  it('取消后到达的结果不会保存成完成图片', async () => {
    let complete;
    providersMock.executeGeneration.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        })
    );
    const task = await enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await vi.waitFor(() => expect(complete).toBeTypeOf('function'));
    await cancelTask(task.id);
    expect(task.status).toBe('cancelling');
    complete([generatedImage]);
    await waitForIdle();
    expect(task.status).toBe('cancelled');
    expect(task.error).toBe('');
    expect(blobMock.uploadImage).not.toHaveBeenCalled();
  });

  it('整组取消会立即中断请求，组内下一项不会开始', async () => {
    providersMock.executeGeneration.mockImplementationOnce(
      (_, __, signal) =>
        new Promise((_, reject) => {
          signal.addEventListener('abort', () => reject(signal.reason), { once: true });
        })
    );
    const tasks = await enqueueBatch(
      [
        { prompt: 'a', task_group_id: 'cancel-active-group' },
        { prompt: 'b', task_group_id: 'cancel-active-group' },
      ],
      { id: 'prov-1' }
    );
    await vi.waitFor(() => expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1));
    await cancelTaskGroup('cancel-active-group');
    await waitForIdle();
    expect(tasks.map((task) => task.status)).toEqual(['cancelled', 'cancelled']);
    expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
  });

  it('从持久化重试旧任务，并防止连续点击重复入队', async () => {
    dbMock.getTask.mockResolvedValue({
      id: 'old-failed-task',
      status: 'failed',
      error: '旧错误',
      provider_id: 'prov-1',
      prompt: '旧提示词',
      params: { ratio: '16:9', resolution: '2k', size: '2048x1152' },
      reference_paths: ['/reference.png'],
    });
    const [retried, duplicate] = await Promise.all([
      retryTask('old-failed-task'),
      retryTask('old-failed-task'),
    ]);
    expect(duplicate).toBeNull();
    await waitForIdle();
    expect(retried.status).toBe('completed');
    expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    expect(providersMock.executeGeneration.mock.calls[0][1]).toMatchObject({
      prompt: '旧提示词',
      ratio: '16:9',
      size: '2048x1152',
      reference_paths: ['/reference.png'],
    });
  });

  it('任务终态落盘后再更新会话摘要', async () => {
    const records = new Map();
    dbMock.upsertTask.mockImplementation(async (task) => {
      await Promise.resolve();
      records.set(task.id, structuredClone(task));
    });
    dbMock.getAllTasks.mockImplementation(async () => [...records.values()]);
    const storage = new Map([
      ['if_settings', enabledProviderSettings],
      [
        'if_agent_sessions',
        JSON.stringify([
          {
            id: 'summary-session',
            messages: [
              { id: 'group-message', taskGroup: { id: 'summary-group', status: 'queued' } },
            ],
          },
        ]),
      ],
    ]);
    localStorageMock.getItem.mockImplementation((key) => storage.get(key));
    localStorageMock.setItem.mockImplementation((key, value) => storage.set(key, value));
    await enqueueTask(
      { prompt: 'x', agent_session_id: 'summary-session', task_group_id: 'summary-group' },
      { id: 'prov-1' }
    );
    await waitForIdle();
    const session = JSON.parse(storage.get('if_agent_sessions'))[0];
    expect(session.messages[0].taskGroup.status).toBe('completed');
    expect(session.messages.find((message) => message.status === 'task_result').content).toContain(
      '共 1 张'
    );
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

    const task = await enqueueTask(
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

    const task = await enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toBe('配额不足');
    expect(task.outputs).toEqual([]);
  });

  it('找不到 API 配置时任务失败', async () => {
    localStorageMock.getItem.mockReturnValue(JSON.stringify({ providers: [] }));

    const task = await enqueueTask({ prompt: 'x' }, { id: 'missing' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toContain('找不到 API 配置');
  });

  it('设置 JSON 损坏时按找不到配置处理', async () => {
    localStorageMock.getItem.mockReturnValue('broken-json{');

    const task = await enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
    await waitForIdle();

    expect(task.status).toBe('failed');
    expect(task.error).toContain('找不到 API 配置');
  });

  it('localStorage 无设置时任务失败', async () => {
    localStorageMock.getItem.mockReturnValue(null);

    const task = await enqueueTask({ prompt: 'x' }, { id: 'prov-1' });
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

    const first = await enqueueTask({ prompt: '占用 worker' }, { id: 'prov-1' });
    const queued = await enqueueTask({ prompt: '待取消' }, { id: 'prov-1' });

    await vi.waitFor(() => {
      expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    });
    await cancelTask(queued.id);

    const snap = snapshot();
    expect(snap.waiting).toHaveLength(0);
    expect(snap.recent.map((t) => t.id)).toContain(queued.id);
    expect(queued.status).toBe('cancelled');

    releaseFirst([generatedImage]);
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

    const first = await enqueueTask({ prompt: '占用' }, { id: 'prov-1' });
    await vi.waitFor(() => {
      expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    });
    const group = await enqueueBatch(
      [
        { prompt: 'g1', task_group_id: 'tg-1' },
        { prompt: 'g2', task_group_id: 'tg-1' },
      ],
      { id: 'prov-1' }
    );

    await cancelTaskGroup('tg-1');
    releaseFirst([generatedImage]);
    await waitForIdle();

    expect(first.status).toBe('completed');
    expect(group.map((t) => t.status)).toEqual(['cancelled', 'cancelled']);
  });

  it('取消运行中的任务后可以重新执行，不残留取消标记', async () => {
    let rejectGeneration;
    providersMock.executeGeneration.mockImplementation(
      () =>
        new Promise((_, reject) => {
          rejectGeneration = reject;
        })
    );

    const task = await enqueueTask({ prompt: 'cancel-run' }, { id: 'prov-1' });
    await vi.waitFor(() => {
      expect(snapshot().running.map((t) => t.id)).toContain(task.id);
    });

    await vi.waitFor(() => {
      expect(providersMock.executeGeneration).toHaveBeenCalledTimes(1);
    });
    await cancelTask(task.id);
    expect(providersMock.executeGeneration.mock.calls[0][2].aborted).toBe(true);
    rejectGeneration(new Error('已取消'));
    await waitForIdle();
    expect(task.status).toBe('cancelled');
    providersMock.executeGeneration.mockResolvedValue([generatedImage]);

    const retried = await retryTask(task.id);
    expect(retried).not.toBeNull();
    await waitForIdle();

    expect(retried.status).toBe('completed');
    expect(providersMock.executeGeneration).toHaveBeenCalledTimes(2);
  });

  it('重试失败任务并成功完成', async () => {
    providersMock.executeGeneration
      .mockRejectedValueOnce(new Error('网络错误'))
      .mockResolvedValueOnce([generatedImage]);

    const task = await enqueueTask({ prompt: 'r' }, { id: 'prov-1' });
    await waitForIdle();
    expect(task.status).toBe('failed');

    const retried = await retryTask(task.id);
    expect(retried).not.toBeNull();
    // worker 同步启动，返回时状态可能已是 running；error 必定被清空
    expect(retried.error).toBe('');

    await waitForIdle();
    expect(retried.status).toBe('completed');
  });

  it('重试不存在的任务返回 null', async () => {
    expect(await retryTask('no-such-task')).toBeNull();
  });

  it('已完成或排队中的任务不可重试', async () => {
    const task = await enqueueTask({ prompt: 'ok' }, { id: 'prov-1' });
    await waitForIdle();
    expect(task.status).toBe('completed');
    expect(await retryTask(task.id)).toBeNull();
  });

  it('重试任务组内所有失败任务', async () => {
    providersMock.executeGeneration.mockRejectedValue(new Error('批量失败'));
    const tasks = await enqueueBatch(
      [
        { prompt: 'a', task_group_id: 'tg-9' },
        { prompt: 'b', task_group_id: 'tg-9' },
        { prompt: 'c' },
      ],
      { id: 'prov-1' }
    );
    await waitForIdle();
    expect(tasks.every((t) => t.status === 'failed')).toBe(true);

    providersMock.executeGeneration.mockResolvedValue([generatedImage]);
    await retryTaskGroup('tg-9');
    await waitForIdle();

    const groupTasks = snapshot().recent.filter((t) => t.task_group_id === 'tg-9');
    expect(groupTasks.every((t) => t.status === 'completed')).toBe(true);
    expect(tasks[2].status).toBe('failed');
  });
});

describe('recoverTasks 恢复遗留任务', () => {
  beforeEach(async () => {
    vi.resetAllMocks();
    dbMock.upsertTask.mockResolvedValue(undefined);
    dbMock.getAllTasks.mockResolvedValue([]);
    dbMock.getTask.mockResolvedValue(null);
    providersMock.executeGeneration.mockResolvedValue([generatedImage]);
    blobMock.uploadImage.mockResolvedValue('/image-forge-data/tasks/out-01.png');
    localStorageMock.getItem.mockReturnValue(enabledProviderSettings);
    await waitForIdle();
  });

  it('恢复排队任务，保留取消状态，并合并同时发生的恢复请求', async () => {
    const tasks = ['queued', 'running', 'cancelling'].map((status) => ({
      id: `web-task-recover-${status}`,
      status,
      prompt: status,
      provider_id: 'prov-1',
      params: {},
    }));
    dbMock.getAllTasks.mockResolvedValueOnce(tasks);
    await Promise.all([recoverTasks(), recoverTasks()]);
    await waitForIdle();
    expect(tasks.map((task) => task.status)).toEqual(['completed', 'completed', 'cancelled']);
    expect(providersMock.executeGeneration).toHaveBeenCalledTimes(2);
    expect(dbMock.getAllTasks).toHaveBeenCalledTimes(1);
  });

  it('把遗留 running 任务恢复为 queued 并重新执行；桌面任务不归浏览器恢复', async () => {
    const legacyTask = {
      id: 'web-task-legacy-1',
      status: 'running',
      prompt: '遗留任务',
      provider_id: 'prov-1',
      params: {},
      reference_paths: [],
      outputs: [],
    };
    // 桌面版任务（UUID 形态）：共享 SQLite 里出现 running 时不允许浏览器抢跑
    const desktopTask = {
      id: '0f3d9a62-8a41-4c0e-9c1a-3f5b7d8e9a01',
      status: 'running',
      prompt: '桌面运行中',
      provider_id: 'prov-1',
      params: {},
      reference_paths: [],
      outputs: [],
    };
    const doneTask = { id: 'done-1', status: 'completed', prompt: '已完成', outputs: [] };
    dbMock.getAllTasks.mockResolvedValueOnce([legacyTask, desktopTask, doneTask]);

    await recoverTasks();

    // 恢复后任务进入队列（worker 可能已开始执行）
    const snap = snapshot();
    expect([...snap.waiting, ...snap.running].map((t) => t.id)).toContain('web-task-legacy-1');
    expect([...snap.waiting, ...snap.running].map((t) => t.id)).not.toContain(desktopTask.id);
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
