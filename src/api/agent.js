// Web 版 Agent 对话引擎，对应桌面版 agent.rs + chat.rs。
// 支持 OpenAI 兼容的流式 Chat Completions + 工具调用循环。

import * as queue from './queue.js';
import * as db from './db.js';

const MAX_TOOL_ROUNDS = 8;
const AGENT_SCHEMA_VERSION = 1;

// ── 系统提示词 ──

function systemPrompt(context) {
  return `你是 Image Forge 本地绘画助手。普通聊天直接回答；需要绘图时必须调用 create_image_tasks。禁止声称执行终端、脚本、任意文件读写、任意 HTTP、浏览器、数据库或插件。缺少绘图信息时返回 schemaVersion=1 的 assistant envelope，status=needs_input 并在 questions 中提出最多 3 个问题；无法完成时返回 status=rejected 和原因；信息完整时返回 status=ready 及逐图 plans，或调用 create_image_tasks。每个 plan 必须明确 resolution、ratio、quality、promptFidelity、referencePolicy 和 referenceIds；referencePolicy=optional 时如果 referenceIds 为空，默认沿用当前附图。参考图只有 ID 和元数据；不支持视觉的模型不能假装看到了图片内容。

当前会话上下文：
${context.trim()}`;
}

// ── 工具定义 ──

const TOOLS = [
  {
    type: 'function',
    function: {
      name: 'create_image_tasks',
      description: '把已经完整明确的单图或多图计划原子提交到绘画队列。',
      parameters: {
        type: 'object',
        properties: {
          plans: {
            type: 'array',
            minItems: 1,
            maxItems: 12,
            items: {
              type: 'object',
              properties: {
                title: { type: 'string' },
                prompt: { type: 'string' },
                providerId: { type: 'string' },
                resolution: { enum: ['standard', '2k', '3k', '4k'] },
                ratio: { type: 'string' },
                quality: { enum: ['auto', 'low', 'medium', 'high'] },
                promptFidelity: { enum: ['original', 'strict', 'off'] },
                referencePolicy: { enum: ['use', 'optional', 'none'] },
                referenceIds: { type: 'array', items: { type: 'string' } },
              },
              required: [
                'title',
                'prompt',
                'resolution',
                'ratio',
                'quality',
                'promptFidelity',
                'referencePolicy',
                'referenceIds',
              ],
              additionalProperties: false,
            },
          },
        },
        required: ['plans'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'get_task_status',
      description: '只读查询一个任务组或单个绘图任务的当前状态。',
      parameters: {
        type: 'object',
        properties: {
          taskGroupId: { type: 'string' },
          taskId: { type: 'string' },
        },
        additionalProperties: false,
        anyOf: [{ required: ['taskGroupId'] }, { required: ['taskId'] }],
      },
    },
  },
];

// ── 流式 Chat Completions ──

/**
 * 发送 OpenAI 兼容的流式 Chat Completions 请求。
 * @param {Object} provider - { baseUrl, apiKey, imageModel }
 * @param {Array} messages - 对话消息数组
 * @param {Function} onDelta - 文本增量回调
 * @returns {Promise<{text: string, toolCalls: Array}>}
 */
async function chatCompletion(provider, messages, onDelta) {
  const baseUrl = (provider.baseUrl || '').replace(/\/+$/, '');
  const apiKey = (provider.apiKey || '').trim();
  const model = provider.imageModel || 'gpt-4o';

  const payload = {
    model,
    messages,
    tools: TOOLS,
    tool_choice: 'auto',
    temperature: 0.2,
    stream: true,
  };

  const res = await fetch(`${baseUrl}/chat/completions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      Accept: 'text/event-stream',
      'Accept-Language': 'zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7',
    },
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    const body = await res.text();
    if (res.status === 400 && body.includes('does not support tools')) {
      throw new Error(`AGENT_TOOLS_UNSUPPORTED: HTTP ${res.status} ${body}`);
    }
    let msg = body;
    try {
      msg = JSON.parse(body).error?.message || body;
    } catch {
      /* ignore parse error */
    }
    throw new Error(`Agent 请求失败: HTTP ${res.status} ${msg}`);
  }

  return parseSSEStream(res, onDelta);
}

/**
 * 非流式回退：用于不支持 tools 的模型。
 */
async function chatCompletionNonStream(provider, messages) {
  const baseUrl = (provider.baseUrl || '').replace(/\/+$/, '');
  const apiKey = (provider.apiKey || '').trim();
  const model = provider.imageModel || 'gpt-4o';

  const payload = {
    model,
    messages,
    temperature: 0.2,
    stream: false,
  };

  const res = await fetch(`${baseUrl}/chat/completions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    const body = await res.text();
    let msg = body;
    try {
      msg = JSON.parse(body).error?.message || body;
    } catch {
      /* ignore parse error */
    }
    throw new Error(`Agent 请求失败: HTTP ${res.status} ${msg}`);
  }

  const json = await res.json();
  const choice = json.choices?.[0] || {};
  return {
    text: choice.message?.content || '',
    toolCalls: [],
  };
}

// ── SSE 流解析 ──

async function parseSSEStream(res, onDelta) {
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let fullText = '';
  const toolCallAccum = new Map(); // index -> { id, name, arguments }

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || !trimmed.startsWith('data:')) continue;
      const data = trimmed.slice(5).trim();
      if (data === '[DONE]') continue;

      try {
        const chunk = JSON.parse(data);
        const delta = chunk.choices?.[0]?.delta;
        if (!delta) continue;

        // 文本增量
        if (delta.content) {
          fullText += delta.content;
          onDelta({ phase: 'delta', chunk: delta.content });
        }

        // 工具调用增量
        const toolCalls = delta.tool_calls || [];
        for (const tc of toolCalls) {
          const idx = tc.index ?? 0;
          if (!toolCallAccum.has(idx)) {
            toolCallAccum.set(idx, {
              id: tc.id || '',
              name: '',
              arguments: '',
            });
          }
          const acc = toolCallAccum.get(idx);
          if (tc.id) acc.id = tc.id;
          if (tc.function?.name) acc.name += tc.function.name;
          if (tc.function?.arguments) acc.arguments += tc.function.arguments;
        }
      } catch {
        // 忽略解析错误的行
      }
    }
  }

  const toolCalls = Array.from(toolCallAccum.values()).filter((tc) => tc.name && tc.arguments);

  return { text: fullText, toolCalls };
}

