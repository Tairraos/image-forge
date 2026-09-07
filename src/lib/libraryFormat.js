// 图片库相关的任务与日期格式化工具，供独立图片库与 agent 内嵌图片库共用。
export function taskSource(task) {
  if (task.origin === 'agent-direct') return 'direct';
  return task.origin === 'agent' ||
    task.agentSessionId ||
    task.agent_session_id ||
    task.taskGroupId ||
    task.task_group_id
    ? 'agent'
    : 'drawing';
}

export function taskGroupStatus(tasks, fallback = 'missing') {
  const statuses = tasks.map((task) => task.status).filter(Boolean);
  if (!statuses.length) return fallback;
  if (statuses.includes('cancelling')) return 'cancelling';
  if (statuses.includes('running')) return 'running';
  if (statuses.includes('queued')) return 'queued';
  if (statuses.every((status) => status === 'completed')) return 'completed';
  if (statuses.includes('failed')) return 'failed';
  if (statuses.includes('cancelled')) return 'cancelled';
  return 'missing';
}

export function agentMessagesForDisplay(messages, history) {
  const announcedGroups = new Set(messages.map((message) => message.taskGroup?.id).filter(Boolean));
  return messages.flatMap((message) => {
    const resultGroupId =
      message.status === 'task_result'
        ? /^\[taskGroupId=([^\]]+)\]/.exec(message.content || '')?.[1]
        : '';
    if (resultGroupId && announcedGroups.has(resultGroupId)) return [];
    // 旧版直接绘画只留下结果摘要时，也用任务卡片展示已保存的图片。
    const group =
      message.taskGroup || (resultGroupId ? { id: resultGroupId, status: 'missing' } : null);
    if (!group?.id) return [message];
    const taskIds = new Set(group.taskIds || []);
    const tasks = history.filter(
      (task) => taskIds.has(task.id) || (task.taskGroupId || task.task_group_id) === group.id
    );
    return [
      {
        ...message,
        content: resultGroupId ? '' : message.content,
        taskGroup: {
          ...group,
          taskIds: tasks.length ? tasks.map((task) => task.id) : group.taskIds || [],
          status: taskGroupStatus(tasks, group.status),
          images: tasks.flatMap((task) =>
            (task.outputs || []).map((output) => previewItem(task, output))
          ),
          errors: [...new Set(tasks.map((task) => task.error).filter(Boolean))],
          progress: {
            total: tasks.length || taskIds.size,
            completed: tasks.filter((task) => task.status === 'completed').length,
          },
        },
      },
    ];
  });
}

const TASK_SOURCE_LABELS = {
  agent: 'Agent',
  direct: '直接绘画',
  drawing: '绘画',
};

export function taskSourceLabel(task) {
  return TASK_SOURCE_LABELS[taskSource(task)] || '绘画';
}

export function taskSourceOptions() {
  return [
    { label: '全部来源', value: 'all' },
    { label: 'Agent 对话', value: 'agent' },
    { label: '直接绘画', value: 'direct' },
    { label: '绘画', value: 'drawing' },
  ];
}

// 桌面端任务记录是 camelCase（referencePaths），Web 端 IndexedDB 是 snake_case（reference_paths）
export function taskReferencePaths(task) {
  return task?.referencePaths || task?.reference_paths || [];
}

export function taskTime(task) {
  return (
    task.completedAt ||
    task.completed_at ||
    task.updatedAt ||
    task.updated_at ||
    task.createdAt ||
    task.created_at ||
    ''
  );
}

export function previewItem(task, output) {
  return {
    ...output,
    task,
    title: task.prompt || output.fileName || output.file_name || '生成图片',
    meta: [taskSourceLabel(task), output.size || task.params?.size, task.model]
      .filter(Boolean)
      .join(' · '),
    prompt: task.prompt || '',
    revisedPrompt: output.revisedPrompt || output.revised_prompt || '',
    referencePaths: taskReferencePaths(task),
    time: taskTime(task),
    model: task.model || task.providerName || task.provider_name || '',
    size: output.size || task.params?.size || '',
  };
}

// 参考图进大图查看器时与主图共享同一任务上下文（提示词、操作按钮），
// 改写提示词取首个输出里的值（参考图本身没有改写提示词）。
export function referencePreviewItem(task, path, index, total) {
  const firstOutput = (task.outputs || [])[0] || {};
  return {
    path,
    task,
    title: `参考图 ${index + 1}/${total}`,
    meta: [taskSourceLabel(task), task.model || task.providerName || task.provider_name || '']
      .filter(Boolean)
      .join(' · '),
    prompt: task.prompt || '',
    revisedPrompt: firstOutput.revisedPrompt || firstOutput.revised_prompt || '',
    referencePaths: taskReferencePaths(task),
    time: taskTime(task),
    model: task.model || task.providerName || task.provider_name || '',
    size: '',
  };
}

export function dateKey(value) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function monthKey(value) {
  return dateKey(value).slice(0, 7);
}

export function formatMonth(value) {
  const [year, month] = (/^\d{4}-\d{2}$/.test(value) ? value : monthKey(new Date())).split('-');
  return `${year} 年 ${Number(month)} 月`;
}

export function formatFullDate(value) {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    weekday: 'short',
  });
}

export function formatDayHeading(value) {
  const today = dateKey(new Date());
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  if (value === today) return '今天';
  if (value === dateKey(yesterday)) return '昨天';
  return new Date(`${value}T00:00:00`).toLocaleDateString('zh-CN', {
    month: 'long',
    day: 'numeric',
  });
}

export function formatTime(value) {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? ''
    : date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
}
