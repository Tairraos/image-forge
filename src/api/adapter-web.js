// Web 版适配器（阶段 1 占位，后续阶段逐步实现）。
// 所有函数签名与 adapter-tauri.js 保持一致。

const noop = () => {
  throw new Error("Web 适配器尚未实现");
};

export const loadAppState = noop;
export const aboutInfo = noop;
export const runtimeLogs = noop;
export const saveSettings = noop;
export const listAgentSessions = noop;
export const createAgentSession = noop;
export const getAgentSession = noop;
export const deleteAgentSession = noop;
export const renameAgentSession = noop;
export const sendAgentMessage = noop;
export const createAgentDirectImageTask = noop;
export const cancelAgentTurn = noop;
export const cancelAgentTaskGroup = noop;
export const retryAgentTaskGroup = noop;
export const getTaskStatus = noop;
export const queueSnapshot = noop;
export const deleteTask = noop;
export const agentLibrary = noop;
export const referenceFromPath = noop;
export const referenceFromClipboard = noop;
export const downloadOutput = noop;
export const revealPath = noop;
export const saveTemplate = noop;
export const exportTemplates = noop;
export const importTemplates = noop;
export const deleteTemplate = noop;
export const moveTemplate = noop;
export const scanCleanupCandidates = noop;
export const cleanupDataFiles = noop;
export const readClipboardText = noop;
export const listProviderModels = noop;