// ── Agent 主循环 ──

/**
 * 执行一个完整的 Agent 对话轮次。
 * @param {Object} provider - 对话模型配置
 * @param {Object} session - Agent 会话对象
 * @param {string} content - 用户消息
 * @param {Array} attachments - 附件列表
 * @param {Function} onEvent - 事件回调 ({ phase, chunk, message, toolName, sessionId })
 * @returns {Promise<Object>} 更新后的 session
 */
export async function runAgentTurn(provider, session, content, attachments, onEvent) {
  // 构建上下文
  const context = buildContext(session, attachments);
  const systemMsg = { role: 'system', content: systemPrompt(context) };

  // 构建消息列表；调用方（adapter-web）可能已把当前用户消息预写入会话，跳过避免重复
  const history = session.messages || [];
  const last = history[history.length - 1];
  const skipTrailingDuplicate =
    Boolean(last) &&
    last.role === 'user' &&
    (last.content || '') === content &&
    (content || '') !== '';
  const historyMessages = skipTrailingDuplicate ? history.slice(0, -1) : history;
  const messages = [systemMsg, ...agentMessagesToChat(historyMessages)];
  // 用户消息
  const userMsg = buildUserMessage(content, attachments);
  messages.push(userMsg);

  let completedToolCalls = [];
  let fallbackMode = false;

  for (let round = 0; round < MAX_TOOL_ROUNDS; round++) {
    let response;

    if (fallbackMode) {
      // 非流式回退
      const nonStream = await chatCompletionNonStream(provider, messages);
      const envelope = parseEnvelope(nonStream.text);
      if (envelope) {
        const result = handleEnvelope(envelope, completedToolCalls);
        if (result) return finalizeSession(session, messages, result);
      }
      response = nonStream;
    } else {
      try {
        response = await chatCompletion(provider, messages, (event) => {
          onEvent({ ...event, sessionId: session.id });
        });
      } catch (error) {
        if (error.message?.startsWith('AGENT_TOOLS_UNSUPPORTED:')) {
          fallbackMode = true;
          continue;
        }
        throw error;
      }
    }

    // 检查是否看起来像 JSON envelope（非 tools 模式下）
    if (!response.toolCalls.length && looksLikeEnvelope(response.text)) {
      const envelope = parseEnvelope(response.text);
      if (envelope) {
        const result = handleEnvelope(envelope, completedToolCalls);
        if (result) return finalizeSession(session, messages, result);
      }
    }

    // 无工具调用 = 纯文本回复
    if (!response.toolCalls.length) {
      return finalizeSession(session, messages, {
        text: response.text,
        status: 'chat',
        questions: [],
        toolCalls: completedToolCalls,
      });
    }

    // 执行工具调用
    const assistantMsg = {
      role: 'assistant',
      content: response.text || null,
      tool_calls: response.toolCalls.map((tc) => ({
        id: tc.id,
        type: 'function',
        function: { name: tc.name, arguments: tc.arguments },
      })),
    };
    messages.push(assistantMsg);

    for (const tc of response.toolCalls) {
      onEvent({
        phase: 'tool_start',
        message: `正在执行 ${tc.name}`,
        toolName: tc.name,
        sessionId: session.id,
      });

      let args;
      try {
        args = JSON.parse(tc.arguments);
      } catch {
        args = {};
      }

      const result = await executeToolCall(tc.name, args, session, attachments);

      const toolResult = {
        role: 'tool',
        tool_call_id: tc.id,
        name: tc.name,
        content: JSON.stringify({ result: result.value, error: result.error }),
      };
      messages.push(toolResult);

      completedToolCalls.push({
        schemaVersion: AGENT_SCHEMA_VERSION,
        id: tc.id,
        name: tc.name,
        arguments: args,
        result: result.value,
        error: result.error || null,
        status: result.error ? 'failed' : 'completed',
        createdAt: new Date().toISOString(),
        completedAt: new Date().toISOString(),
      });

      onEvent({
        phase: 'tool_result',
        message: result.error || '工具执行完成',
        toolName: tc.name,
        sessionId: session.id,
      });
    }
  }

  throw new Error('Agent Tool Call 超过最大循环次数，已停止本轮');
}

