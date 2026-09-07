// agent.spec.js — Web 版 Agent 对话引擎测试（mock fetch，不发真实请求）
// 覆盖流式解析、工具调用循环、Envelope 降级与非流式回退。

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AGENT_SCHEMA_VERSION, TOOLS, runAgentTurn } from '../../src/api/agent.js';
import { enqueueTask } from '../../src/api/queue.js';
import { getAllTasks } from '../../src/api/db.js';

vi.mock('../../src/api/queue.js', () => ({
  enqueueTask: vi.fn(() => ({ id: 'task-1' })),
}));

vi.mock('../../src/api/db.js', () => ({
  getAllTasks: vi.fn(async () => []),
}));

const fetchMock = vi.fn();
const chatProvider = {
  baseUrl: 'https://chat.example.com/v1',
  apiKey: 'k',
  imageModel: 'gpt-test',
};

const imagePlan = {
  title: '柴犬',
  prompt: '一只柴犬',
  resolution: 'standard',
  ratio: '1:1',
  quality: 'high',
  promptFidelity: 'original',
  referencePolicy: 'none',
  referenceIds: [],
};

function sseResponse(lines) {
  const encoder = new TextEncoder();
  return {
    ok: true,
    status: 200,
    body: {
      getReader() {
        let index = 0;
        return {
          async read() {
            if (index < lines.length) {
              const value = lines[index];
              index += 1;
              return { done: false, value: encoder.encode(value) };
            }
            return { done: true };
          },
        };
      },
    },
  };
}

function textChunk(text) {
  return `data: ${JSON.stringify({ choices: [{ delta: { content: text } }] })}\n\n`;
}

// 分三段下发工具调用，模拟流式拼接 name / arguments
function toolCallChunks(id, name, args) {
  const half = Math.ceil(args.length / 2);
  return [
    `data: ${JSON.stringify({ choices: [{ delta: { tool_calls: [{ index: 0, id, function: { name: name.slice(0, 8) } }] } }] })}\n\n`,
    `data: ${JSON.stringify({ choices: [{ delta: { tool_calls: [{ index: 0, function: { name: name.slice(8), arguments: args.slice(0, half) } }] } }] })}\n\n`,
    `data: ${JSON.stringify({ choices: [{ delta: { tool_calls: [{ index: 0, function: { arguments: args.slice(half) } }] } }] })}\n\n`,
    'data: [DONE]\n\n',
  ];
}

function jsonNonStreamResponse(payload) {
  return { ok: true, status: 200, json: async () => payload };
}

function newSession(id) {
  return { id, title: '测试会话', messages: [], status: 'idle', task_group_ids: [] };
}

// 模拟 localStorage（agent 的会话落库与生图配置读取用）
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
Object.defineProperty(globalThis, 'localStorage', {
  value: localStorageMock,
  writable: true,
});

