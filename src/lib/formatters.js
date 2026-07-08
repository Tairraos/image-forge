import { convertFileSrc } from "../tauri";

export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

export function statusType(status) {
  if (status === "completed") return "success";
  if (status === "failed" || status === "cancelled") return "error";
  if (status === "running" || status === "cancelling") return "warning";
  return "info";
}

export function statusLabel(status) {
  return {
    queued: "排队",
    running: "运行",
    completed: "完成",
    failed: "失败",
    cancelled: "取消",
    cancelling: "取消中",
  }[status] || status;
}

export function shortId(id) {
  return String(id || "").slice(0, 8);
}

export function fileName(path) {
  return String(path || "").split(/[\\/]/).pop() || "image";
}

export function fileUrl(path) {
  return `${convertFileSrc(path)}?v=${encodeURIComponent(path)}`;
}
