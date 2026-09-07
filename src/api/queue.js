// Web 版队列调度系统，对应桌面版 queue.rs。
// 单 worker 顺序处理任务，支持取消、恢复和重试。

import * as db from './db.js';
import * as localStore from './localStore.js';
import { executeGeneration } from './providers.js';
import { uploadImage } from './blob.js';
import { taskGroupStatus } from '../lib/libraryFormat.js';

// ── 队列状态 ──

let waiting = [];
let running = [];
let recent = [];
let workerActive = false;
let activeController = null;
let recovery = null;
const retrying = new Set();

// 事件回调（供 UI 层监听队列变化）
let onChangeCallback = null;

export function onQueueChange(callback) {
  onChangeCallback = callback;
}

function notifyChange() {
  if (onChangeCallback) {
    try {
      onChangeCallback(snapshot());
    } catch (error) {
      console.error('队列界面刷新失败:', error);
    }
  }
}

export function snapshot() {
  return {
    waiting: [...waiting],
    running: [...running],
    recent: [...recent],
    workerActive,
    updatedAt: new Date().toISOString(),
  };
}

// ── 入队 ──

/**
 * 单个任务入队
 * @param {Object} request - GenerateRequest 结构
 * @param {Object} provider - API 配置
 * @returns {Promise<Object>} 已持久化的 TaskRecord
 */
export async function enqueueTask(request, provider) {
  const now = new Date().toISOString();
  const id = request.id || `web-task-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const task = {
    id,
    created_at: now,
    updated_at: now,
    completed_at: '',
    library_date: now.slice(0, 10),
    prompt: request.prompt || '',
    model: request.model || provider?.imageModel || '',
    provider_name: provider?.name || '',
    provider_id: provider?.id || '',
    origin: request.origin || 'drawing',
    agent_session_id: request.agent_session_id || '',
    task_group_id: request.task_group_id || '',
    status: 'queued',
    reference_paths: request.reference_paths || [],
    params: {
      ratio: request.ratio || '1:1',
      resolution: request.resolution || '1K',
      quality: request.quality || '',
      count: request.count || 1,
      output_format: request.output_format || 'png',
      background: request.background || '',
      size: request.size || '',
    },
    outputs: [],
    usage: null,
  };
  // 先保存再执行，写入失败时不发送可能产生费用的生图请求。
  await db.upsertTask(task);
  waiting.push(task);
  notifyChange();
  ensureWorker();
  return task;
}

/**
 * 批量入队
 */
export async function enqueueBatch(requests, provider) {
  const tasks = [];
  for (const request of requests) tasks.push(await enqueueTask(request, provider));
  return tasks;
}

// ── Worker ──

function ensureWorker() {
  if (workerActive || waiting.length === 0) return;
  workerActive = true;
  void runWorker();
}

async function runWorker() {
  while (waiting.length > 0) {
    const task = waiting.shift();
    const controller = new AbortController();
    activeController = controller;
    task.status = 'running';
    task.updated_at = new Date().toISOString();
    task.started_at = task.updated_at;
    running.push(task);
    notifyChange();

    try {
      await db.upsertTask(task);
      controller.signal.throwIfAborted();
      const provider = await loadProvider(task.provider_id);
      if (!provider) throw new Error(`找不到 API 配置: ${task.provider_id}`);

      const request = {
        prompt: task.prompt,
        ratio: task.params?.ratio || '1:1',
        resolution: task.params?.resolution || '1K',
        count: task.params?.count || 1,
        output_format: task.params?.output_format || 'png',
        quality: task.params?.quality || '',
        background: task.params?.background || '',
        size: task.params?.size || '',
        reference_paths: task.reference_paths || [],
      };

      const results = await executeGeneration(
        { ...provider, imageModel: task.model || provider.imageModel },
        request,
        controller.signal
      );
      controller.signal.throwIfAborted();
      if (!results?.length) throw new Error('生图服务未返回图像数据');

      // 保存生成的图片
      const outputs = [];
      const now = new Date();
      const timestamp = `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, '0')}${String(now.getDate()).padStart(2, '0')}-${String(now.getHours()).padStart(2, '0')}${String(now.getMinutes()).padStart(2, '0')}${String(now.getSeconds()).padStart(2, '0')}`;

      for (let i = 0; i < results.length; i++) {
        const result = results[i];
        const blob = new Blob([result.bytes], { type: `image/${result.output_format || 'png'}` });
        const fileName = `${timestamp}-${task.id}-${String(i + 1).padStart(2, '0')}.${result.output_format || 'png'}`;

        // 把图片写入 ~/.image-forge/tasks/<YYYY-MM-DD>/<fileName>（本地开发）；
        // 配了 VITE_BLOB_READ_WRITE_TOKEN 则上传到 Vercel Blob。
        // 任何分支都不再把图片字节写进 localStorage / IndexedDB。
        const datePath = `tasks/${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
        controller.signal.throwIfAborted();
        const imageUrl = await uploadImage(fileName, blob, datePath, controller.signal);
        controller.signal.throwIfAborted();

        outputs.push({
          path: imageUrl,
          file_name: fileName,
          mime_type: `image/${result.output_format || 'png'}`,
          output_format: result.output_format || 'png',
          size: result.size || '',
          background: result.background || '',
          quality: result.quality || '',
          revised_prompt: result.revised_prompt || '',
          usage: result.usage || null,
        });
      }

      task.status = 'completed';
      task.completed_at = new Date().toISOString();
      task.library_date = task.completed_at.slice(0, 10);
      task.outputs = outputs;
      task.usage = results[0]?.usage || null;
    } catch (error) {
      if (controller.signal.aborted) {
        task.status = 'cancelled';
        task.error = '';
      } else {
        task.status = 'failed';
        task.error = error.message || String(error);
        console.error('生图失败:', task.id, error);
      }
    }

    task.updated_at = new Date().toISOString();
    task.completed_at = task.updated_at;
    // 会话摘要与 UI 读取终态前，保证任务已落盘。
    try {
      await db.upsertTask(task);
      await recordAgentTaskResult(task);
    } catch (error) {
      task.error = `任务记录保存失败：${error.message || error}`;
      console.error(task.error);
    }
    activeController = null;
    running = running.filter((t) => t.id !== task.id);
    recent.push(task);
    notifyChange();
  }

  workerActive = false;
  notifyChange();
}