beforeEach(() => {
  vi.stubGlobal('fetch', fetchMock);
  localStorageMock.clear();
  localStorage.setItem(
    'if_settings',
    JSON.stringify({
      providers: [
        { id: 'prov-img', name: '生图', modelType: 'image-gpt', imageModel: 'img-model' },
      ],
    })
  );
  localStorage.setItem('if_agent_sessions', '[]');
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('纯文本回复', () => {
  it('流式解析文本并写入会话', async () => {
    fetchMock.mockResolvedValueOnce(
      sseResponse([textChunk('你好，'), textChunk('需要画什么？'), 'data: [DONE]\n\n'])
    );

    const events = [];
    const session = await runAgentTurn(chatProvider, newSession('sess-1'), '你好', [], (e) =>
      events.push(e)
    );

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe('https://chat.example.com/v1/chat/completions');
    expect(init.headers.Authorization).toBe('Bearer k');
    const payload = JSON.parse(init.body);
    expect(payload.stream).toBe(true);
    expect(payload.tools.map((t) => t.function.name)).toEqual([
      'create_image_tasks',
      'get_task_status',
      'list_templates',
    ]);
    expect(payload.messages.at(-1)).toEqual({ role: 'user', content: '你好' });

    expect(session.messages).toHaveLength(1);
    expect(session.messages[0]).toMatchObject({
      role: 'assistant',
      content: '你好，需要画什么？',
      taskGroup: null,
    });
    expect(events[0]).toMatchObject({ phase: 'delta', chunk: '你好，', sessionId: 'sess-1' });
  });

  it('带参考图时用户消息包含 image_url part 且上下文列出参考图', async () => {
    fetchMock.mockResolvedValueOnce(sseResponse([textChunk('收到'), 'data: [DONE]\n\n']));
    const attachments = [
      {
        id: 'att-1',
        fileName: 'a.png',
        mimeType: 'image/png',
        dataUrl: 'data:image/png;base64,AAAA',
        path: '/tmp/a.png',
      },
    ];

    await runAgentTurn(chatProvider, newSession('sess-2'), '看图', attachments, () => {});

    const payload = JSON.parse(fetchMock.mock.calls[0][1].body);
    expect(payload.messages[0].content).toContain('当前参考图 (1 张)');
    const userMsg = payload.messages.at(-1);
    expect(userMsg.content).toHaveLength(2);
    expect(userMsg.content[1].image_url.url).toBe('data:image/png;base64,AAAA');
  });
});

describe('工具调用循环', () => {
  it('先检查全部计划，后面的无效参考图不会导致前面的图片提前入队', async () => {
    const plans = [imagePlan, { ...imagePlan, referencePolicy: 'use', referenceIds: ['missing'] }];
    fetchMock.mockResolvedValueOnce(
      sseResponse(toolCallChunks('validate', 'create_image_tasks', JSON.stringify({ plans })))
    );
    fetchMock.mockResolvedValueOnce(
      sseResponse([textChunk('请重新添加参考图'), 'data: [DONE]\n\n'])
    );
    const session = await runAgentTurn(chatProvider, newSession('validate'), '绘画', [], () => {});
    expect(enqueueTask).not.toHaveBeenCalled();
    expect(session.messages.at(-1).toolCall.error).toContain('参考图 ID');
    expect(session.messages.at(-1).taskGroup).toBeNull();
  });

  it('optional 没有指定 ID 时沿用本轮附图', async () => {
    fetchMock.mockResolvedValueOnce(
      sseResponse(
        toolCallChunks(
          'optional',
          'create_image_tasks',
          JSON.stringify({ plans: [{ ...imagePlan, referencePolicy: 'optional' }] })
        )
      )
    );
    fetchMock.mockResolvedValueOnce(sseResponse([textChunk('已入队'), 'data: [DONE]\n\n']));
    await runAgentTurn(
      chatProvider,
      newSession('optional'),
      '按参考图绘画',
      [{ id: 'ref', path: '/reference.png' }],
      () => {}
    );
    expect(enqueueTask.mock.calls[0][0].reference_paths).toEqual(['/reference.png']);
  });

  it('停止后保留已创建任务的卡片，后续计划不会继续入队', async () => {
    const controller = new AbortController();
    enqueueTask.mockImplementationOnce(async () => {
      controller.abort();
      return { id: 'first-task' };
    });
    fetchMock.mockResolvedValueOnce(
      sseResponse(
        toolCallChunks(
          'cancel-plans',
          'create_image_tasks',
          JSON.stringify({ plans: [imagePlan, imagePlan] })
        )
      )
    );
    const session = await runAgentTurn(
      chatProvider,
      newSession('cancel-plans'),
      '两张图',
      [],
      () => {},
      controller.signal
    );
    expect(enqueueTask).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(session.messages.at(-1)).toMatchObject({
      status: 'cancelled',
      taskGroup: { taskIds: ['first-task'] },
    });
  });

  it('create_image_tasks 入队任务并在会话中记录任务组', async () => {
    const plans = [
      {
        title: '柴犬',
        prompt: '一只柴犬',
        resolution: 'standard',
        ratio: '1:1',
        quality: 'high',
        promptFidelity: 'original',
        referencePolicy: 'none',
        referenceIds: [],
      },
    ];
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(toolCallChunks('call-1', 'create_image_tasks', JSON.stringify({ plans })))
      )
      .mockResolvedValueOnce(sseResponse([textChunk('已创建任务'), 'data: [DONE]\n\n']));

    const events = [];
    const session = await runAgentTurn(chatProvider, newSession('sess-3'), '画柴犬', [], (e) =>
      events.push(e)
    );

    expect(enqueueTask).toHaveBeenCalledTimes(1);
    const [request, provider] = enqueueTask.mock.calls[0];
    expect(request).toMatchObject({
      origin: 'agent',
      agent_session_id: 'sess-3',
      prompt: '一只柴犬',
      model: 'img-model',
    });
    expect(request.task_group_id).toMatch(/^web-tg-/);
    expect(provider.id).toBe('prov-img');

    const lastMsg = session.messages.at(-1);
    expect(lastMsg.taskGroup).toMatchObject({
      status: 'queued',
      taskIds: ['task-1'],
      titles: ['柴犬'],
    });
    expect(lastMsg.toolCall.status).toBe('completed');

    expect(events.filter((e) => e.phase === 'tool_start')).toHaveLength(1);
    expect(
      events.filter((e) => e.phase === 'tool_result' && e.message === '工具执行完成')
    ).toHaveLength(1);

    const saved = JSON.parse(localStorage.getItem('if_agent_sessions'));
    expect(saved.some((s) => s.id === 'sess-3')).toBe(true);
  });

  it('referencePolicy=use 缺少参考图时工具报错回传', async () => {
    const plans = [
      {
        title: 'x',
        prompt: 'p',
        resolution: 'standard',
        ratio: '1:1',
        quality: 'high',
        promptFidelity: 'original',
        referencePolicy: 'use',
        referenceIds: [],
      },
    ];
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(toolCallChunks('call-2', 'create_image_tasks', JSON.stringify({ plans })))
      )
      .mockResolvedValueOnce(sseResponse([textChunk('好的'), 'data: [DONE]\n\n']));

    const events = [];
    await runAgentTurn(chatProvider, newSession('sess-4'), '画', [], (e) => events.push(e));

    expect(enqueueTask).not.toHaveBeenCalled();
    expect(
      events.some((e) => e.phase === 'tool_result' && e.message.includes('必须指定参考图'))
    ).toBe(true);
  });

  it('plans 为空时工具报错回传', async () => {
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(toolCallChunks('call-2b', 'create_image_tasks', JSON.stringify({ plans: [] })))
      )
      .mockResolvedValueOnce(sseResponse([textChunk('好的'), 'data: [DONE]\n\n']));

    const events = [];
    await runAgentTurn(chatProvider, newSession('sess-4b'), '画', [], (e) => events.push(e));

    expect(enqueueTask).not.toHaveBeenCalled();
    expect(events.some((e) => e.message.includes('图片计划数量必须在 1 到 12 之间'))).toBe(true);
  });

  it('没有可用生图配置时报错回传', async () => {
    localStorage.setItem('if_settings', JSON.stringify({ providers: [] }));
    const plans = [
      {
        title: 'x',
        prompt: 'p',
        resolution: 'standard',
        ratio: '1:1',
        quality: 'high',
        promptFidelity: 'original',
        referencePolicy: 'none',
        referenceIds: [],
      },
    ];
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(toolCallChunks('call-2c', 'create_image_tasks', JSON.stringify({ plans })))
      )
      .mockResolvedValueOnce(sseResponse([textChunk('好的'), 'data: [DONE]\n\n']));

    const events = [];
    await runAgentTurn(chatProvider, newSession('sess-4c'), '画', [], (e) => events.push(e));

    expect(enqueueTask).not.toHaveBeenCalled();
    expect(events.some((e) => e.message.includes('没有可用的生图 API 配置'))).toBe(true);
  });

  it('get_task_status 回传查询结果', async () => {
    getAllTasks.mockResolvedValueOnce([
      {
        id: 't-9',
        task_group_id: 'tg-9',
        status: 'completed',
        prompt: 'p',
        model: 'm',
        outputs: [{}],
      },
    ]);
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(
          toolCallChunks('call-3', 'get_task_status', JSON.stringify({ taskGroupId: 'tg-9' }))
        )
      )
      .mockResolvedValueOnce(sseResponse([textChunk('查完'), 'data: [DONE]\n\n']));

    await runAgentTurn(chatProvider, newSession('sess-5'), '查询', [], () => {});

    const secondPayload = JSON.parse(fetchMock.mock.calls[1][1].body);
    const toolMsg = secondPayload.messages.find((m) => m.role === 'tool');
    const result = JSON.parse(toolMsg.content).result;
    expect(result.count).toBe(1);
    expect(result.tasks[0]).toMatchObject({ id: 't-9', status: 'completed', outputs: 1 });
  });

  it('templateId 合并模板参考图，模板不存在时工具报错', async () => {
    localStorage.setItem(
      'if_templates',
      JSON.stringify([
        {
          id: 'tpl-1',
          title: '海报模板',
          content: '一张{主题}海报',
          referencePaths: ['/refs/tpl-a.png'],
        },
      ])
    );
    const withTemplate = { ...imagePlan, referencePolicy: 'optional', templateId: 'tpl-1' };
    const missingTemplate = {
      ...imagePlan,
      title: '另一张',
      referencePolicy: 'optional',
      templateId: 'tpl-404',
    };
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(
          toolCallChunks(
            'call-tid',
            'create_image_tasks',
            JSON.stringify({ plans: [withTemplate] })
          )
        )
      )
      .mockResolvedValueOnce(
        sseResponse(
          toolCallChunks(
            'call-tid2',
            'create_image_tasks',
            JSON.stringify({ plans: [missingTemplate] })
          )
        )
      )
      .mockResolvedValueOnce(sseResponse([textChunk('结束'), 'data: [DONE]\n\n']));

    await runAgentTurn(chatProvider, newSession('sess-tid'), '用模板画', [], () => {});

    const [request] = enqueueTask.mock.calls[0];
    expect(request.reference_paths).toEqual(['/refs/tpl-a.png']);
    // 模板不存在的失败结果会回传给模型
    const lastPayload = JSON.parse(fetchMock.mock.calls[2][1].body);
    const toolMsgs = lastPayload.messages.filter((m) => m.role === 'tool');
    expect(toolMsgs.at(-1).content).toContain('模板不存在：tpl-404');
  });

  it('超过最大循环次数时抛错停止', async () => {
    fetchMock.mockImplementation(async () =>
      sseResponse(toolCallChunks('call-loop', 'get_task_status', JSON.stringify({ taskId: 't' })))
    );

    await expect(
      runAgentTurn(chatProvider, newSession('sess-6'), 'loop', [], () => {})
    ).rejects.toThrow('超过最大循环次数');
    expect(fetchMock).toHaveBeenCalledTimes(8);
  });
});

