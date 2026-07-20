import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { availableMonitors, getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";

const tauri = window.__TAURI_INTERNALS__;
const WINDOW_STATE_KEY = "image-forge-window-state";

export function invoke(command, args = {}) {
  if (!tauri) {
    return Promise.reject(new Error("请在 Tauri 桌面窗口中运行"));
  }
  return tauri.invoke(command, args);
}

export function convertFileSrc(path) {
  return tauri?.convertFileSrc(path) ?? path;
}

export function openDialog(options) {
  return invoke("plugin:dialog|open", { options });
}

export function saveDialog(options) {
  return invoke("plugin:dialog|save", { options });
}

export function listenDragDrop(handler) {
  return getCurrentWebview().onDragDropEvent(handler);
}

export function listenEvent(event, handler) {
  return listen(event, handler);
}

export async function restoreWindowState() {
  if (!tauri) return;
  let state;
  try {
    state = JSON.parse(localStorage.getItem(WINDOW_STATE_KEY) || "null");
  } catch {
    localStorage.removeItem(WINDOW_STATE_KEY);
    return;
  }
  if (!state) return;
  const appWindow = getCurrentWindow();
  const scaleFactor = await appWindow.scaleFactor();
  const monitors = await availableMonitors();
  const monitor = monitors.find(({ workArea }) =>
    state.x >= workArea.position.x
    && state.y >= workArea.position.y
    && state.x < workArea.position.x + workArea.size.width
    && state.y < workArea.position.y + workArea.size.height
  ) || monitors[0];
  const area = monitor?.workArea;
  const { width, height } = logicalWindowSize(state, scaleFactor, area?.size);
  await appWindow.setSize(new LogicalSize(width, height));
  if (area) {
    const physicalWidth = width * scaleFactor;
    const physicalHeight = height * scaleFactor;
    const x = Math.max(area.position.x, Math.min(numberOr(state.x, area.position.x), area.position.x + area.size.width - physicalWidth));
    const y = Math.max(area.position.y, Math.min(numberOr(state.y, area.position.y), area.position.y + area.size.height - physicalHeight));
    await appWindow.setPosition(new PhysicalPosition(x, y));
  }
}

function numberOr(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

export function logicalWindowSize(state, scaleFactor, physicalWorkArea) {
  const unitScale = state?.unit === "logical" ? 1 : scaleFactor;
  const savedWidth = numberOr(state?.width, 1360) / unitScale;
  const savedHeight = numberOr(state?.height, 930) / unitScale;
  const maxWidth = physicalWorkArea ? physicalWorkArea.width / scaleFactor : Infinity;
  const maxHeight = physicalWorkArea ? physicalWorkArea.height / scaleFactor : Infinity;
  return {
    width: Math.max(1200, Math.min(savedWidth, maxWidth)),
    height: Math.max(800, Math.min(savedHeight, maxHeight)),
  };
}

export async function listenWindowState() {
  if (!tauri) return () => {};
  const appWindow = getCurrentWindow();
  let timer = 0;
  const save = async () => {
    const [position, size, scaleFactor] = await Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      appWindow.scaleFactor(),
    ]);
    const logicalSize = size.toLogical(scaleFactor);
    localStorage.setItem(WINDOW_STATE_KEY, JSON.stringify({
      x: position.x,
      y: position.y,
      width: logicalSize.width,
      height: logicalSize.height,
      unit: "logical",
    }));
  };
  const schedule = () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void save().catch(() => {}), 250);
  };
  const [unlistenMoved, unlistenResized] = await Promise.all([
    appWindow.onMoved(schedule),
    appWindow.onResized(schedule),
  ]);
  return () => {
    window.clearTimeout(timer);
    unlistenMoved();
    unlistenResized();
    void save().catch(() => {});
  };
}
