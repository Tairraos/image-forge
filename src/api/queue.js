// Web 版队列调度系统，对应桌面版 queue.rs。
// 单 worker 顺序处理任务，支持并发限制、取消和重试。

import * as db from './db.js';
import { executeGeneration } from './providers.js';
import { uploadImage } from './blob.js';

// ── 队列状态 ──

let waiting = [];
let running = [];
let recent = [];
let workerActive = false;
let cancelSet = new Set(); // 被取消的任务 ID 集合

// 事件回调（供 UI 层监听队列变化）
let onChangeCallback = null;

export function onQueueChange(callback) {
  onChangeCallback = callback;
}

function notifyChange() {
  if (onChangeCallback) {
    onChangeCallback(snapshot());
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
 * @returns {Object} TaskRecord
 */
export function enqueueTask(request, provider) {
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
  // 持久化到 IndexedDB
  db.upsertTask(task).catch((e) => console.warn('队列任务持久化失败:', e));
  waiting.push(task);
  notifyChange();
  ensureWorker();
  return task;
}

/**
 * 批量入队
 */
export function enqueueBatch(requests, provider) {
  return requests.map((req) => enqueueTask(req, provider));
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
    // 检查是否已被取消
    if (cancelSet.has(task.id)) {
      cancelSet.delete(task.id);
      task.status = 'cancelled';
      task.updated_at = new Date().toISOString();
      db.upsertTask(task).catch(() => {});
      recent.push(task);
      notifyChange();
      continue;
    }

    task.status = 'running';
    task.updated_at = new Date().toISOString();
    running.push(task);
    notifyChange();

    try {
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

      const results = await executeGeneration(provider, request);

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
        const imageUrl = await uploadImage(fileName, blob, datePath);

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
      task.status = 'failed';
      task.error = error.message || String(error);
      console.error('生图失败:', task.id, error);
    }

    task.updated_at = new Date().toISOString();
    running = running.filter((t) => t.id !== task.id);
    recent.push(task);

    // 持久化
    db.upsertTask(task).catch(() => {});

    notifyChange();
  }

  workerActive = false;
  notifyChange();
}

// ── 取消 ──

export function cancelTask(taskId) {
  cancelSet.add(taskId);
  // 如果任务在 waiting 中，直接标记取消
  const idx = waiting.findIndex((t) => t.id === taskId);
  if (idx >= 0) {
    const task = waiting.splice(idx, 1)[0];
    task.status = 'cancelled';
    task.updated_at = new Date().toISOString();
    db.upsertTask(task).catch(() => {});
    recent.push(task);
    cancelSet.delete(taskId);
    notifyChange();
  }
}

export function cancelTaskGroup(taskGroupId) {
  // 取消任务组中所有排队/运行中的任务
  for (const task of [...waiting, ...running]) {
    if (task.task_group_id === taskGroupId) {
      cancelTask(task.id);
    }
  }
}

// ── 重试 ──

export function retryTask(taskId) {
  const all = [...recent, ...waiting, ...running];
  const task = all.find((t) => t.id === taskId);
  if (!task || !['failed', 'cancelled'].includes(task.status)) return null;

  task.status = 'queued';
  task.updated_at = new Date().toISOString();
  task.error = '';
  waiting.push(task);
  recent = recent.filter((t) => t.id !== taskId);
  notifyChange();
  ensureWorker();
  return task;
}

export function retryTaskGroup(taskGroupId) {
  const all = [...recent, ...waiting, ...running];
  for (const task of all) {
    if (task.task_group_id === taskGroupId && ['failed', 'cancelled'].includes(task.status)) {
      retryTask(task.id);
    }
  }
}

// ── 辅助 ──

async function loadProvider(providerId) {
  const raw = localStorage.getItem('if_settings');
  if (!raw) return null;
  try {
    const settings = JSON.parse(raw);
    return (settings.providers || []).find((p) => p.id === providerId) || null;
  } catch {
    return null;
  }
}

// 恢复：应用启动时把遗留的 running 任务恢复为 queued
export async function recoverTasks() {
  const all = await db.getAllTasks();
  for (const task of all) {
    if (task.status === 'running') {
      task.status = 'queued';
      task.updated_at = new Date().toISOString();
      db.upsertTask(task).catch(() => {});
      waiting.push(task);
    }
  }
  if (waiting.length > 0) {
    ensureWorker();
  }
  // 加载最近完成的任务到 recent
  const completed = all.filter(
    (t) => t.status === 'completed' || t.status === 'failed' || t.status === 'cancelled'
  );
  recent = completed.slice(0, 50);
  notifyChange();
}