describe('流式回复的结束与取消', () => {
  it('收到 DONE 就结束读取，保留没有末尾换行的最后一帧', async () => {
    const read = vi.fn().mockResolvedValueOnce({
      done: false,
      value: new TextEncoder().encode(textChunk('完整内容') + 'data: [DONE]\n\n'),
    });
    fetchMock.mockResolvedValueOnce({ ok: true, body: { getReader: () => ({ read }) } });
    const session = await runAgentTurn(chatProvider, newSession('done'), '你好', [], () => {});
    expect(session.messages.at(-1).content).toBe('完整内容');
    expect(read).toHaveBeenCalledTimes(1);
    fetchMock.mockResolvedValueOnce(sseResponse([textChunk('最后一帧').trimEnd()]));
    const ended = await runAgentTurn(chatProvider, newSession('last-frame'), '你好', [], () => {});
    expect(ended.messages.at(-1).content).toBe('最后一帧');
  });

  it('停止流式请求后保留已生成文字，状态为已停止', async () => {
    const controller = new AbortController();
    const events = [];
    fetchMock.mockImplementationOnce(async (_, init) => {
      expect(init.signal).toBe(controller.signal);
      return {
        ok: true,
        body: new ReadableStream({
          start(stream) {
            stream.enqueue(new TextEncoder().encode(textChunk('已经写下的内容')));
            init.signal.addEventListener('abort', () => stream.error(init.signal.reason), {
              once: true,
            });
          },
        }),
      };
    });
    const pending = runAgentTurn(
      chatProvider,
      newSession('stop-stream'),
      '你好',
      [],
      (event) => events.push(event),
      controller.signal
    );
    await vi.waitFor(() => expect(events.some((event) => event.phase === 'delta')).toBe(true));
    controller.abort();
    const session = await pending;
    expect(session.messages.at(-1)).toMatchObject({
      status: 'cancelled',
      content: '已经写下的内容',
    });
  });
});