// ── 工具执行 ──

async function executeToolCall(name, args, session, attachments) {
  if (name === 'create_image_tasks') {
    return executeCreateImageTasks(args, session, attachments);
  }
  if (name === 'get_task_status') {
    return executeGetTaskStatus(args);
  }
  return { value: null, error: `未知工具: ${name}` };
}

async function executeCreateImageTasks(args, session, attachments) {
  const plans = args.plans || [];
  if (!plans.length || plans.length > 12) {
    return { value: null, error: '图片计划数量必须在 1 到 12 之间' };
  }

  const settings = JSON.parse(localStorage.getItem('if_settings') || '{}');
  const providers = settings.providers || [];

  // 构建 attachment id -> path 映射
  const attachmentMap = new Map();
  for (const msg of session.messages || []) {
    for (const att of msg.attachments || []) {
      attachmentMap.set(att.id, att.path);
    }
  }
  for (const att of attachments || []) {
    attachmentMap.set(att.id, att.path);
  }

  const taskGroupId = `web-tg-${Date.now()}`;
  const taskIds = [];
  const titles = [];

  for (const plan of plans) {
    const policy = ['use', 'optional', 'none'].includes(plan.referencePolicy)
      ? plan.referencePolicy
      : 'optional';

    if (policy === 'use' && !plan.referenceIds?.length) {
      return { value: null, error: 'referencePolicy=use 时必须指定参考图' };
    }

    let refPaths = [];
    if (policy !== 'none') {
      refPaths = (plan.referenceIds || []).map((id) => attachmentMap.get(id) || '').filter(Boolean);
    }

    const provider =
      providers.find((p) => p.id === plan.providerId) ||
      providers.find((p) => p.modelType !== 'chat') ||
      providers[0];

    if (!provider) {
      return { value: null, error: '没有可用的生图 API 配置' };
    }

    const request = {
      origin: 'agent',
      agent_session_id: session.id,
      task_group_id: taskGroupId,
      prompt: plan.prompt || '',
      model: provider.imageModel || '',
      ratio: plan.ratio || '1:1',
      resolution: plan.resolution || 'standard',
      count: 1,
      output_format: 'png',
      quality: plan.quality || 'auto',
      reference_paths: refPaths,
    };

    const task = queue.enqueueTask(request, provider);
    taskIds.push(task.id);
    titles.push(plan.title || '图片');
  }

  return {
    value: {
      taskGroupId,
      taskIds,
      titles,
      status: 'queued',
      message: `已创建 ${taskIds.length} 个绘图任务`,
    },
    error: '',
  };
}

