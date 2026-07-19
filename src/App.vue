<template>
  <n-config-provider :theme-overrides="themeOverrides" component-size="small">
    <n-global-style />
    <AppShell
      :mode="workspaceMode"
      @update:mode="workspaceMode = $event"
      @show-api="showApiDialog = true"
      @show-template-manager="showTemplateManagerDialog = true"
      @show-skill-manager="showSkillManagerDialog = true"
      @show-about="openAbout"
    >

      <DrawingWorkspace
        v-show="workspaceMode === 'drawing'"
        :filtered-history="filteredHistory"
        :selected-task-id="selectedTaskId"
        :history-query="historyQuery"
        :history-scope="historyScope"
        :history-scroll-request="historyScrollRequest"
        :selected-task="selectedTask"
        :current-outputs="currentOutputs"
        :form="form"
        :image-provider-options="imageProviderOptions"
        :chat-provider-options="chatProviderOptions"
        :references="references"
        :submitting="submitting"
        :reference-drag-active="referenceDragActive"
        :workspace-style="workspaceStyle"
        @select-task="selectedTaskId = $event"
        @update:history-query="historyQuery = $event"
        @update:history-scope="historyScope = $event"
        @reuse="reuseTask"
        @refresh-task="refreshTask"
        @retry="retryTask"
        @delete="deleteTask"
        @copy-output="copyOutput"
        @download-output="downloadOutput"
        @reveal-output="reveal($event.path)"
        @start-panel-resize="startPanelResize"
        @show-detail="showTaskDetail = true"
        @model-template="newTemplateFromTask"
        @submit="submitTask"
        @show-template="showTemplateReferenceDialog = true"
        @clear-prompt="clearPrompt"
        @prompt-focus="capturePromptCursor"
        @prompt-cursor="capturePromptCursor"
        @prompt-paste="handlePromptPaste"
        @paste-reference="pasteWorkbenchReferenceImage"
        @add-reference="addReferenceImages"
        @remove-reference="removeReference(references, $event)"
        @reference-drag-over="referenceDragActive = true"
        @reference-drag-leave="referenceDragActive = false"
        @drop-reference="handleReferenceDropEvent"
      />

      <AgentWorkspace
        v-show="workspaceMode === 'agent'"
        :sessions="agentSessions"
        :current-session="currentAgentSession"
        :messages="currentAgentMessages"
        :provider-options="chatProviderOptions"
        :provider-id="agentProviderId"
        :image-provider-id="activeProvider?.id || ''"
        :busy="agentBusy"
        :stream-text="agentStreamText"
        :tool-status-text="agentToolStatus"
        :answers="agentAnswers"
        :attachments="agentAttachments"
        @create="createAgentConversation"
        @select="selectAgentConversation"
        @send="sendAgentConversationMessage"
        @stop="stopAgentConversation"
        @add-reference="addAgentReferenceImages"
        @remove-attachment="removeAgentAttachment"
        @update:provider-id="agentProviderId = $event"
        @open-task-group="openAgentTaskGroup"
        @cancel-task-group="cancelAgentTaskGroup"
        @retry-task-group="retryAgentTaskGroup"
        @retry="retryAgentMessage"
        @paste-reference="pasteAgentReferenceImage"
        @drop-reference="addAgentReferencePaths"
        @update-answer="updateAgentAnswer"
        @answer-questions="answerAgentQuestions"
        @delete-session="deleteAgentConversation"
      />

      <template #footer>
        <footer class="status-bar">
          <span class="status-pill" :data-tone="statusTone">{{ statusText }}</span>
          <div class="status-summary">
            <span class="status-meta">当前 API：{{ activeProvider?.name || "未配置" }} · Images API</span>
            <span class="status-count">{{ queue.running.length }} 运行</span>
            <span class="status-count">{{ queue.waiting.length }} 排队</span>
            <span v-if="activeProvider && !activeProvider.apiKey" class="warn-text">API Key 未设置</span>
          </div>
        </footer>
      </template>

      <template #dialogs>
        <ApiSourceDialog
          v-model:show="showApiDialog"
          :settings="settings"
          @save="saveApiSettings"
        />

      <TemplateManagerDialog
        v-model:show="showTemplateManagerDialog"
        v-model:query="templateQuery"
        :templates="filteredTemplates"
        @view="viewTemplate"
        @edit="editTemplate"
        @delete="deletePromptTemplate"
        @create="newTemplate"
        @import="importPromptTemplates"
        @export="exportPromptTemplates"
        @move="movePromptTemplate"
        @show-effect="showTemplateEffect"
      />

      <TemplateReferenceDialog
        v-model:show="showTemplateReferenceDialog"
        v-model:query="templateReferenceQuery"
        :source-content="templateReferenceSourceContent"
        :generated-content="templateReferenceGeneratedContent"
        :templates="filteredReferenceTemplates"
        :selected-template-id="selectedReferenceTemplateId"
        :chat-provider-id="form.chatProviderId"
        :chat-provider-options="chatProviderOptions"
        :filled-ranges="templateFilledRanges"
        :filling="templateFilling"
        :references="templateReferenceReferences"
        :effect-image="templateReferenceEffectImage"
        @update:chat-provider-id="form.chatProviderId = $event"
        @update:source-content="updateTemplateReferenceSource"
        @update:generated-content="updateTemplateReferenceGenerated"
        @select-template="selectReferenceTemplate"
        @ai-fill="fillReferenceTemplate"
        @insert="insertReferenceTemplate"
        @add-reference="addTemplateCallReferenceImages"
        @paste-reference="pasteTemplateCallReferenceImage"
        @remove-reference="removeReference(templateReferenceReferences, $event)"
        @show-effect="showTemplateEffectByPreview(templateReferenceEffectImage)"
      />

      <SkillManagerDialog
        v-model:show="showSkillManagerDialog"
        v-model:query="skillQuery"
        :skills="filteredSkills"
        @create="newSkill"
        @view="viewSkill"
        @edit="editSkill"
        @delete="deleteSkill"
      />

      <SkillEditorDialog
        v-model:show="showSkillEditor"
        :skill="skillDraft"
        :mode="skillEditorMode"
        :fetching="skillFetching"
        @fetch="fetchSkillContent"
        @save="saveSkill"
        @drop-markdown="handleSkillMarkdownDrop"
      />

      <TemplateEditorDialog
        v-model:show="showTemplateEditor"
        :template="templateDraft"
        :mode="templateEditorMode"
        :references="templateDraftReferences"
        :effect-image="templateDraftEffectImage"
        :reference-drag-active="templateDraftDragActive"
        @save="savePromptTemplate"
        @add-reference="addTemplateDraftReferenceImages"
        @remove-reference="removeReference(templateDraftReferences, $event)"
        @add-effect-image="addTemplateDraftEffectImage"
        @paste-effect-image="pasteTemplateDraftEffectImage"
        @remove-effect-image="templateDraftEffectImage = null"
        @paste-reference="handleTemplateDraftPaste"
        @reference-drag-over="templateDraftDragActive = true"
        @reference-drag-leave="templateDraftDragActive = false"
        @drop-reference="handleTemplateDraftDropEvent"
        @update:show="templateDraftDragActive = false"
      />

      <EffectImageViewer
        v-model:show="effectViewer.show"
        :image-path="effectViewer.path"
        :title="effectViewer.title"
      />

      <TaskDetailDialog
        v-model:show="showTaskDetail"
        :task="selectedTask"
        @reuse="reuseTask"
      />

      <AboutDialog
        v-model:show="showAboutDialog"
        :info="aboutInfo"
        @show-logs="openRuntimeLogs"
        @cleanup="openCleanup"
      />

      <CleanupDialog
        v-model:show="showCleanupDialog"
        :candidates="cleanupCandidates"
        :loading="cleanupLoading"
        :confirming="cleanupConfirming"
        :error="cleanupError"
        @confirm="confirmCleanup"
      />

      <RuntimeLogDialog
        v-model:show="showRuntimeLogDialog"
        :logs="runtimeLogText"
      />

      <ConfirmDialog
        v-model:show="confirmation.visible"
        :title="confirmation.title"
        :message="confirmation.message"
        @confirm="resolveConfirmation(true)"
        @cancel="resolveConfirmation(false)"
      />

      <NoticeDialog
        v-model:show="notice.visible"
        :title="notice.title"
        :message="notice.message"
        :button-text="notice.buttonText"
        @close="resolveNotice"
      />
      </template>
    </AppShell>
  </n-config-provider>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import AgentWorkspace from "./components/AgentWorkspace.vue";
