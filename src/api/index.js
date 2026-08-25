// 统一 API 入口：根据运行时环境自动选择 Tauri 或 Web 适配器。
// 所有函数签名与 adapter-tauri.js / adapter-web.js 保持一致。

const isTauri = Boolean(window.__TAURI_INTERNALS__);

const adapter = isTauri ? await import('./adapter-tauri.js') : await import('./adapter-web.js');

export const {
  loadAppState,
  aboutInfo,
  runtimeLogs,
  saveSettings,
  listAgentSessions,
  createAgentSession,
  getAgentSession,
  deleteAgentSession,
  renameAgentSession,
  sendAgentMessage,
  createAgentDirectImageTask,
  cancelAgentTurn,
  cancelAgentTaskGroup,
  retryAgentTaskGroup,
  getTaskStatus,
  queueSnapshot,
  deleteTask,
  agentLibrary,
  referenceFromPath,
  referenceFromClipboard,
  downloadOutput,
  revealPath,
  saveTemplate,
  exportTemplates,
  importTemplates,
  exportDataBundle,
  importDataBundle,
  deleteTemplate,
  moveTemplate,
  scanCleanupCandidates,
  cleanupDataFiles,
  readClipboardText,
  listProviderModels,
  onAgentEvent,
  onQueueChange,
} = adapter;