describe('Envelope 降级与非流式回退', () => {
  it('识别 needs_input envelope 并记录问题', async () => {
    const envelope = {
      schemaVersion: 1,
      type: 'assistant',
      status: 'needs_input',
      content: '需要更多信息',
      questions: ['主体是什么？'],
    };
    fetchMock.mockResolvedValueOnce(
      sseResponse([
        `data: ${JSON.stringify({ choices: [{ delta: { content: JSON.stringify(envelope) } }] })}\n\n`,
        'data: [DONE]\n\n',
      ])
    );

    const session = await runAgentTurn(
      chatProvider,
      newSession('sess-7'),
      '画个东西',
      [],
      () => {}
    );

    expect(session.messages.at(-1)).toMatchObject({
      content: '需要更多信息',
      questions: ['主体是什么？'],
    });
  });

  it('模型不支持 tools 时回退非流式请求', async () => {
    fetchMock
      .mockResolvedValueOnce({
        ok: false,
        status: 400,
        text: async () => 'model does not support tools',
      })
      .mockResolvedValueOnce(
        jsonNonStreamResponse({ choices: [{ message: { content: '非流式回复' } }] })
      );

    const session = await runAgentTurn(chatProvider, newSession('sess-8'), 'hi', [], () => {});

    const payload = JSON.parse(fetchMock.mock.calls[1][1].body);
    expect(payload.stream).toBe(false);
    expect(payload.tools).toBeUndefined();
    expect(session.messages.at(-1).content).toBe('非流式回复');
  });

  it('普通 HTTP 错误直接上抛', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: async () => JSON.stringify({ error: { message: '炸了' } }),
    });

    await expect(
      runAgentTurn(chatProvider, newSession('sess-9'), 'hi', [], () => {})
    ).rejects.toThrow('Agent 请求失败: HTTP 500 炸了');
  });
});