import AppShell from "./components/AppShell.vue";
import DrawingWorkspace from "./components/DrawingWorkspace.vue";
import AboutDialog from "./components/dialogs/AboutDialog.vue";
import CleanupDialog from "./components/dialogs/CleanupDialog.vue";
import ApiSourceDialog from "./components/dialogs/ApiSourceDialog.vue";
import ConfirmDialog from "./components/dialogs/ConfirmDialog.vue";
import EffectImageViewer from "./components/dialogs/EffectImageViewer.vue";
import NoticeDialog from "./components/dialogs/NoticeDialog.vue";
import RuntimeLogDialog from "./components/dialogs/RuntimeLogDialog.vue";
import SkillEditorDialog from "./components/dialogs/SkillEditorDialog.vue";
import SkillManagerDialog from "./components/dialogs/SkillManagerDialog.vue";
import TaskDetailDialog from "./components/dialogs/TaskDetailDialog.vue";
import TemplateEditorDialog from "./components/dialogs/TemplateEditorDialog.vue";
import TemplateManagerDialog from "./components/dialogs/TemplateManagerDialog.vue";
import TemplateReferenceDialog from "./components/dialogs/TemplateReferenceDialog.vue";
import { clamp, fileName, statusLabel } from "./lib/formatters";
import {
  deepClone,
  defaultSettings,
  emptySkill,
  emptyTemplate,
  normalizeSettingsForUi,
} from "./lib/models";
import { clipboardHasImage, extractClipboardFilePaths, extractDroppedFilePaths } from "./lib/referenceFiles";
import { installAutoHideScrollbars } from "./lib/scrollbarVisibility";
import {
  DEFAULT_PROMPT_MODE,
  DEFAULT_RATIO,
  orientationForRatio,
  sizeForPreset,
} from "./lib/options";
import { themeOverrides } from "./lib/theme";
import { invoke, listenDragDrop, listenEvent, listenWindowState, openDialog, restoreWindowState, saveDialog } from "./tauri";

const statusText = ref("启动中");
const statusTone = ref("busy");
const workspaceMode = ref(localStorage.getItem("image-forge-workspace-mode") || "drawing");
const agentSessions = ref([]);
const currentAgentSessionId = ref("");
const agentProviderId = ref("");
const agentBusy = ref(false);
const agentStreamText = ref("");
const agentToolStatus = ref("");
const agentAnswers = ref({});
const agentAttachments = ref([]);
const settings = ref(defaultSettings());
const history = ref([]);
const queue = reactive({ waiting: [], running: [], recent: [], workerActive: false, updatedAt: "" });
const templates = ref([]);
const skills = ref([]);
const references = ref([]);
const referenceDragActive = ref(false);
const templateDraftReferences = ref([]);
const templateDraftEffectImage = ref(null);
const templateDraftDragActive = ref(false);
const templateReferenceReferences = ref([]);
const templateReferenceEffectImage = ref(null);
const selectedTaskId = ref("");
const historyScrollRequest = ref(0);
const submitting = ref(false);
const historyQuery = ref("");
const historyScope = ref("today");
const todayKey = ref(localDateKey(new Date()));
const templateQuery = ref("");
const skillQuery = ref("");
const templateReferenceQuery = ref("");
const templateReferenceSourceContent = ref("");
const templateReferenceGeneratedContent = ref("");
const selectedReferenceTemplateId = ref("");
const templateFilledRanges = ref([]);
const templateFilling = ref(false);
const templateFillSessionId = ref("");
const skillFetching = ref(false);
const promptCursor = ref(0);

const showApiDialog = ref(false);
const showTemplateManagerDialog = ref(false);
const showTemplateReferenceDialog = ref(false);
const showTemplateEditor = ref(false);
const showSkillManagerDialog = ref(false);
const showSkillEditor = ref(false);
const showTaskDetail = ref(false);
const showAboutDialog = ref(false);
const showRuntimeLogDialog = ref(false);
const showCleanupDialog = ref(false);
const confirmation = reactive({
  visible: false,
  title: "请确认",
  message: "",
  resolve: null,
});
const notice = reactive({
  visible: false,
  title: "提示",
  message: "",
  buttonText: "确认",
  resolve: null,
});
const effectViewer = reactive({ show: false, path: "", title: "" });
const ACTIVE_QUEUE_POLL_INTERVAL = 5000;
const AGENT_TASK_GROUP_POLL_INTERVAL = 5000;

const templateDraft = reactive(emptyTemplate());
const templateEditorMode = ref("edit");
const skillDraft = reactive(emptySkill());
const skillEditorMode = ref("edit");
const aboutInfo = ref({ version: "", buildTime: "" });
const runtimeLogText = ref("");
const cleanupCandidates = ref([]);
const cleanupLoading = ref(false);
const cleanupConfirming = ref(false);
const cleanupError = ref("");

watch(workspaceMode, (mode) => localStorage.setItem("image-forge-workspace-mode", mode));

const form = reactive({
  providerId: "",
  chatProviderId: "",
  prompt: "",
  promptMode: DEFAULT_PROMPT_MODE,
  resolution: "4k",
  ratio: DEFAULT_RATIO,
  quality: "medium",
});

const panelSizes = reactive({
  queue: 310,
  composer: 420,
});

const workspaceStyle = computed(() => ({
  gridTemplateColumns: `${panelSizes.queue}px 10px minmax(0, 1fr) 10px ${panelSizes.composer}px`,
}));

let pollTimer = 0;
let todayRolloverTimer = 0;
let removeScrollbarVisibility = null;
let unlistenDragDrop = null;
let unlistenQueueUpdated = null;
let unlistenTemplateFill = null;
let unlistenAgentProgress = null;
let unlistenAgentTaskGroup = null;
let unlistenWindowState = null;
let queueRefreshInFlight = false;
let queueRefreshQueued = false;
let agentTaskGroupPollTimer = 0;
let agentTaskGroupRefreshInFlight = false;

const imageProviders = computed(() =>
  settings.value.providers.filter((provider) => provider.modelType !== "chat"),
);

const chatProviders = computed(() =>
  settings.value.providers.filter((provider) => provider.modelType === "chat"),
);

const imageProviderOptions = computed(() =>
  imageProviders.value.map((provider) => ({
    label: modelOptionLabel(provider),
    value: provider.id,
  })),
);

const chatProviderOptions = computed(() =>
  chatProviders.value.map((provider) => ({
    label: modelOptionLabel(provider),
    value: provider.id,
  })),
);

const activeProvider = computed(() =>
  imageProviders.value.find((provider) => provider.id === form.providerId)
  || imageProviders.value.find((provider) => provider.id === settings.value.activeImageProviderId)
  || imageProviders.value[0],
);

const currentAgentSession = computed(() =>
  agentSessions.value.find((session) => session.id === currentAgentSessionId.value) || null,
);

const currentAgentMessages = computed(() => currentAgentSession.value?.messages || []);

const historyTimeline = computed(() => {
  const byId = new Map();
  for (const task of [...history.value, ...queue.running, ...queue.waiting]) {
    if (task?.id) byId.set(task.id, task);
  }
  return Array.from(byId.values()).sort((left, right) =>
    taskTime(left).localeCompare(taskTime(right)),
  );
});

const filteredHistory = computed(() => {
  const query = historyQuery.value.trim().toLowerCase();
  const items = historyScope.value === "today"
    ? historyTimeline.value.filter((task) => localDateKey(taskTime(task)) === todayKey.value)
    : historyTimeline.value;
  if (!query) return items;
  return items.filter((task) =>
    [task.id, task.prompt, task.providerName, task.model]
      .filter(Boolean)
      .join(" ")
      .toLowerCase()
      .includes(query),
  );
});

const selectedTask = computed(() =>
  historyTimeline.value.find((task) => task.id === selectedTaskId.value)
  || null,
);

const currentOutputs = computed(() => selectedTask.value?.outputs || []);


