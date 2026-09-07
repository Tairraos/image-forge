import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createAgentDirectImageTask,
  getAgentSession,
  sendAgentMessage,
  cancelAgentTurn,
} from '../../src/api/adapter-web.js';
import * as queue from '../../src/api/queue.js';
import * as localStore from '../../src/api/localStore.js';

vi.mock('../../src/api/queue.js', () => ({ enqueueTask: vi.fn() }));
vi.mock('../../src/api/localStore.js', () => ({
  readSettings: vi.fn(),
  readSessions: vi.fn(),
  writeSession: vi.fn(),
  updateSession: vi.fn(),
}));

const provider = {
  id: 'image-1',
  modelType: 'image-gpt',
  apiKey: 'test',
  imageModel: 'image-model',
};
const plan = { providerId: provider.id, ratio: '16:9', resolution: '2k' };
let session;

afterEach(() => vi.unstubAllGlobals());

it('Web 对话可停止，同一会话不能并发提交，用户消息在请求前已保存', async () => {
  localStore.readSettings.mockResolvedValue({
    providers: [{ ...provider, modelType: 'chat', baseUrl: 'https://example.com/v1' }],
  });
  const fetchMock = vi.fn(
    (_, init) =>
      new Promise((_, reject) => {
        init.signal.addEventListener('abort', () => reject(init.signal.reason), { once: true });
      })
  );
  vi.stubGlobal('fetch', fetchMock);
  const pending = sendAgentMessage(session.id, provider.id, '请写一段话', []);
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  expect(session.messages[0]).toMatchObject({ role: 'user', content: '请写一段话' });
  await expect(sendAgentMessage(session.id, provider.id, '不能重复提交', [])).rejects.toThrow(
    '正在生成'
  );
  await cancelAgentTurn(session.id);
  const result = await pending;
  expect(result.messages.at(-1).status).toBe('cancelled');
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

beforeEach(() => {
  vi.resetAllMocks();
  session = { id: 'session-1', title: '', messages: [] };
  localStore.readSettings.mockResolvedValue({ providers: [provider] });
  localStore.readSessions.mockImplementation(async () => [structuredClone(session)]);
  localStore.writeSession.mockImplementation(async (value) => {
    session = structuredClone(value);
    return session;
  });
  localStore.updateSession.mockImplementation(async (id, update) => {
    const current = (await localStore.readSessions()).find((item) => item.id === id);
    const updated = await update(current || null);
    return updated ? localStore.writeSession(updated) : null;
  });
  queue.enqueueTask.mockImplementation(async (request) => ({ ...request, status: 'queued' }));
});

it('对话请求失败时保留等待期间写入的图片结果', async () => {
  localStore.readSettings.mockResolvedValue({
    providers: [{ ...provider, modelType: 'chat', baseUrl: 'https://example.com/v1' }],
  });
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => {
      session.messages.push({
        id: 'image-result',
        role: 'tool',
        status: 'task_result',
        content: '图片已完成',
      });
      throw new Error('连接中断');
    })
  );
  const result = await sendAgentMessage(session.id, provider.id, '继续绘画', []);
  expect(result.messages.map((message) => message.id)).toContain('image-result');
  expect(result.messages.at(-1).error).toContain('连接中断');
});

it.each([true, false])('图片理解开关为 %s 时按设置发送图像，会话只保存路径', async (chatVision) => {
  localStore.readSettings.mockResolvedValue({
    providers: [{ ...provider, modelType: 'chat', chatVision, baseUrl: 'https://example.com/v1' }],
  });
  const fetchMock = vi.fn(async (url) => {
    if (url === '/reference.png') {
      return { ok: true, blob: async () => new Blob(['image-bytes'], { type: 'image/png' }) };
    }
    let sent = false;
    return {
      ok: true,
      body: {
        getReader: () => ({
          read: async () => {
            if (sent) return { done: true };
            sent = true;
            return {
              done: false,
              value: new TextEncoder().encode(
                'data: {"choices":[{"delta":{"content":"完成"}}]}\n\ndata: [DONE]\n\n'
              ),
            };
          },
        }),
      },
    };
  });
  vi.stubGlobal('fetch', fetchMock);
  await sendAgentMessage(session.id, provider.id, '参考这张图片', [
    {
      id: 'ref',
      path: '/reference.png',
      dataUrl: '不应绕过开关发送',
    },
  ]);
  expect(fetchMock).toHaveBeenCalledTimes(chatVision ? 2 : 1);
  const payload = JSON.parse(fetchMock.mock.calls.at(-1)[1].body);
  if (chatVision) {
    expect(payload.messages.at(-1).content[1].image_url.url).toMatch(/^data:image\/png;base64,/);
    expect(fetchMock.mock.calls[0][1].signal).toBeInstanceOf(AbortSignal);
  } else {
    expect(payload.messages.at(-1).content).toBe('参考这张图片');
  }
  expect(session.messages[0].attachments[0]).toMatchObject({ id: 'ref', path: '/reference.png' });
  expect(session.messages[0].attachments[0]).not.toHaveProperty('dataUrl');
});