async function executeGetTaskStatus(args) {
  const taskGroupId = args.taskGroupId || '';
  const taskId = args.taskId || '';
  const all = await db.getAllTasks();
  const tasks = all.filter((t) => t.task_group_id === taskGroupId || t.id === taskId);
  return {
    value: {
      tasks: tasks.map((t) => ({
        id: t.id,
        status: t.status,
        prompt: t.prompt || '',
        model: t.model || '',
        outputs: t.outputs?.length || 0,
      })),
      count: tasks.length,
    },
    error: '',
  };
}

// ── Envelope 降级协议 ──

function parseEnvelope(text) {
  const body = extractJSON(text);
  if (!body) return null;
  try {
    const env = JSON.parse(body);
    if (
      env.schemaVersion &&
      (env.type === 'assistant' || env.type === 'tool_call' || env.type === 'tool_result')
    ) {
      return env;
    }
  } catch {
    // 不是有效 JSON
  }
  return null;
}

function looksLikeEnvelope(text) {
  return /"schemaVersion"\s*:\s*\d/.test(text) || /"type"\s*:\s*"(assistant|tool_call)"/.test(text);
}

function extractJSON(text) {
  // 提取第一个 JSON 对象
  const start = text.indexOf('{');
  if (start < 0) return null;
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    if (text[i] === '{') depth++;
    if (text[i] === '}') depth--;
    if (depth === 0) return text.slice(start, i + 1);
  }
  return null;
}

function handleEnvelope(envelope, completedToolCalls) {
  if (envelope.type === 'assistant') {
    return {
      text: envelope.content || '',
      status: envelope.status || 'chat',
      questions: envelope.questions || [],
      toolCalls: completedToolCalls,
    };
  }
  return null;
}

// ── 会话最终化 ──

function buildContext(session, attachments) {
  const parts = [];
  parts.push(`会话 ID: ${session.id}`);
  parts.push(`会话标题: ${session.title || '新对话'}`);
  const refCount = (attachments || []).length;
  if (refCount) {
    const refInfo = attachments
      .map((a) => `  - ${a.id}: ${a.fileName || '图片'} (${a.mimeType || 'image/png'})`)
      .join('\n');
    parts.push(`当前参考图 (${refCount} 张):\n${refInfo}`);
  }
  return parts.join('\n');
}

function buildUserMessage(content, attachments) {
  const parts = [{ type: 'text', text: content }];
  for (const att of attachments || []) {
    if (att.dataUrl) {
      parts.push({
        type: 'image_url',
        image_url: { url: att.dataUrl, detail: 'auto' },
      });
    }
  }
  return {
    role: 'user',
    content: parts.length === 1 ? content : parts,
  };
}

const FOLD_TEXT_LIMIT = 400;

// 历史消息重建：assistant 带 toolCall 时补齐配对的 tool 结果消息，
// tool/task_group-only 消息折叠为 assistant 文本，避免产生孤立 role=tool 或未配对的 tool_calls。
function agentMessagesToChat(messages) {
  const seenToolCallIds = new Set();
  const out = [];
  for (const msg of messages || []) {
    out.push(...agentMessageToChat(msg, seenToolCallIds));
  }
  return out;
}

function limitFoldText(text) {
  const compact = String(text || '').trim();
  if (compact.length <= FOLD_TEXT_LIMIT) return compact;
  return `${compact.slice(0, FOLD_TEXT_LIMIT)}…`;
}

function taskGroupNote(taskGroup) {
  if (!taskGroup) return '';
  const count = taskGroup.taskIds?.length || 0;
  const id = taskGroup.id ? `taskGroupId=${taskGroup.id}` : '';
  return `（绘图任务组 ${id}：${taskGroup.status || 'unknown'}，共 ${count} 张）`;
}

function toolCallsOf(msg) {
  if (Array.isArray(msg.toolCalls) && msg.toolCalls.length) return msg.toolCalls;
  if (msg.toolCall) return [msg.toolCall];
  return [];
}