const filteredTemplates = computed(() => {
  const query = templateQuery.value.trim().toLowerCase();
  if (!query) return templates.value;
  return templates.value.filter((item) =>
    [item.id, item.title, item.content].filter(Boolean).join(" ").toLowerCase().includes(query),
  );
});

const filteredReferenceTemplates = computed(() => {
  const query = templateReferenceQuery.value.trim().toLowerCase();
  if (!query) return templates.value;
  return templates.value.filter((item) =>
    [item.id, item.title, item.content].filter(Boolean).join(" ").toLowerCase().includes(query),
  );
});

const filteredSkills = computed(() => {
  const query = skillQuery.value.trim().toLowerCase();
  if (!query) return skills.value;
  return skills.value.filter((item) =>
    [item.name, item.notes, item.sourceUrl, item.content]
      .filter(Boolean)
      .join(" ")
      .toLowerCase()
      .includes(query),
  );
});

onMounted(async () => {
  removeScrollbarVisibility = installAutoHideScrollbars();
  window.addEventListener("keydown", handleWorkspaceShortcut);
  try {
    await restoreWindowState();
    unlistenWindowState = await listenWindowState();
  } catch {
    // 浏览器预览或窗口权限不可用时沿用配置中的默认尺寸。
  }
  try {
    unlistenDragDrop = await listenDragDrop(handleReferenceDragDrop);
  } catch {
    // 浏览器预览没有 Tauri 拖放事件，保留 HTML5 drop 作为兼容路径。
  }
  try {
    unlistenQueueUpdated = await listenEvent("queue-updated", handleQueueUpdatedEvent);
  } catch {
    // 预览环境可能没有事件通道。
  }
  try {
    unlistenTemplateFill = await listenEvent("template-fill", handleTemplateFillEvent);
  } catch {
    // 预览环境可能没有事件通道。
  }
  try {
    unlistenAgentProgress = await listenEvent("agent-progress", handleAgentProgressEvent);
  } catch {
    // 预览环境可能没有事件通道。
  }
  try {
    unlistenAgentTaskGroup = await listenEvent("agent-task-group", handleAgentTaskGroupEvent);
  } catch {
    // 预览环境可能没有事件通道。
  }
  await refreshAll();
  await refreshAgentSessions();
  syncAgentTaskGroupPolling();
  historyScrollRequest.value += 1;
  scheduleTodayRollover();
});

onUnmounted(() => {
  window.clearInterval(pollTimer);
  window.clearInterval(agentTaskGroupPollTimer);
  window.clearTimeout(todayRolloverTimer);
  unlistenDragDrop?.();
  unlistenQueueUpdated?.();
  unlistenTemplateFill?.();
  unlistenAgentProgress?.();
  unlistenAgentTaskGroup?.();
  unlistenWindowState?.();
  removeScrollbarVisibility?.();
  window.removeEventListener("keydown", handleWorkspaceShortcut);
});

function handleWorkspaceShortcut(event) {
  if (event.defaultPrevented || event.altKey || event.shiftKey || (!event.metaKey && !event.ctrlKey)) return;
  const mode = { 1: "drawing", 2: "agent" }[event.key];
  if (!mode) return;
  event.preventDefault();
  workspaceMode.value = mode;
}