describe('多轮对话历史重建', () => {
  it('第二轮请求中历史工具消息正确配对且用户消息不重复', async () => {
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(
          toolCallChunks('call-h1', 'create_image_tasks', JSON.stringify({ plans: [imagePlan] }))
        )
      )
      .mockResolvedValueOnce(sseResponse([textChunk('第一轮完成'), 'data: [DONE]\n\n']));
    const session = await runAgentTurn(chatProvider, newSession('sess-m1'), '画柴犬', [], () => {});

    fetchMock.mockResolvedValueOnce(sseResponse([textChunk('第二轮回复'), 'data: [DONE]\n\n']));
    await runAgentTurn(chatProvider, session, '再来一张', [], () => {});

    const payload = JSON.parse(fetchMock.mock.calls[2][1].body);
    const msgs = payload.messages;
    expect(msgs.filter((m) => m.role === 'user' && m.content === '再来一张')).toHaveLength(1);
    expect(msgs.at(-1)).toEqual({ role: 'user', content: '再来一张' });
    for (let i = 0; i < msgs.length; i++) {
      if (msgs[i].role === 'tool') {
        expect(msgs[i - 1]?.role).toBe('assistant');
        expect(msgs[i - 1].tool_calls.map((c) => c.id)).toContain(msgs[i].tool_call_id);
      }
      if (msgs[i].tool_calls) {
        expect(msgs[i + 1]?.role).toBe('tool');
      }
    }
    const historyTool = msgs.find((m) => m.role === 'tool' && m.content.includes('task-1'));
    expect(historyTool).toBeTruthy();
  });

  it('跨端同步来的 task_group 消息在重建时折叠为 assistant 文本', async () => {
    const session = newSession('sess-m2');
    session.messages.push({
      id: 'm1',
      role: 'tool',
      status: 'task_group',
      content: '已创建 1 个绘图任务',
      taskGroup: { id: 'tg-x', status: 'completed', taskIds: ['t-1'], titles: ['海报'] },
    });

    fetchMock.mockResolvedValueOnce(sseResponse([textChunk('好的'), 'data: [DONE]\n\n']));
    await runAgentTurn(chatProvider, session, '继续', [], () => {});

    const payload = JSON.parse(fetchMock.mock.calls[0][1].body);
    expect(payload.messages.some((m) => m.role === 'tool')).toBe(false);
    const folded = payload.messages.find(
      (m) => m.role === 'assistant' && (m.content || '').includes('tg-x')
    );
    expect(folded).toBeTruthy();
    expect(folded.content).toContain('已创建 1 个绘图任务');
  });

  it('finalizeSession 全量落盘 toolCalls 且 toolCall 保留首项兼容旧 UI', async () => {
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(
          toolCallChunks('call-h2', 'create_image_tasks', JSON.stringify({ plans: [imagePlan] }))
        )
      )
      .mockResolvedValueOnce(sseResponse([textChunk('完成'), 'data: [DONE]\n\n']));
    const session = await runAgentTurn(chatProvider, newSession('sess-m3'), '画', [], () => {});

    const lastMsg = session.messages.at(-1);
    expect(lastMsg.toolCalls).toHaveLength(1);
    expect(lastMsg.toolCall.id).toBe(lastMsg.toolCalls[0].id);
  });

  it('图片先完成时，回复保留结果摘要和最新标题，并恢复任务组终态', async () => {
    const original = newSession('sess-fast-image');
    localStorage.setItem('if_agent_sessions', JSON.stringify([original]));
    enqueueTask.mockImplementationOnce(async (request) => {
      const current = JSON.parse(localStorage.getItem('if_agent_sessions'))[0];
      current.title = '最新会话标题';
      current.messages.push({
        id: 'fast-result',
        role: 'tool',
        status: 'task_result',
        content: `[taskGroupId=${request.task_group_id}] 绘图任务组已完成，共 1 张`,
      });
      localStorage.setItem('if_agent_sessions', JSON.stringify([current]));
      getAllTasks.mockResolvedValueOnce([
        { id: 'fast-task', task_group_id: request.task_group_id, status: 'completed' },
      ]);
      return { id: 'fast-task' };
    });
    fetchMock
      .mockResolvedValueOnce(
        sseResponse(
          toolCallChunks('call-fast', 'create_image_tasks', JSON.stringify({ plans: [imagePlan] }))
        )
      )
      .mockResolvedValueOnce(sseResponse([textChunk('图片已生成'), 'data: [DONE]\n\n']));
    const result = await runAgentTurn(chatProvider, original, '画一只柴犬', [], () => {});
    expect(result.title).toBe('最新会话标题');
    expect(result.messages.filter((message) => message.id === 'fast-result')).toHaveLength(1);
    expect(result.messages.at(-1).taskGroup.status).toBe('completed');
    expect(JSON.parse(localStorage.getItem('if_agent_sessions'))[0]).toEqual(result);
  });
});

