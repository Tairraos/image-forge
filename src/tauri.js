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