// 首次加载或重大变更后，重新拉取设置、历史、队列和模板。
async function refreshAll() {
  try {
    const state = await invoke("load_app_state");
    applyState(state);
    setStatus("就绪", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function refreshAgentSessions() {
  try {
    const list = await invoke("list_agent_sessions");
    agentSessions.value = Array.isArray(list) ? list : [];
    if (!currentAgentSessionId.value && agentSessions.value[0]) {
      currentAgentSessionId.value = agentSessions.value[0].id;
      agentProviderId.value = agentSessions.value[0].modelProviderId || form.chatProviderId;
    }
    if (!agentSessions.value.length) await createAgentConversation();
    if (currentAgentSessionId.value) {
      await refreshAgentTaskGroups();
      syncAgentTaskGroupPolling();
    } else {
      syncAgentTaskGroupPolling();
    }
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function createAgentConversation() {
  if (!agentProviderId.value) agentProviderId.value = form.chatProviderId;
  try {
    const session = await invoke("create_agent_session", { providerId: agentProviderId.value || "" });
    agentSessions.value = [session, ...agentSessions.value.filter((item) => item.id !== session.id)];
    currentAgentSessionId.value = session.id;
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function selectAgentConversation(sessionId) {
  try {
    const session = await invoke("get_agent_session", { sessionId });
    agentSessions.value = [session, ...agentSessions.value.filter((item) => item.id !== session.id)];
    currentAgentSessionId.value = session.id;
    agentProviderId.value = session.modelProviderId || agentProviderId.value;
    agentStreamText.value = "";
    agentToolStatus.value = "";
    await refreshAgentTaskGroups();
    syncAgentTaskGroupPolling();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deleteAgentConversation(sessionId) {
  const confirmed = await requestConfirmation("删除 Agent 对话", "确认把这个 Agent 对话移入系统回收站？关联的绘图任务不会删除。");
  if (!confirmed) return;
  try {
    await invoke("delete_agent_session", { sessionId });
    currentAgentSessionId.value = "";
    await refreshAgentSessions();
    setStatus("Agent 对话已移入回收站", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function sendAgentConversationMessage(payload) {
  const content = typeof payload === "string" ? payload : payload?.content || "";
  const drawThisTurn = typeof payload !== "string" && Boolean(payload?.drawThisTurn);
  if (!currentAgentSessionId.value) await createAgentConversation();
  if (!currentAgentSessionId.value) return;
  if (drawThisTurn) {
    await createAgentDrawingTask(content);
    return;
  }
  agentBusy.value = true;
  agentStreamText.value = "";
  try {
    const session = await invoke("send_agent_message", {
      sessionId: currentAgentSessionId.value,
      providerId: agentProviderId.value,
      skillId: "",
      content,
      attachments: agentAttachments.value.map(({ dataUrl, ...attachment }) => attachment),
    });
    agentSessions.value = [session, ...agentSessions.value.filter((item) => item.id !== session.id)];
    agentAttachments.value = [];
    await refreshAgentTaskGroups();
    syncAgentTaskGroupPolling();
  } catch (error) {
    setStatus(String(error), "error");
    if (currentAgentSessionId.value) {
      await selectAgentConversation(currentAgentSessionId.value);
    }
  } finally {
    agentBusy.value = false;
    agentStreamText.value = "";
    agentToolStatus.value = "";
  }
}

async function createAgentDrawingTask(content) {
  if (!activeProvider.value?.apiKey) {
    setStatus("请先在 API 源里填写生图模型 API Key", "error");
    showApiDialog.value = true;
    return;
  }
  const attachments = agentAttachments.value.map(({ dataUrl, ...attachment }) => attachment);
  agentBusy.value = true;
  try {
    await invoke("create_agent_direct_image_task", {
      sessionId: currentAgentSessionId.value,
      content,
      attachments,
      plan: {
        title: content.split(/\r?\n/, 1)[0].slice(0, 32) || "直接绘画",
        prompt: content,
        providerId: activeProvider.value.id,
        resolution: form.resolution,
        ratio: form.ratio,
        quality: form.quality,
        promptFidelity: form.promptMode,
        referencePolicy: attachments.length ? "use" : "none",
        referenceIds: attachments.map((attachment) => attachment.id),
      },
    });
    agentAttachments.value = [];
    await refreshQueueOnly();
    await selectAgentConversation(currentAgentSessionId.value);
    setStatus("绘画任务已加入队列", "ok");
  } catch (error) {
    setStatus(String(error), "error");
    await selectAgentConversation(currentAgentSessionId.value);
  } finally {
    agentBusy.value = false;
  }
}

async function stopAgentConversation() {
  if (currentAgentSessionId.value) {
    try {
      await invoke("cancel_agent_turn", { sessionId: currentAgentSessionId.value });
      await selectAgentConversation(currentAgentSessionId.value);
    } catch (error) {
      setStatus(String(error), "error");
    }
  }
  agentBusy.value = false;
  agentStreamText.value = "";
}

async function addAgentReferenceImages() {
  const selected = await openDialog({ multiple: true, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
  const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
  await addAgentReferencePaths(paths);
}

async function addAgentReferencePaths(paths) {
  for (const path of paths || []) {
    try {
      const preview = await invoke("reference_from_path", { path });
      agentAttachments.value.push({
        id: createAgentAttachmentId(),
        path: preview.path,
        fileName: preview.fileName,
        mimeType: preview.mimeType,
        dataUrl: preview.dataUrl,
      });
    } catch (error) {
      setStatus(String(error), "error");
    }
  }
}

async function pasteAgentReferenceImage(event) {
  const paths = extractClipboardFilePaths(event?.clipboardData);
  const containsImage = paths.length > 0 || clipboardHasImage(event?.clipboardData);
  if (containsImage) event?.preventDefault?.();
  if (paths.length) {
    await addAgentReferencePaths(paths);
    return;
  }
  try {
    const preview = await invoke("reference_from_clipboard");
    if (preview && !agentAttachments.value.some((item) => item.path === preview.path)) {
      agentAttachments.value.push({
        id: createAgentAttachmentId(),
        path: preview.path,
        fileName: preview.fileName,
        mimeType: preview.mimeType,
        dataUrl: preview.dataUrl,
      });
    }
  } catch {
    // 普通文本粘贴不处理。
  }
}

function createAgentAttachmentId() {
  if (globalThis.crypto?.randomUUID) return `ref-${globalThis.crypto.randomUUID()}`;
  return `ref-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function removeAgentAttachment(id) {
  agentAttachments.value = agentAttachments.value.filter((item) => item.id !== id);
}

function handleAgentProgressEvent(event) {
  const payload = event?.payload || {};
  if (payload.sessionId !== currentAgentSessionId.value) return;
  if (payload.phase === "delta") agentStreamText.value += payload.chunk || "";
  if (["tool_delta", "tool_start", "tool_result"].includes(payload.phase)) {
    const tool = payload.toolName ? ` · ${payload.toolName}` : "";
    agentToolStatus.value = `${payload.message || "正在执行工具"}${tool}`;
  }
  if (payload.phase === "error") setStatus(payload.message || "Agent 调用失败", "error");
}

function openAgentTaskGroup(group) {
  workspaceMode.value = "drawing";
  const firstId = group?.taskIds?.[0];
  if (firstId) selectedTaskId.value = firstId;
  void refreshQueueOnly();
}

async function cancelAgentTaskGroup(group) {
  if (!group?.id) return;
  const confirmed = await requestConfirmation("取消任务组", "确认取消这个 Agent 任务组？已完成的任务不会删除，失败项仍可重试。");
  if (!confirmed) return;
  try {
    await invoke("cancel_agent_task_group", { taskGroupId: group.id });
    await refreshQueueOnly();
    if (currentAgentSessionId.value) await selectAgentConversation(currentAgentSessionId.value);
    await refreshAgentTaskGroups();
    setStatus("Agent 任务组已取消", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function retryAgentTaskGroup(group) {
  if (!group?.id) return;
  try {
    await invoke("retry_agent_task_group", { taskGroupId: group.id });
    await refreshQueueOnly();
    if (currentAgentSessionId.value) await selectAgentConversation(currentAgentSessionId.value);
    await refreshAgentTaskGroups();
    setStatus("Agent 失败任务已重新排队", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function retryAgentMessage(message) {
  const messages = currentAgentMessages.value;
  const index = messages.findIndex((item) => item.id === message?.id);
  const previousUser = messages
    .slice(0, index < 0 ? messages.length : index)
    .reverse()
    .find((item) => item.role === "user");
  if (previousUser?.content) {
    void sendAgentConversationMessage({ content: previousUser.content, drawThisTurn: false });
  }
}

function updateAgentAnswer({ key, value }) {
  agentAnswers.value = { ...agentAnswers.value, [key]: value };
}

function answerAgentQuestions(message) {
  const content = (message.questions || [])
    .map((question) => `${question.label}：${String(agentAnswers.value[question.key] || "").trim()}`)
    .join("\n");
  if (content.trim()) void sendAgentConversationMessage(content);
  agentAnswers.value = {};
}

async function handleAgentTaskGroupEvent(event) {
  const group = event?.payload;
  if (!group || group.sessionId !== currentAgentSessionId.value) return;
  workspaceMode.value = "drawing";
  const first = group.tasks?.[0];
  if (first?.id) selectedTaskId.value = first.id;
  await refreshQueueOnly();
  await refreshAgentTaskGroups();
  syncAgentTaskGroupPolling();
  historyScrollRequest.value += 1;
  setStatus(`Agent 已创建 ${group.tasks?.length || 0} 个绘图任务`, "ok");
}

function singleLine(value) {
  return String(value || "").replace(/\s+/g, " ").trim();
}

async function refreshAgentTaskGroups({ silent = true } = {}) {
  if (agentTaskGroupRefreshInFlight) return;
  const sessionId = currentAgentSessionId.value;
  const groups = currentAgentTaskGroups();
  if (!sessionId || !groups.length) return;
  agentTaskGroupRefreshInFlight = true;
  try {
    const updates = await Promise.all(groups.map(async (group) => {
      try {
        const tasks = await invoke("get_task_status", { taskGroupId: group.id, taskId: "" });
        const records = Array.isArray(tasks) ? tasks : [];
        return {
          id: group.id,
          status: summarizeTaskGroupStatus(records, group.status),
          taskIds: records.map((task) => task.id).filter(Boolean),
          titles: records.map((task) => singleLine(task.prompt)).filter(Boolean),
        };
      } catch (error) {
        return {
          id: group.id,
          status: "missing",
          taskIds: [],
          titles: [String(error)],
        };
      }
    }));
    if (sessionId !== currentAgentSessionId.value) return;
    applyAgentTaskGroupUpdates(sessionId, updates);
  } catch (error) {
    if (!silent) setStatus(String(error), "error");
  } finally {
    agentTaskGroupRefreshInFlight = false;
    syncAgentTaskGroupPolling();
  }
}

function currentAgentTaskGroups() {
  return currentAgentMessages.value
    .map((message) => message.taskGroup)
    .filter((group) => group?.id);
}

function applyAgentTaskGroupUpdates(sessionId, updates) {
  const byId = new Map(updates.map((item) => [item.id, item]));
  agentSessions.value = agentSessions.value.map((session) => {
    if (session.id !== sessionId) return session;
    return {
      ...session,
      messages: (session.messages || []).map((message) => {
        const group = message.taskGroup;
        const update = group?.id ? byId.get(group.id) : null;
        if (!update) return message;
        return {
          ...message,
          taskGroup: {
            ...group,
            status: update.status || group.status,
            taskIds: update.taskIds.length ? update.taskIds : group.taskIds,
            titles: update.titles.length ? update.titles : group.titles,
          },
        };
      }),
    };
  });
}

function summarizeTaskGroupStatus(tasks, fallback = "queued") {
  const statuses = tasks.map((task) => task.status).filter(Boolean);
  if (!statuses.length) return fallback || "queued";
  if (statuses.some((status) => status === "cancelling")) return "cancelling";
  if (statuses.some((status) => status === "running")) return "running";
  if (statuses.some((status) => status === "queued")) return "queued";
  if (statuses.every((status) => status === "completed")) return "completed";
  if (statuses.some((status) => status === "failed")) return "failed";
  if (statuses.some((status) => status === "cancelled")) return "cancelled";
  if (statuses.some((status) => status === "missing")) return "missing";
  return statuses[0] || fallback || "queued";
}

function syncAgentTaskGroupPolling() {
  window.clearInterval(agentTaskGroupPollTimer);
  agentTaskGroupPollTimer = 0;
  if (!currentAgentTaskGroups().some((group) => !isTerminalTaskGroupStatus(group.status))) return;
  agentTaskGroupPollTimer = window.setInterval(() => {
    void refreshAgentTaskGroups();
  }, AGENT_TASK_GROUP_POLL_INTERVAL);
}

function isTerminalTaskGroupStatus(status) {
  return ["completed", "failed", "cancelled", "missing"].includes(status);
}

// 只在队列活跃期间保留兜底轮询，平时由后端事件驱动刷新。
async function refreshQueueOnly({ silent = true } = {}) {
  if (queueRefreshInFlight) {
    queueRefreshQueued = true;
    return null;
  }
  queueRefreshInFlight = true;
  try {
    const snapshot = await invoke("queue_snapshot");
    applyQueue(snapshot);
    return snapshot;
  } catch (error) {
    if (!silent) setStatus(String(error), "error");
    return null;
  }
  finally {
    queueRefreshInFlight = false;
    if (queueRefreshQueued) {
      queueRefreshQueued = false;
      void refreshQueueOnly();
    }
  }
}

function handleQueueUpdatedEvent(event) {
  const snapshot = event?.payload;
  if (snapshot) {
    applyQueue(snapshot);
    if (currentAgentTaskGroups().some((group) => !isTerminalTaskGroupStatus(group.status))) {
      void refreshAgentTaskGroups();
    }
    return;
  }
  void refreshQueueOnly();
}

function syncQueuePolling() {
  window.clearInterval(pollTimer);
  pollTimer = 0;
  if (!isQueueActive()) return;
  pollTimer = window.setInterval(() => {
    void refreshQueueOnly();
  }, ACTIVE_QUEUE_POLL_INTERVAL);
}

function isQueueActive(snapshot = queue) {
  return Boolean(
    (snapshot.waiting?.length || 0)
    || (snapshot.running?.length || 0)
    || snapshot.workerActive,
  );
}

// 拖拽左右分隔条时，只调整相邻 panel 宽度并保留中间预览区最小空间。
function startPanelResize(target, event) {
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = panelSizes[target];

  const move = (moveEvent) => {
    const total = Math.max(0, window.innerWidth - 28 - 20);
    const resultMin = 430;
    if (target === "queue") {
      const max = Math.max(260, total - panelSizes.composer - resultMin);
      panelSizes.queue = clamp(startWidth + moveEvent.clientX - startX, 280, Math.min(500, max));
    } else {
      const max = Math.max(360, total - panelSizes.queue - resultMin);
      panelSizes.composer = clamp(startWidth - (moveEvent.clientX - startX), 400, Math.min(560, max));
    }
  };

  const stop = () => {
    document.body.classList.remove("resizing-panels");
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", stop);
    window.removeEventListener("pointercancel", stop);
  };

  document.body.classList.add("resizing-panels");
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", stop);
  window.addEventListener("pointercancel", stop);
}

// 把 Rust 返回的完整状态归一化为前端响应式状态。
function applyState(state) {
  settings.value = normalizeSettingsForUi(state.settings || defaultSettings());
  history.value = state.history || [];
  applyQueue(state.queue || {});
  templates.value = state.templates || [];
  skills.value = state.skills || [];
  ensureSelectedModels();
  ensureSelectedTask();
}

// 合并队列快照，并用后端 recent 字段刷新左侧历史时间线。
function applyQueue(snapshot) {
  queue.waiting = snapshot.waiting || [];
  queue.running = snapshot.running || [];
  queue.recent = snapshot.recent || [];
  queue.workerActive = Boolean(snapshot.workerActive);
  queue.updatedAt = snapshot.updatedAt || "";
  if (snapshot.recent) {
    history.value = snapshot.recent;
  }
  syncQueuePolling();
  ensureSelectedTask();
}

// 没有选中任务时，默认定位到最新一条历史记录。
function ensureSelectedTask() {
  if (selectedTask.value) return;
  const next = historyTimeline.value.at(-1);
  selectedTaskId.value = next?.id || "";
}

// 将当前工作台参数组装成 Images API 请求并加入后台队列。
async function submitTask() {
  if (!form.prompt.trim()) {
    setStatus("提示词不能为空", "error");
    return;
  }
  if (!activeProvider.value?.apiKey) {
    setStatus("请先在 API 源里填写 API Key", "error");
    showApiDialog.value = true;
    return;
  }
  submitting.value = true;
  try {
    const request = buildImageRequest(form.prompt, form.promptMode);
    const task = await invoke("enqueue_generation", { request });
    selectedTaskId.value = task.id;
    setStatus("已开始生成", "ok");
    await refreshQueueOnly();
    historyScrollRequest.value += 1;
  } catch (error) {
    setStatus(String(error), "error");
  } finally {
    submitting.value = false;
  }
}

function buildImageRequest(prompt, promptFidelity = form.promptMode) {
  return {
    providerId: form.providerId || settings.value.activeImageProviderId || settings.value.activeProviderId,
    prompt,
    referencePaths: references.value.map((item) => item.path),
    size: sizeForPreset(form.resolution, form.ratio),
    resolution: form.resolution,
    ratio: form.ratio,
    orientation: orientationForRatio(form.ratio),
    quality: form.quality,
    outputFormat: "png",
    count: 1,
    promptFidelity,
  };
}

// 从文件选择器导入参考图，并转换为可预览的数据 URL。
async function addReferenceImages() {
  await chooseReferenceImages(references, "已添加参考图");
}

// 在提示词框粘贴图片时，把剪贴板图片保存为参考图。
async function handlePromptPaste(event) {
  await pasteReferenceImage(event, references, "已从剪贴板添加参考图");
}

async function pasteWorkbenchReferenceImage() {
  await pasteClipboardReference(references, "已从剪贴板添加参考图");
}

async function chooseReferenceImages(target, successMessage) {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    await addReferencePaths(target, paths, successMessage);
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function addReferencePaths(target, paths, successMessage) {
  return addReferencePathsWithOptions(target, paths, successMessage);
}

async function addReferencePathsWithOptions(target, paths, successMessage, options = {}) {
  let added = 0;
  let lastError = null;
  try {
    for (const path of paths) {
      try {
        const preview = await invoke("reference_from_path", { path });
        if (appendReferencePreview(target, preview)) added += 1;
      } catch (error) {
        lastError = error;
      }
    }
    if (added) setStatus(`${successMessage}：${added} 张`, "ok");
    if (!added && lastError && !options.silentInvalid) setStatus(String(lastError), "error");
  } catch (error) {
    setStatus(String(error), "error");
  }
  return added;
}

function handleReferenceDragDrop(event) {
  const payload = event?.payload || {};
  if (showSkillEditor.value && skillDropTarget(payload.position)) {
    clearReferenceDragTargets();
    if (payload.type === "drop" && payload.paths?.length) {
      void loadSkillMarkdownPath(payload.paths[0]);
    }
    return;
  }
  const target = referenceDropTarget(payload.position) || defaultReferenceDropTarget();
  if (payload.type === "enter" || payload.type === "over") {
    setReferenceDragTarget(target);
    return;
  }
  if (payload.type === "leave") {
    clearReferenceDragTargets();
    return;
  }
  clearReferenceDragTargets();
  if (payload.type === "drop" && payload.paths?.length) {
    void addDraggedReferencePaths(target, payload.paths);
  }
}

function skillDropTarget(position) {
  const x = Number(position?.x);
  const y = Number(position?.y);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return false;
  const scale = window.devicePixelRatio || 1;
  return [[x, y], [x / scale, y / scale]].some(([left, top]) =>
    Boolean(document.elementFromPoint(left, top)?.closest("[data-skill-drop-target]")),
  );
}

function handleReferenceDropEvent(event) {
  clearReferenceDragTargets();
  const paths = extractDroppedFilePaths(event?.dataTransfer);
  if (paths.length) void addDraggedReferencePaths("workbench", paths);
}

function handleTemplateDraftDropEvent(event) {
  clearReferenceDragTargets();
  const paths = extractDroppedFilePaths(event?.dataTransfer);
  if (paths.length) void addDraggedReferencePaths("template-draft", paths);
}

function addDraggedReferencePaths(target, paths) {
  const destination = target === "template-draft" ? templateDraftReferences : references;
  const message = target === "template-draft" ? "已添加模板参考图" : "已添加拖放参考图";
  return addReferencePathsWithOptions(destination, paths, message, { silentInvalid: true });
}

function referenceDropTarget(position) {
  const x = Number(position?.x);
  const y = Number(position?.y);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return false;
  const scale = window.devicePixelRatio || 1;
  for (const [left, top] of [[x, y], [x / scale, y / scale]]) {
    const zone = document
      .elementFromPoint(left, top)
      ?.closest("[data-reference-drop-target]");
    if (zone?.dataset.referenceDropTarget) return zone.dataset.referenceDropTarget;
  }
  return "";
}

function defaultReferenceDropTarget() {
  if (showTemplateEditor.value && templateEditorMode.value !== "view") return "template-draft";
  return "workbench";
}

function setReferenceDragTarget(target) {
  referenceDragActive.value = target === "workbench";
  templateDraftDragActive.value = target === "template-draft";
}

function clearReferenceDragTargets() {
  referenceDragActive.value = false;
  templateDraftDragActive.value = false;
}

async function pasteReferenceImage(event, target, successMessage) {
  const clipboardData = event?.clipboardData;
  const filePaths = extractClipboardFilePaths(clipboardData);
  if (filePaths.length) {
    event.preventDefault();
    await addReferencePathsWithOptions(target, filePaths, successMessage, {
      silentInvalid: true,
    });
    return;
  }
  const items = Array.from(clipboardData?.items || []);
  const files = Array.from(clipboardData?.files || []);
  const types = Array.from(clipboardData?.types || []);
  const hasImage = [...items, ...files].some((item) => item.type?.startsWith("image/"));
  const hasFilePayload = files.length > 0 || types.includes("Files");
  const hasText = ["text/plain", "text/uri-list"].some((type) => {
    try {
      return Boolean(clipboardData?.getData?.(type)?.trim());
    } catch {
      return false;
    }
  });
  if (!hasImage && !hasFilePayload && hasText) return;
  event?.preventDefault();
  await pasteClipboardReference(target, successMessage);
}

async function pasteClipboardReference(target, successMessage) {
  try {
    const preview = await invoke("reference_from_clipboard");
    if (!preview) {
      setStatus("剪贴板中没有可用图片", "error");
      return false;
    }
    if (!appendReferencePreview(target, preview)) {
      setStatus("剪贴板图片已经添加", "busy");
      return false;
    }
    setStatus(successMessage, "ok");
    return true;
  } catch (error) {
    setStatus(String(error), "error");
    return false;
  }
}

function appendReferencePreview(target, preview) {
  if (target.value.some((item) => item.path === preview.path)) return false;
  target.value.push({ ...preview, previewUrl: preview.dataUrl });
  return true;
}

// 参考图只从当前草稿移除，不需要删除确认。
function removeReference(target, index) {
  const items = Array.isArray(target) ? target : target?.value;
  if (!Array.isArray(items) || !items[index]) return;
  items.splice(index, 1);
}

async function restoreReferencePreviews(paths) {
  const restored = [];
  let missing = 0;
  for (const path of paths || []) {
    try {
      const preview = await invoke("reference_from_path", { path });
      restored.push({ ...preview, previewUrl: preview.dataUrl });
    } catch {
      missing += 1;
    }
  }
  return { restored, missing };
}

// 将历史任务的提示词、参数、模型和参考图恢复到工作台。
async function reuseTask(task) {
  if (!task) return;

  const params = task.params || {};
  form.prompt = task.prompt || "";
  form.promptMode = params.promptFidelity || DEFAULT_PROMPT_MODE;
  form.resolution = params.resolution || form.resolution;
  form.ratio = params.ratio || form.ratio;
  form.quality = params.quality || form.quality;

  const provider = imageProviders.value.find((item) => item.id === task.providerId)
    || imageProviders.value.find((item) =>
      item.name === task.providerName && item.imageModel === task.model,
    )
    || imageProviders.value.find((item) => item.imageModel === task.model);
  if (provider) form.providerId = provider.id;
  const missingProvider = !provider && Boolean(task.providerId || task.providerName || task.model);

  const restoredReferences = [];
  let missingReferenceCount = 0;
  for (const path of task.referencePaths || []) {
    try {
      const preview = await invoke("reference_from_path", { path });
      restoredReferences.push({ ...preview, previewUrl: preview.dataUrl });
    } catch {
      missingReferenceCount += 1;
    }
  }
  references.value = restoredReferences;
  showTaskDetail.value = false;

  const warnings = [];
  if (missingProvider) warnings.push("原生图模型已不存在");
  if (missingReferenceCount) warnings.push(`${missingReferenceCount} 张参考图已不存在`);
  const warningMessage = warnings.length ? `，${warnings.join("，")}` : "";
  setStatus(`已将任务参数填入工作台${warningMessage}`, warnings.length ? "busy" : "ok");
}

// 手动刷新单个任务的状态，主要用于正在运行的历史项。
async function refreshTask(task) {
  const snapshot = await refreshQueueOnly({ silent: false });
  if (!snapshot) return;
  const refreshed = historyTimeline.value.find((item) => item.id === task.id);
  setStatus(
    refreshed ? `任务状态：${statusLabel(refreshed.status)}` : "任务已不在历史记录中",
    refreshed ? "ok" : "busy",
  );
}

// 把失败任务重新放回队列，沿用原始请求文件。
async function retryTask(task) {
  try {
    await invoke("retry_task", { taskId: task.id });
    selectedTaskId.value = task.id;
    setStatus("任务已重新排队", "ok");
    await refreshQueueOnly();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 删除历史记录，同时由后端负责把对应输出图移入回收站。
async function deleteTask(task) {
  const confirmed = await requestConfirmation(
    "删除历史任务",
    "确认删除这条生成记录？对应图片会移入系统回收站。",
  );
  if (!confirmed) return;
  try {
    await invoke("delete_task", { taskId: task.id });
    if (selectedTaskId.value === task.id) selectedTaskId.value = "";
    setStatus("生成记录已删除", "ok");
    await refreshAll();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 保存 API 源配置后，重新选择可用的生图和对话模型。
async function saveApiSettings(nextSettings) {
  try {
    const saved = await invoke("save_settings", { settings: nextSettings });
    settings.value = normalizeSettingsForUi(saved);
    ensureSelectedModels(true);
    setStatus("API 源已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 下载输出图到系统 Downloads，并立即在 Finder 中定位。
async function downloadOutput(output) {
  try {
    const savedPath = await invoke("download_output", { path: output.path });
    setStatus(`已保存到下载目录：${fileName(savedPath)}`, "ok");
    await reveal(savedPath);
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 把生成图写入系统剪贴板，方便粘贴到其它应用。
async function copyOutput(output) {
  try {
    await invoke("copy_image_to_clipboard", { path: output.path });
    setStatus("图片已复制到剪贴板", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 打开空白模板编辑器。
function newTemplate() {
  Object.assign(templateDraft, emptyTemplate());
  templateDraftReferences.value = [];
  templateDraftEffectImage.value = null;
  templateEditorMode.value = "new";
  showTemplateEditor.value = true;
}

async function newTemplateFromTask({ task, output }) {
  if (!task || !output?.path) return;
  Object.assign(templateDraft, {
    ...emptyTemplate(),
    content: output.revisedPrompt || task.prompt || "",
  });
  const { restored, missing } = await restoreReferencePreviews(task.referencePaths || []);
  templateDraftReferences.value = restored;
  templateDraftEffectImage.value = await restoreEffectImage(output.path);
  templateEditorMode.value = "new";
  showTemplateEditor.value = true;
  if (missing) setStatus(`${missing} 张任务参考图已不存在`, "busy");
}

// 以只读模式查看模板，并在弹窗中高亮占位符。
async function viewTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  const { restored } = await restoreReferencePreviews(template.referencePaths);
  templateDraftReferences.value = restored;
  templateDraftEffectImage.value = await restoreEffectImage(template.effectImagePath);
  templateEditorMode.value = "view";
  showTemplateEditor.value = true;
}

// 以编辑模式打开模板。
async function editTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  const { restored } = await restoreReferencePreviews(template.referencePaths);
  templateDraftReferences.value = restored;
  templateDraftEffectImage.value = await restoreEffectImage(template.effectImagePath);
  templateEditorMode.value = "edit";
  showTemplateEditor.value = true;
}

// 保存新增或编辑后的模板，并刷新模板列表。
async function savePromptTemplate() {
  try {
    templateDraft.referencePaths = templateDraftReferences.value.map((item) => item.path);
    templateDraft.effectImagePath = templateDraftEffectImage.value?.path || "";
    templates.value = await invoke("save_template", { template: deepClone(templateDraft) });
    showTemplateEditor.value = false;
    setStatus("模板已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 打开空白 Skill 编辑器，名称会在保存时从 Markdown 中自动提取。
function newSkill() {
  Object.assign(skillDraft, emptySkill());
  skillEditorMode.value = "new";
  showSkillEditor.value = true;
}

function viewSkill(skill) {
  Object.assign(skillDraft, deepClone(skill));
  skillEditorMode.value = "view";
  showSkillEditor.value = true;
}

function editSkill(skill) {
  Object.assign(skillDraft, deepClone(skill));
  skillEditorMode.value = "edit";
  showSkillEditor.value = true;
}

// 从 URL 提取 Markdown，后端会继续尝试目录下的大小写 Skill 文件名。
async function fetchSkillContent() {
  if (!skillDraft.sourceUrl.trim()) return;
  skillFetching.value = true;
  setStatus("正在提取 Skill…", "busy");
  try {
    const result = await invoke("fetch_skill_markdown", { sourceUrl: skillDraft.sourceUrl });
    skillDraft.sourceUrl = result.sourceUrl;
    skillDraft.content = result.content;
    setStatus("Skill 已提取", "ok");
  } catch (error) {
    const message = String(error);
    await showNotice("Skill 提取失败", message);
    setStatus(message, "error");
  } finally {
    skillFetching.value = false;
  }
}

async function handleSkillMarkdownDrop(event) {
  const file = Array.from(event?.dataTransfer?.files || [])[0];
  if (file?.path) {
    await loadSkillMarkdownPath(file.path);
    return;
  }
  if (file?.name && !file.name.toLowerCase().endsWith(".md")) {
    setStatus("只支持拖入 Skill 目录或 .md 文件", "error");
    return;
  }
  if (file?.text) {
    skillDraft.content = await file.text();
    setStatus("已读取 Markdown Skill", "ok");
    return;
  }
  const path = extractDroppedFilePaths(event?.dataTransfer)[0];
  if (path) await loadSkillMarkdownPath(path);
}

async function loadSkillMarkdownPath(path) {
  try {
    skillDraft.content = await invoke("read_skill_markdown_file", { path });
    skillDraft.sourcePath = path;
    setStatus("已读取 Markdown Skill", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 保存纯 Markdown Skill；脚本依赖错误使用统一通知弹窗反馈。
async function saveSkill() {
  try {
    try {
      skills.value = await invoke("save_skill", { skill: deepClone(skillDraft), replace: false });
    } catch (error) {
      if (!String(error).includes("CONFIRM_REPLACE_SKILL")) throw error;
      const confirmed = await requestConfirmation(
        "覆盖保存 Skill",
        "同名 Skill 已存在，确认把旧版本移入回收站并保存当前内容？",
      );
      if (!confirmed) return;
      skills.value = await invoke("save_skill", { skill: deepClone(skillDraft), replace: true });
    }
    showSkillEditor.value = false;
    setStatus("Skill 已保存", "ok");
  } catch (error) {
    const message = String(error);
    await showNotice("无法保存 Skill", message);
    setStatus(message, "error");
  }
}

async function deleteSkill(skillId) {
  const confirmed = await requestConfirmation("删除 Skill", "确认删除这个 Skill？");
  if (!confirmed) return;
  try {
    skills.value = await invoke("delete_skill", { skillId });
    setStatus("Skill 已删除", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function addTemplateDraftReferenceImages() {
  await chooseReferenceImages(templateDraftReferences, "已添加模板参考图");
}

async function addTemplateDraftEffectImage() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return;
    const preview = await invoke("reference_from_path", { path });
    templateDraftEffectImage.value = { ...preview, previewUrl: preview.dataUrl };
    setStatus("已添加模板效果图", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function pasteTemplateDraftEffectImage() {
  try {
    const preview = await invoke("reference_from_clipboard");
    if (!preview) {
      setStatus("剪贴板中没有可用图片", "error");
      return;
    }
    templateDraftEffectImage.value = { ...preview, previewUrl: preview.dataUrl };
    setStatus("已从剪贴板添加模板效果图", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function handleTemplateDraftPaste(event) {
  await pasteReferenceImage(event, templateDraftReferences, "已从剪贴板添加模板参考图");
}

// 让用户选择保存位置，并导出包含 Markdown 和参考图资源的模板 ZIP。
async function exportPromptTemplates() {
  if (!templates.value.length) {
    setStatus("没有可导出的模板", "error");
    return;
  }
  try {
    const destination = await saveDialog({
      defaultPath: "ImageForge-templates.zip",
      filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
    });
    if (!destination) return;
    const savedPath = await invoke("export_templates", { destination });
    setStatus(`模板已导出：${fileName(savedPath)}`, "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 从 Image Forge 模板包导入提示词和参考图，重复模板由后端自动跳过。
async function importPromptTemplates() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "Image Forge 模板包", extensions: ["zip"] }],
    });
    const archivePath = Array.isArray(selected) ? selected[0] : selected;
    if (!archivePath) return;
    const result = await invoke("import_templates", { archivePath });
    templates.value = result.templates || [];
    const message = `导入 ${result.importedCount || 0} 个，重复 ${result.skippedCount || 0} 个。`;
    await showNotice("模板导入结果", message);
    setStatus(message, "ok");
  } catch (error) {
    const message = String(error);
    await showNotice("模板导入失败", message);
    setStatus(message, "error");
  }
}

// 删除模板维护列表中的指定模板。
async function deletePromptTemplate(id) {
  const confirmed = await requestConfirmation(
    "删除模板",
    "确认删除这个模板？未被其它记录引用的参考图会移入系统回收站。",
  );
  if (!confirmed) return;
  try {
    templates.value = await invoke("delete_template", { templateId: id });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 交换当前模板与搜索结果中相邻模板的位置，并持久化完整模板顺序。
async function movePromptTemplate({ templateId, targetTemplateId }) {
  try {
    templates.value = await invoke("move_template", { templateId, targetTemplateId });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

// 在引用模板弹窗中选择模板，并保留搜索条件。
async function selectReferenceTemplate(template) {
  selectedReferenceTemplateId.value = template.id;
  templateReferenceSourceContent.value = template.content || "";
  templateReferenceGeneratedContent.value = "";
  templateFilledRanges.value = [];
  const selectedId = template.id;
  const { restored, missing } = await restoreReferencePreviews(template.referencePaths);
  if (selectedReferenceTemplateId.value !== selectedId) return;
  templateReferenceReferences.value = restored;
  templateReferenceEffectImage.value = await restoreEffectImage(template.effectImagePath);
  if (missing) setStatus(`${missing} 张模板参考图已不存在`, "busy");
}

async function restoreEffectImage(path) {
  if (!path) return null;
  try {
    const preview = await invoke("reference_from_path", { path });
    return { ...preview, previewUrl: preview.dataUrl };
  } catch {
    return null;
  }
}

function showTemplateEffect(template) {
  if (!template?.effectImagePath) return;
  effectViewer.path = template.effectImagePath;
  effectViewer.title = `${template.title || "模板"} · 效果图`;
  effectViewer.show = true;
}

function showTemplateEffectByPreview(preview) {
  if (!preview?.path) return;
  effectViewer.path = preview.path;
  effectViewer.title = "模板效果图";
  effectViewer.show = true;
}

async function addTemplateCallReferenceImages() {
  await chooseReferenceImages(templateReferenceReferences, "已添加本次调用参考图");
}

async function pasteTemplateCallReferenceImage() {
  await pasteClipboardReference(templateReferenceReferences, "已从剪贴板添加本次调用参考图");
}

function updateTemplateReferenceSource(content) {
  templateReferenceSourceContent.value = content;
  templateFilledRanges.value = mapFilledRanges(content, templateReferenceGeneratedContent.value);
}

function updateTemplateReferenceGenerated(content) {
  templateReferenceGeneratedContent.value = content;
  templateFilledRanges.value = mapFilledRanges(templateReferenceSourceContent.value, content);
}

// 调用对话模型填充模板中的 `{}` 占位区域。
async function fillReferenceTemplate() {
  if (!templateReferenceSourceContent.value.trim()) {
    setStatus("请先选择或输入模板内容", "error");
    return;
  }
  if (!form.chatProviderId) {
    setStatus("请先选择对话模型", "error");
    return;
  }
  templateFilling.value = true;
  templateFillSessionId.value = createTemplateFillSessionId();
  const sessionId = templateFillSessionId.value;
  templateReferenceGeneratedContent.value = "";
  templateFilledRanges.value = [];
  setStatus("AI 正在填充模板…", "busy");
  try {
    const original = templateReferenceSourceContent.value;
    const filled = await invoke("fill_prompt_template", {
      sessionId,
      providerId: form.chatProviderId,
      template: original,
    });
    if (sessionId !== templateFillSessionId.value) return;
    templateReferenceGeneratedContent.value = filled;
    templateFilledRanges.value = mapFilledRanges(original, filled);
    setStatus("模板已填充", "ok");
  } catch (error) {
    const message = String(error);
    await showNotice("AI 填充失败", message);
    setStatus(message, "error");
  } finally {
    templateFilling.value = false;
  }
}

function handleTemplateFillEvent(event) {
  const payload = event?.payload || {};
  if (!templateFilling.value || payload.sessionId !== templateFillSessionId.value) return;
  if (payload.phase === "delta" && payload.mode === "stream") {
    templateReferenceGeneratedContent.value += payload.chunk || "";
    templateFilledRanges.value = [];
  }
}

function createTemplateFillSessionId() {
  if (globalThis.crypto?.randomUUID) {
    return `template-fill-${globalThis.crypto.randomUUID()}`;
  }
  return `template-fill-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

async function openAbout() {
  showAboutDialog.value = true;
  try {
    aboutInfo.value = await invoke("about_info");
  } catch (error) {
    aboutInfo.value = {
      version: "",
      buildTime: "",
    };
    setStatus(`读取关于信息失败：${String(error)}`, "error");
  }
}

async function openRuntimeLogs() {
  try {
    runtimeLogText.value = await invoke("runtime_logs");
  } catch (error) {
    runtimeLogText.value = `读取运行日志失败：${String(error)}`;
  }
  showRuntimeLogDialog.value = true;
}

async function openCleanup() {
  cleanupCandidates.value = [];
  cleanupError.value = "";
  cleanupLoading.value = true;
  showCleanupDialog.value = true;
  try {
    cleanupCandidates.value = await invoke("scan_cleanup_candidates");
  } catch (error) {
    cleanupError.value = String(error);
  } finally {
    cleanupLoading.value = false;
  }
}

async function confirmCleanup() {
  cleanupConfirming.value = true;
  cleanupError.value = "";
  try {
    const removed = await invoke("cleanup_data_files");
    cleanupCandidates.value = [];
    showCleanupDialog.value = false;
    setStatus(`已清理 ${removed.length} 个孤岛文件`, "ok");
  } catch (error) {
    cleanupError.value = String(error);
  } finally {
    cleanupConfirming.value = false;
  }
}

// 将引用模板内容插入到提示词当前光标位置。
async function insertReferenceTemplate() {
  const content = templateReferenceGeneratedContent.value.trim()
    ? templateReferenceGeneratedContent.value
    : templateReferenceSourceContent.value;
  if (!content.trim()) {
    setStatus("模板内容为空", "error");
    return;
  }
  insertTextAtCursor(content);
  for (const preview of templateReferenceReferences.value) {
    appendReferencePreview(references, preview);
  }
  if (selectedReferenceTemplateId.value) {
    try {
      templates.value = await invoke("mark_template_used", { templateId: selectedReferenceTemplateId.value });
    } catch (error) {
      setStatus(String(error), "error");
    }
  }
  showTemplateReferenceDialog.value = false;
  setStatus("模板及参考图已引用到工作台", "ok");
}

// 清空工作台的提示词、参考图和拖放状态，并重置插入光标。
function clearPrompt() {
  form.prompt = "";
  references.value = [];
  referenceDragActive.value = false;
  promptCursor.value = 0;
}

// 记录提示词光标位置，供模板插入使用。
function capturePromptCursor(event) {
  const target = event?.target;
  if (typeof target?.selectionStart === "number") {
    promptCursor.value = target.selectionStart;
  }
}

// 在记录的光标位置插入文本，不覆盖原有提示词。
function insertTextAtCursor(text) {
  const start = clamp(promptCursor.value, 0, form.prompt.length);
  form.prompt = `${form.prompt.slice(0, start)}${text}${form.prompt.slice(start)}`;
  promptCursor.value = start + text.length;
}

// 根据原模板占位符位置，推算 AI 填充后应高亮的文本范围。
function mapFilledRanges(original, filled) {
  const placeholders = templatePlaceholders(original);
  if (!placeholders.length) return [];
  const ranges = [];
  let originalCursor = 0;
  let filledCursor = 0;
  for (const placeholder of placeholders) {
    const prefix = original.slice(originalCursor, placeholder.start);
    const prefixIndex = prefix ? filled.indexOf(prefix, filledCursor) : filledCursor;
    if (prefixIndex >= 0) {
      filledCursor = prefixIndex + prefix.length;
    }
    const suffixStart = placeholder.end;
    const nextPlaceholder = placeholders.find((item) => item.start >= suffixStart);
    const suffix = nextPlaceholder
      ? original.slice(suffixStart, nextPlaceholder.start)
      : original.slice(suffixStart);
    const suffixIndex = suffix ? filled.indexOf(suffix, filledCursor) : filled.length;
    const end = suffixIndex >= 0 ? suffixIndex : filled.length;
    if (end > filledCursor) {
      ranges.push({ start: filledCursor, end });
    }
    filledCursor = end;
    originalCursor = suffixStart;
  }
  return ranges;
}

// 识别模板里由 `{}` 包裹的占位描述。
function templatePlaceholders(value) {
  const matches = [];
  const pattern = /\{[^{}]+\}/g;
  let match;
  while ((match = pattern.exec(value)) !== null) {
    matches.push({ start: match.index, end: match.index + match[0].length });
  }
  return matches;
}

// 调用系统文件管理器定位文件或目录。
async function reveal(path) {
  try {
    await invoke("reveal_path", { path });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function setStatus(message, tone = "idle") {
  statusText.value = message;
  statusTone.value = tone;
}

function requestConfirmation(title, message) {
  return new Promise((resolve) => {
    confirmation.title = title;
    confirmation.message = message;
    confirmation.resolve = resolve;
    confirmation.visible = true;
  });
}

function resolveConfirmation(confirmed) {
  const resolve = confirmation.resolve;
  confirmation.resolve = null;
  confirmation.visible = false;
  resolve?.(confirmed);
}

function showNotice(title, message, buttonText = "确认") {
  return new Promise((resolve) => {
    notice.title = title;
    notice.message = message;
    notice.buttonText = buttonText;
    notice.resolve = resolve;
    notice.visible = true;
  });
}

function resolveNotice() {
  const resolve = notice.resolve;
  notice.resolve = null;
  notice.visible = false;
  resolve?.();
}

function modelOptionLabel(provider) {
  return `${provider.name} · ${provider.imageModel || "未设置模型"}`;
}

function taskTime(task) {
  return task.createdAt || task.updatedAt || task.completedAt || "";
}

function localDateKey(value) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function scheduleTodayRollover() {
  window.clearTimeout(todayRolloverTimer);
  const now = new Date();
  const nextDay = new Date(now);
  nextDay.setHours(24, 0, 0, 0);
  todayRolloverTimer = window.setTimeout(() => {
    todayKey.value = localDateKey(new Date());
    scheduleTodayRollover();
  }, Math.max(1000, nextDay.getTime() - now.getTime()));
}

// 根据设置、历史成功任务和当前列表，保证模型选择始终可用。
function ensureSelectedModels(preferSaved = false) {
  if (preferSaved || !imageProviders.value.some((provider) => provider.id === form.providerId)) {
    form.providerId = preferSaved
      ? (settings.value.activeImageProviderId || settings.value.activeProviderId || imageProviders.value[0]?.id || "")
      : (lastSuccessfulImageProviderId() || imageProviders.value[0]?.id || "");
  }
  if (preferSaved || !chatProviders.value.some((provider) => provider.id === form.chatProviderId)) {
    form.chatProviderId = settings.value.activeChatProviderId || chatProviders.value[0]?.id || "";
  }
}

// 从最近成功任务中找回生图模型，作为工作台默认选择。
function lastSuccessfulImageProviderId() {
  for (const task of [...history.value].reverse()) {
    if (task.status !== "completed") continue;
    const provider = imageProviders.value.find((item) => item.id === task.providerId)
      || imageProviders.value.find((item) =>
        item.name === task.providerName && item.imageModel === task.model,
      )
      || imageProviders.value.find((item) => item.imageModel === task.model);
    if (provider) return provider.id;
  }
  return "";
}
</script>