describe('导出契约', () => {
  it('schema 版本与工具清单', () => {
    expect(AGENT_SCHEMA_VERSION).toBe(1);
    expect(TOOLS.map((t) => t.function.name)).toEqual([
      'create_image_tasks',
      'get_task_status',
      'list_templates',
    ]);
  });

  it('list_templates 返回模板摘要并截断内容', async () => {
    localStorage.setItem(
      'if_templates',
      JSON.stringify([
        {
          id: 'tpl-1',
          title: '电影感海报',
          content: '一部{主题}的电影海报'.repeat(30),
          referencePaths: ['/refs/a.png'],
        },
        { id: 'tpl-2', title: '极简图标', prompt: '极简风格的{物体}图标', referencePaths: [] },
      ])
    );
    fetchMock.mockResolvedValueOnce(
      sseResponse(toolCallChunks('call-tpl', 'list_templates', JSON.stringify({})))
    );
    fetchMock.mockResolvedValueOnce(sseResponse([textChunk('已列出'), 'data: [DONE]\n\n']));

    const session = await runAgentTurn(
      chatProvider,
      newSession('sess-tpl'),
      '有哪些模板',
      [],
      () => {}
    );
    expect(session.messages.at(-1).taskGroup).toBeNull();

    const secondPayload = JSON.parse(fetchMock.mock.calls[1][1].body);
    const toolMsg = secondPayload.messages.find((m) => m.role === 'tool');
    const result = JSON.parse(toolMsg.content).result;
    expect(result.templates).toHaveLength(2);
    expect(result.templates[0]).toMatchObject({
      id: 'tpl-1',
      title: '电影感海报',
      referenceCount: 1,
    });
    expect(result.templates[0].preview.length).toBeLessThanOrEqual(201);
    expect(result.templates[0].preview.endsWith('…')).toBe(true);
    expect(result.templates[1].preview).toBe('极简风格的{物体}图标');
  });
});