// 任务终态后回写 Agent 会话；重试完成时更新同一条 task_result，避免重复或过期摘要。
async function recordAgentTaskResult(task) {
  const sessionId = task.agent_session_id || '';
  const groupId = task.task_group_id || '';
  if (!sessionId || !groupId) return;
  try {
    const all = await db.getAllTasks();
    const groupTasks = all.filter((t) => t.task_group_id === groupId);
    const statuses = groupTasks.map((t) => t.status || '');
    if (!statuses.length) return;
    const groupStatus = taskGroupStatus(groupTasks);

    await localStore.updateSession(sessionId, (session) => {
      if (!session) return null;
      let changed = false;
      for (const msg of session.messages || []) {
        if (msg.taskGroup?.id === groupId && msg.taskGroup.status !== groupStatus) {
          msg.taskGroup.status = groupStatus;
          changed = true;
        }
      }
      const terminal = ['completed', 'failed', 'cancelled'].includes(groupStatus);
      const previousResult = (session.messages || []).find(
        (m) => m.status === 'task_result' && String(m.content || '').includes(groupId)
      );
      if (terminal) {
        const succeeded = statuses.filter((s) => s === 'completed').length;
        const failed = statuses.filter((s) => s === 'failed').length;
        const outputs = groupTasks
          .flatMap((t) => (t.outputs || []).map((o) => o.path))
          .filter(Boolean);
        const content =
          groupStatus === 'completed'
            ? `[taskGroupId=${groupId}] 绘图任务组已完成，共 ${outputs.length || statuses.length} 张${
                outputs.length ? `：${outputs.join('、')}` : ''
              }`
            : groupStatus === 'cancelled'
              ? `[taskGroupId=${groupId}] 绘图任务组已取消，已完成 ${outputs.length} 张。`
              : `[taskGroupId=${groupId}] 绘图任务组未全部成功：成功 ${succeeded} 张，失败 ${failed} 张。`;
        if (previousResult) {
          previousResult.content = content;
        } else {
          session.messages = [
            ...(session.messages || []),
            {
              id: `web-msg-${groupId}-result`,
              role: 'tool',
              status: 'task_result',
              content,
              createdAt: new Date().toISOString(),
            },
          ];
        }
        changed = true;
      }
      if (changed) {
        session.updatedAt = new Date().toISOString();
        return session;
      }
      return null;
    });
  } catch {
    // 会话回写失败不影响生图流程
  }
}

