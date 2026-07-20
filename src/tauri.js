import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { availableMonitors, getCurrentWindow, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";

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
  const monitors = await availableMonitors();
  const monitor = monitors.find(({ workArea }) =>
    state.x >= workArea.position.x
    && state.y >= workArea.position.y
    && state.x < workArea.position.x + workArea.size.width
    && state.y < workArea.position.y + workArea.size.height
  ) || monitors[0];
  const area = monitor?.workArea;
  const width = Math.max(1200, Math.min(numberOr(state.width, 1200), area?.size.width || Infinity));
  const height = Math.max(800, Math.min(numberOr(state.height, 800), area?.size.height || Infinity));
  await appWindow.setSize(new PhysicalSize(width, height));
  if (area) {
    const x = Math.max(area.position.x, Math.min(numberOr(state.x, area.position.x), area.position.x + area.size.width - width));
    const y = Math.max(area.position.y, Math.min(numberOr(state.y, area.position.y), area.position.y + area.size.height - height));
    await appWindow.setPosition(new PhysicalPosition(x, y));
  }
}

function numberOr(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

export async function listenWindowState() {
  if (!tauri) return () => {};
  const appWindow = getCurrentWindow();
  let timer = 0;
  const save = async () => {
    const [position, size] = await Promise.all([appWindow.outerPosition(), appWindow.outerSize()]);
    localStorage.setItem(WINDOW_STATE_KEY, JSON.stringify({ x: position.x, y: position.y, width: size.width, height: size.height }));
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
