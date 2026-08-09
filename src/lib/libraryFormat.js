// 图片库相关的任务与日期格式化工具，供独立图片库与 agent 内嵌图片库共用。
export function taskSource(task) {
  return task.origin === "agent" || task.agentSessionId || task.taskGroupId ? "agent" : "drawing";
}

export function taskSourceLabel(task) {
  return taskSource(task) === "agent" ? "Agent" : "绘画";
}

export function taskTime(task) {
  return task.completedAt || task.updatedAt || task.createdAt || "";
}

export function previewItem(task, output) {
  return {
    ...output,
    title: task.prompt || output.fileName || "生成图片",
    meta: [taskSourceLabel(task), output.size || task.params?.size, task.model].filter(Boolean).join(" · "),
  };
}

export function dateKey(value) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function monthKey(value) {
  return dateKey(value).slice(0, 7);
}

export function formatMonth(value) {
  const [year, month] = (/^\d{4}-\d{2}$/.test(value) ? value : monthKey(new Date())).split("-");
  return `${year} 年 ${Number(month)} 月`;
}

export function formatFullDate(value) {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("zh-CN", { year: "numeric", month: "long", day: "numeric", weekday: "short" });
}

export function formatDayHeading(value) {
  const today = dateKey(new Date());
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  if (value === today) return "今天";
  if (value === dateKey(yesterday)) return "昨天";
  return new Date(`${value}T00:00:00`).toLocaleDateString("zh-CN", { month: "long", day: "numeric" });
}

export function formatTime(value) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}
