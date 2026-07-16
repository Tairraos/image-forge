import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";

const tauri = window.__TAURI_INTERNALS__;

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