// ── 取消 ──

export async function cancelTask(taskId) {
  // 如果任务在 waiting 中，直接标记取消
  const idx = waiting.findIndex((t) => t.id === taskId);
  if (idx >= 0) {
    const task = waiting.splice(idx, 1)[0];
    task.status = 'cancelled';
    task.updated_at = new Date().toISOString();
    task.completed_at = task.updated_at;
    await db.upsertTask(task);
    recent.push(task);
    await recordAgentTaskResult(task);
    notifyChange();
    return;
  }
  const task = running.find((t) => t.id === taskId && t.status === 'running');
  if (task) {
    task.status = 'cancelling';
    activeController?.abort();
    notifyChange();
  }
}

export async function cancelTaskGroup(taskGroupId) {
  const tasks = [...waiting, ...running].filter((t) => t.task_group_id === taskGroupId);
  // 一次标记整组，避免等待落盘时 worker 开始执行组内下一项。
  await Promise.all(tasks.map((task) => cancelTask(task.id)));
  if (tasks.length) await recordAgentTaskResult(tasks[0]);
}

// ── 重试 ──

export async function retryTask(taskId) {
  if (retrying.has(taskId) || [...waiting, ...running].some((t) => t.id === taskId)) return null;
  retrying.add(taskId);
  try {
    const source = recent.find((t) => t.id === taskId) || (await db.getTask(taskId));
    if (!source || !['failed', 'cancelled'].includes(source.status)) return null;
    const task = {
      ...source,
      status: 'queued',
      updated_at: new Date().toISOString(),
      started_at: '',
      completed_at: '',
      error: '',
      outputs: [],
      usage: null,
    };
    await db.upsertTask(task);
    waiting.push(task);
    recent = recent.filter((t) => t.id !== taskId);
    notifyChange();
    ensureWorker();
    return task;
  } finally {
    retrying.delete(taskId);
  }
}

export async function retryTaskGroup(taskGroupId) {
  const all = [...recent, ...(await db.getAllTasks())];
  const ids = new Set();
  for (const task of all) {
    if (task.task_group_id === taskGroupId && ['failed', 'cancelled'].includes(task.status)) {
      ids.add(task.id);
    }
  }
  for (const id of ids) await retryTask(id);
}

// ── 辅助 ──

async function loadProvider(providerId) {
  const settings = (await localStore.readSettings()) || {};
  return (settings.providers || []).find((p) => p.id === providerId) || null;
}

// 恢复：应用启动时恢复 queued / running，取消中的任务保持取消。
// 共享 SQLite 里也有桌面版的任务，只有 web-task- 前缀的归本浏览器恢复执行。
export function recoverTasks() {
  if (!recovery) {
    recovery = restoreTasks().finally(() => {
      recovery = null;
      ensureWorker();
      notifyChange();
    });
  }
  return recovery;
}

async function restoreTasks() {
  const all = await db.getAllTasks();
  const knownIds = new Set([...waiting, ...running, ...recent].map((t) => t.id));
  recent = all.filter((t) => ['completed', 'failed', 'cancelled'].includes(t.status)).slice(0, 50);
  for (const task of [...all].reverse()) {
    if (knownIds.has(task.id) || !String(task.id || '').startsWith('web-task-')) continue;
    if (!['queued', 'running', 'cancelling'].includes(task.status)) continue;
    task.status = task.status === 'cancelling' ? 'cancelled' : 'queued';
    task.updated_at = new Date().toISOString();
    await db.upsertTask(task);
    if (task.status === 'cancelled') {
      recent.push(task);
      await recordAgentTaskResult(task);
    } else {
      waiting.push(task);
    }
  }
}
