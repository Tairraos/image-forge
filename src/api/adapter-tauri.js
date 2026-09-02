// Tauri 桌面版适配器：所有 invoke 调用的薄封装。
// 每个函数直接对应 Rust 端的一个 Tauri 命令。
import { invoke as tauriInvoke } from '../tauri';

function invoke(cmd, args = {}) {
  return tauriInvoke(cmd, args);
}

// ── 应用状态 ──
export const loadAppState = () => invoke('load_app_state');
export const aboutInfo = () => invoke('about_info');
export const runtimeLogs = () => invoke('runtime_logs');

// ── 设置 ──
export const saveSettings = (settings) => invoke('save_settings', { settings });

// ── Agent 会话 ──
export const listAgentSessions = () => invoke('list_agent_sessions');
export const createAgentSession = (providerId) => invoke('create_agent_session', { providerId });
export const getAgentSession = (sessionId) => invoke('get_agent_session', { sessionId });
export const deleteAgentSession = (sessionId) => invoke('delete_agent_session', { sessionId });
export const renameAgentSession = (sessionId, title) =>
  invoke('rename_agent_session', { sessionId, title });
export const sendAgentMessage = (sessionId, providerId, content, attachments) =>
  invoke('send_agent_message', { sessionId, providerId, content, attachments });
export const createAgentDirectImageTask = (sessionId, content, attachments, plan) =>
  invoke('create_agent_direct_image_task', { sessionId, content, attachments, plan });
export const cancelAgentTurn = (sessionId) => invoke('cancel_agent_turn', { sessionId });
export const fillPromptTemplate = (sessionId, providerId, template) =>
  invoke('fill_prompt_template', { sessionId, providerId, template });

// ── 任务组 ──
export const cancelAgentTaskGroup = (taskGroupId) =>
  invoke('cancel_agent_task_group', { taskGroupId });
export const retryAgentTaskGroup = (taskGroupId) =>
  invoke('retry_agent_task_group', { taskGroupId });
export const redrawTask = (taskId) => invoke('redraw_task', { taskId });
export const getTaskStatus = (taskGroupId, taskId) =>
  invoke('get_task_status', { taskGroupId, taskId });

// ── 队列 ──
export const queueSnapshot = () => invoke('queue_snapshot');

// ── 历史 / 图片库 ──
export const deleteTask = (taskId) => invoke('delete_task', { taskId });
export const agentLibrary = (month, query) => invoke('agent_library', { month, query });

// ── 参考图 ──
export const referenceFromPath = (path) => invoke('reference_from_path', { path });
export const referenceFromClipboard = () => invoke('reference_from_clipboard');

// ── 输出 ──
export const downloadOutput = (path) => invoke('download_output', { path });
export const revealPath = (path) => invoke('reveal_path', { path });

// ── 模板 ──
export const saveTemplate = (template) => invoke('save_template', { template });
export const exportTemplates = (destination) => invoke('export_templates', { destination });
export const importTemplates = (archivePath) => invoke('import_templates', { archivePath });
export const deleteTemplate = (templateId) => invoke('delete_template', { templateId });
export const moveTemplate = (templateId, targetTemplateId) =>
  invoke('move_template', { templateId, targetTemplateId });

// ── 数据导出/导入 ──
export const exportDataBundle = (categories) => invoke('export_data_bundle', { categories });
export const importDataBundle = (file) =>
  invoke('import_data_bundle', { filePath: file.path || file });

// ── 清理 ──
export const scanCleanupCandidates = () => invoke('scan_cleanup_candidates');
export const cleanupDataFiles = () => invoke('cleanup_data_files');

// ── 工具 ──
export const readClipboardText = () => invoke('read_clipboard_text');
export const listProviderModels = (provider) => invoke('list_provider_models', { provider });

// Web-only helpers（Tauri 端为 no-op）
export const onAgentEvent = () => () => {};
export const onQueueChange = () => () => {};