function agentMessageToChat(msg, seenToolCallIds = new Set()) {
  if (msg.role === 'user') {
    // 用户消息可能包含附件
    const parts = [{ type: 'text', text: msg.content || '' }];
    for (const att of msg.attachments || []) {
      if (att.dataUrl) {
        parts.push({
          type: 'image_url',
          image_url: { url: att.dataUrl, detail: 'auto' },
        });
      }
    }
    return [
      {
        role: 'user',
        content: parts.length === 1 ? msg.content || '' : parts,
      },
    ];
  }

  const calls = toolCallsOf(msg);

  // 历史里的 tool 消息（含跨端同步）：没有可配对的前置 assistant 时补一个；
  // 重复的 tool_call_id 直接折叠为文本，避免出现孤立 tool 消息。
  if (msg.role === 'tool' && calls.length) {
    const out = [];
    for (const call of calls) {
      if (seenToolCallIds.has(call.id)) {
        out.push({ role: 'assistant', content: `（已调用工具 ${call.name}）` });
        continue;
      }
      seenToolCallIds.add(call.id);
      out.push({
        role: 'assistant',
        content: null,
        tool_calls: [
          {
            id: call.id,
            type: 'function',
            function: {
              name: call.name,
              arguments: JSON.stringify(call.arguments ?? {}),
            },
          },
        ],
      });
      out.push(syntheticToolResult(call));
    }
    return out;
  }

  if (msg.role === 'tool') {
    // task_group / task_result 等没有 tool_call 的消息：折叠为 assistant 文本
    if (msg.taskGroup) {
      return [
        {
          role: 'assistant',
          content: `已创建 ${(msg.taskGroup.taskIds || []).length} 个绘图任务${taskGroupNote(msg.taskGroup)}`,
        },
      ];
    }
    return [{ role: 'assistant', content: limitFoldText(msg.content) }];
  }

  if (msg.role === 'assistant') {
    let content = msg.content || null;
    if (msg.taskGroup && !calls.length) {
      const note = taskGroupNote(msg.taskGroup);
      content = content ? `${content}\n\n${note}` : note;
    }
    if (!calls.length) {
      return [{ role: 'assistant', content }];
    }
    const out = [
      {
        role: 'assistant',
        content,
        tool_calls: calls.map((call) => ({
          id: call.id,
          type: 'function',
          function: {
            name: call.name,
            arguments: JSON.stringify(call.arguments ?? {}),
          },
        })),
      },
    ];
    for (const call of calls) {
      if (seenToolCallIds.has(call.id)) continue;
      seenToolCallIds.add(call.id);
      out.push(syntheticToolResult(call));
    }
    return out;
  }

  return [{ role: msg.role || 'user', content: msg.content || '' }];
}

function syntheticToolResult(call) {
  return {
    role: 'tool',
    tool_call_id: call.id,
    name: call.name,
    content: limitFoldText(
      JSON.stringify({ result: call.result ?? null, error: call.error ?? '' })
    ),
  };
}

function finalizeSession(session, messages, result) {
  // 把 assistant 消息持久化到 session（toolCalls 全量落盘，toolCall 保留首项兼容旧 UI）
  const now = new Date().toISOString();
  const assistantMsg = {
    id: `web-msg-${Date.now()}`,
    role: 'assistant',
    content: result.text || '',
    createdAt: now,
    taskGroup:
      result.toolCalls.length > 0
        ? {
            id: result.toolCalls[0]?.result?.taskGroupId || `web-tg-${Date.now()}`,
            status: 'queued',
            taskIds: result.toolCalls.flatMap((tc) => tc.result?.taskIds || []),
            titles: result.toolCalls.flatMap((tc) => tc.result?.titles || []),
          }
        : null,
    toolCall: result.toolCalls.length > 0 ? result.toolCalls[0] : null,
    toolCalls: result.toolCalls,
    questions: result.questions || [],
  };

  session.messages = [...(session.messages || []), assistantMsg];
  session.updatedAt = now;

  // 保存到 localStorage
  const sessions = JSON.parse(localStorage.getItem('if_agent_sessions') || '[]');
  const idx = sessions.findIndex((s) => s.id === session.id);
  if (idx >= 0) {
    sessions[idx] = session;
  } else {
    sessions.push(session);
  }
  localStorage.setItem('if_agent_sessions', JSON.stringify(sessions));

  return session;
}

export { AGENT_SCHEMA_VERSION, TOOLS };
