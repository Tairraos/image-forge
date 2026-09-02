// templateFill.spec.js — Web 版 AI 模板填充适配器测试（mock fetch）
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fillPromptTemplate } from '../../src/api/adapter-web.js';

const fetchMock = vi.fn();

function createLocalStorageMock() {
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
}

const localStorageMock = createLocalStorageMock();
Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock, writable: true });

beforeEach(() => {
  vi.stubGlobal('fetch', fetchMock);
  localStorageMock.clear();
  localStorage.setItem(
    'if_settings',
    JSON.stringify({
      providers: [
        {
          id: 'chat-1',
          name: '对话模型',
          modelType: 'chat',
          baseUrl: 'https://c.example.com/v1',
          apiKey: 'k',
          imageModel: 'chat-model',
        },
      ],
    })
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('fillPromptTemplate（Web 版）', () => {
  it('空模板直接报错', async () => {
    await expect(fillPromptTemplate('s', 'chat-1', '  ')).rejects.toThrow('模板内容不能为空');
  });

  it('缺少对话模型配置时报错', async () => {
    localStorage.setItem('if_settings', JSON.stringify({ providers: [] }));
    await expect(fillPromptTemplate('s', 'chat-1', '模板')).rejects.toThrow('请选择可用的对话模型');
  });

  it('调用对话模型并返回填充后的文本', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => ({ choices: [{ message: { content: ' 一张夕阳下的海滩海报 ' } }] }),
    });

    const result = await fillPromptTemplate('sess-1', 'chat-1', '一张{主题}海报');

    expect(result).toBe('一张夕阳下的海滩海报');
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe('https://c.example.com/v1/chat/completions');
    const payload = JSON.parse(init.body);
    expect(payload.messages[0].role).toBe('system');
    expect(payload.messages[0].content).toContain('模板填充助手');
    expect(payload.messages[1].content).toBe('一张{主题}海报');
  });

  it('HTTP 错误归一化为可读信息', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: async () => JSON.stringify({ error: { message: '炸了' } }),
    });
    await expect(fillPromptTemplate('s', 'chat-1', '模板')).rejects.toThrow(
      '模板填充失败: HTTP 500 炸了'
    );
  });
});
