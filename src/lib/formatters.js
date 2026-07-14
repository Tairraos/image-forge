import { convertFileSrc } from "../tauri";

export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

export function fileName(path) {
  return String(path || "").split(/[\\/]/).pop() || "image";
}

export function fileUrl(path) {
  return `${convertFileSrc(path)}?v=${encodeURIComponent(path)}`;
}

export function statusLabel(status) {
  return {
    queued: "排队中",
    running: "生成中",
    cancelling: "取消中",
    completed: "完成",
    failed: "失败",
    cancelled: "已取消",
  }[status] || "未知";
}