it('读取参考图期间停止对话，不再发送模型请求', async () => {
  localStore.readSettings.mockResolvedValue({
    providers: [{ ...provider, modelType: 'chat', chatVision: true }],
  });
  const fetchMock = vi.fn(
    (_, init) =>
      new Promise((_, reject) => {
        init.signal.addEventListener('abort', () => reject(init.signal.reason), { once: true });
      })
  );
  vi.stubGlobal('fetch', fetchMock);
  const pending = sendAgentMessage(session.id, provider.id, '看图', [
    { id: 'ref', path: '/reference.png' },
  ]);
  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
  await cancelAgentTurn(session.id);
  const result = await pending;
  expect(result.messages.at(-1)).toMatchObject({ status: 'cancelled', error: '' });
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

describe('直接绘画', () => {
  it('先保存消息和任务组，再入队；刷新仍保留提示词、参考图和尺寸', async () => {
    queue.enqueueTask.mockImplementationOnce(async (request) => {
      expect(session.messages[1].taskGroup.taskIds).toContain(request.id);
      return request;
    });
    const attachments = [
      {
        id: 'ref-1',
        path: '/image-forge-data/ref.png',
        fileName: 'ref.png',
        dataUrl: 'data:image/png;base64,AA==',
      },
    ];
    const group = await createAgentDirectImageTask(session.id, ' 日落海报 ', attachments, {
      ...plan,
      prompt: '不应覆盖用户内容',
    });
    const restored = await getAgentSession(session.id);
    expect(restored.title).toBe('日落海报');
    expect(restored.messages[0]).toMatchObject({
      role: 'user',
      content: '日落海报',
      attachments: [{ id: 'ref-1', path: attachments[0].path }],
    });
    expect(restored.messages[0].attachments[0]).not.toHaveProperty('dataUrl');
    expect(restored.messages[1].taskGroup).toMatchObject(group);
    expect(queue.enqueueTask.mock.calls[0][0]).toMatchObject({
      prompt: '日落海报',
      size: '2048x1152',
      origin: 'agent-direct',
      reference_paths: ['/image-forge-data/ref.png'],
      task_group_id: group.id,
    });
  });

  it('入队失败保留原文和可重试错误，不留下不存在的任务组', async () => {
    queue.enqueueTask.mockRejectedValueOnce(new Error('磁盘不可写'));
    await expect(createAgentDirectImageTask(session.id, '猫', [], plan)).rejects.toThrow(
      '磁盘不可写'
    );
    expect(session.messages[0].content).toBe('猫');
    expect(session.messages[1].taskGroup).toBeNull();
    expect(session.messages[1].error).toContain('磁盘不可写');
  });

  it('存储和配置校验失败时不会发送请求', async () => {
    await expect(createAgentDirectImageTask(session.id, ' ', [], plan)).rejects.toThrow(
      '消息不能为空'
    );
    localStore.readSettings.mockResolvedValueOnce({ providers: [{ ...provider, apiKey: '' }] });
    await expect(createAgentDirectImageTask(session.id, '猫', [], plan)).rejects.toThrow('API Key');
    localStore.writeSession.mockRejectedValueOnce(new Error('会话保存失败'));
    await expect(createAgentDirectImageTask(session.id, '猫', [], plan)).rejects.toThrow(
      '会话保存失败'
    );
    expect(queue.enqueueTask).not.toHaveBeenCalled();
  });
});
