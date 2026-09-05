<template>
  <div v-if="isWeb && !unlocked" class="lock-screen">
    <div class="lock-screen-card">
      <img :src="logoUrl" alt="Image Forge" class="lock-screen-logo" />
      <h1>Image Forge</h1>
      <p>输入访问密码以继续</p>
      <n-input
        v-model:value="lockPassword"
        type="password"
        size="large"
        placeholder="访问密码"
        :status="lockError ? 'error' : undefined"
        @keydown.enter="unlock"
      />
      <n-button
        type="primary"
        size="large"
        :disabled="!lockPassword.trim()"
        class="lock-screen-btn"
        @click="unlock"
      >
        解锁
      </n-button>
      <p v-if="lockError" class="lock-screen-error">{{ lockError }}</p>
    </div>
  </div>
  <n-config-provider v-else :theme-overrides="themeOverrides" component-size="small">
    <n-global-style />
    <AppShell>
      <AgentWorkspace
        :sessions="agentSessions"
        :current-session="currentAgentSession"
        :messages="currentAgentDisplayMessages"
        :provider-id="form.chatProviderId"
        :image-provider-id="activeProvider?.id || ''"
        :busy="agentBusy"
        :stream-text="agentStreamText"
        :tool-status-text="agentToolStatus"
        :answers="agentAnswers"
        :attachments="agentAttachments"
        :agent-library-version="agentLibraryVersion"
        :ratio="form.ratio"
        :resolution="form.resolution"
        :prefill-prompt="agentPrefillPrompt"
        :templates="templates"
        :template-fill-busy="templateFillBusy"
        @create="createAgentConversation"
        @select="selectAgentConversation"
        @send="sendAgentConversationMessage"
        @stop="stopAgentConversation"
        @add-reference="addAgentReferenceImages"
        @remove-attachment="removeAgentAttachment"
        @open-task-group="openAgentTaskGroup"
        @preview-images="openImageViewer"
        @cancel-task-group="cancelAgentTaskGroup"
        @retry-task-group="retryAgentTaskGroup"
        @retry="retryAgentMessage"
        @paste-reference="pasteAgentReferenceImage"
        @drop-reference="addAgentReferencePaths"
        @update-answer="updateAgentAnswer"
        @answer-questions="answerAgentQuestions"
        @delete-session="deleteAgentConversation"
        @rename-session="renameAgentConversation"
        @delete-task="deleteTask"
        @download-output="downloadOutput"
        @reveal-output="reveal($event.path)"
        @open-settings="openDesign"
        @apply-template="handleApplyTemplate"
        @fill-template="handleFillTemplate"
        @update:ratio="form.ratio = $event"
        @update:resolution="form.resolution = $event"
        @reference-to-agent="handleLibraryReferenceToAgent"
        @add-to-template="handleLibraryAddToTemplate"
        @redraw-task-group="handleRedrawTaskGroup"
      />

      <template #footer>
        <AppFooterBar
          :status-text="statusText"
          :status-tone="statusTone"
          :image-provider-id="activeProvider?.id || ''"
          :image-provider-name="activeProvider?.name || ''"
          :image-provider-options="imageProviderOptions"
          :chat-provider-id="activeChatProvider?.id || ''"
          :chat-provider-name="activeChatProvider?.name || ''"
          :chat-provider-options="chatProviderOptions"
          :running-count="queue.running.length"
          :waiting-count="queue.waiting.length"
          :image-provider-missing-key="Boolean(activeProvider && !activeProvider.apiKey)"
          @select-image-provider="selectApiProvider('image', $event)"
          @select-chat-provider="selectApiProvider('chat', $event)"
        />
      </template>

      <template #dialogs>
        <ApiSourceDialog
          v-model:show="showApiDialog"
          :settings="settings"
          @save="saveApiSettings"
        />

        <DesignDialog
          v-model:show="showDesignDialog"
          :settings="settings"
          :templates="templates"
          :info="aboutInfo"
          @save-api="saveApiSettings"
          @view-template="viewTemplate"
          @edit-template="editTemplate"
          @delete-template="deletePromptTemplate"
          @create-template="newTemplate"
          @import-template="importPromptTemplates"
          @export-template="exportPromptTemplates"
          @move-template="movePromptTemplate"
          @show-template-effect="showTemplateEffect"
          @show-logs="openRuntimeLogs"
          @cleanup="openCleanup"
          @export-data="openExportData"
          @import-data="openImportData"
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

        <DataTransferDialog v-model:show="showDataTransfer" :mode="dataTransferMode" />

        <EffectImageViewer
          v-model:show="effectViewer.show"
          :image-path="effectViewer.path"
          :title="effectViewer.title"
          :items="effectViewer.items"
          :initial-index="effectViewer.index"
        />

        <CleanupDialog
          v-model:show="showCleanupDialog"
          :candidates="cleanupCandidates"
          :loading="cleanupLoading"
          :confirming="cleanupConfirming"
          :error="cleanupError"
          @confirm="confirmCleanup"
        />

        <RuntimeLogDialog v-model:show="showRuntimeLogDialog" :logs="runtimeLogText" />

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
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue';
import AgentWorkspace from './components/AgentWorkspace.vue';
import AppFooterBar from './components/AppFooterBar.vue';
import AppShell from './components/AppShell.vue';
import CleanupDialog from './components/dialogs/CleanupDialog.vue';
import ApiSourceDialog from './components/dialogs/ApiSourceDialog.vue';
import ConfirmDialog from './components/dialogs/ConfirmDialog.vue';
import DesignDialog from './components/dialogs/DesignDialog.vue';
import EffectImageViewer from './components/dialogs/EffectImageViewer.vue';
import NoticeDialog from './components/dialogs/NoticeDialog.vue';
import RuntimeLogDialog from './components/dialogs/RuntimeLogDialog.vue';
import TemplateEditorDialog from './components/dialogs/TemplateEditorDialog.vue';
import DataTransferDialog from './components/dialogs/DataTransferDialog.vue';
import { fileName } from './lib/formatters';
import { taskReferencePaths } from './lib/libraryFormat';
import { deepClone, defaultSettings, emptyTemplate, normalizeSettingsForUi } from './lib/models';
import {
  clipboardHasImage,
  extractClipboardFilePaths,
  extractDroppedFilePaths,
} from './lib/referenceFiles';
import { installAutoHideScrollbars } from './lib/scrollbarVisibility';
import { DEFAULT_PROMPT_MODE, DEFAULT_RATIO } from './lib/options';
import { themeOverrides } from './lib/theme';
import {
  listenDragDrop,
  listenEvent,
  listenWindowState,
  openDialog,
  restoreWindowState,
  saveDialog,
} from './tauri';
import * as api from './api/index.js';
import logoUrl from './assets/title.png';

const statusText = ref('启动中');
const statusTone = ref('busy');

// 密码锁（仅 Web 版生效）
const isWeb = !window.__TAURI_INTERNALS__;
const AUTH_KEY = 'if_auth';
const ACCESS_PASSWORD = import.meta.env.VITE_ACCESS_PASSWORD || 'image-forge';
const unlocked = ref(!isWeb || localStorage.getItem(AUTH_KEY) === ACCESS_PASSWORD);
const lockPassword = ref('');
const lockError = ref('');

function unlock() {
  if (lockPassword.value.trim() === ACCESS_PASSWORD) {
    localStorage.setItem(AUTH_KEY, ACCESS_PASSWORD);
    unlocked.value = true;
    lockError.value = '';
  } else {
    lockError.value = '密码错误';
    lockPassword.value = '';
  }
}
const agentSessions = ref([]);
const currentAgentSessionId = ref('');
const agentBusy = ref(false);
const agentStreamText = ref('');
const agentToolStatus = ref('');
const agentAnswers = ref({});
const agentAttachments = ref([]);
const templateFillBusy = ref(false);
const agentPrefillPrompt = ref('');
const settings = ref(defaultSettings());
const history = ref([]);
const agentLibraryVersion = ref(0);
const queue = reactive({
  waiting: [],
  running: [],
  recent: [],
  workerActive: false,
  updatedAt: '',
});
const templates = ref([]);
const templateDraftReferences = ref([]);
const templateDraftEffectImage = ref(null);
const templateDraftDragActive = ref(false);

const showApiDialog = ref(false);
const showTemplateEditor = ref(false);
const showDataTransfer = ref(false);
const dataTransferMode = ref('export');
const showDesignDialog = ref(false);
const showRuntimeLogDialog = ref(false);
const showCleanupDialog = ref(false);
const confirmation = reactive({
  visible: false,
  title: '请确认',
  message: '',
  resolve: null,
});
const notice = reactive({
  visible: false,
  title: '提示',
  message: '',
  buttonText: '确认',
  resolve: null,
});
const effectViewer = reactive({ show: false, path: '', title: '', items: [], index: 0 });
const ACTIVE_QUEUE_POLL_INTERVAL = 5000;
const AGENT_TASK_GROUP_POLL_INTERVAL = 5000;

const templateDraft = reactive(emptyTemplate());
const templateEditorMode = ref('edit');
const aboutInfo = ref({ version: '', buildTime: '' });
const runtimeLogText = ref('');
const cleanupCandidates = ref([]);
const cleanupLoading = ref(false);
const cleanupConfirming = ref(false);
const cleanupError = ref('');

const form = reactive({
  providerId: '',
  chatProviderId: '',
  promptMode: DEFAULT_PROMPT_MODE,
  resolution: '4k',
  ratio: DEFAULT_RATIO,
  quality: 'medium',
});

let pollTimer = 0;
let removeScrollbarVisibility = null;
let unlistenDragDrop = null;
let unlistenQueueUpdated = null;
let unlistenAgentProgress = null;
let unlistenAgentTaskGroup = null;
let unlistenWindowState = null;
let unlistenMenuOpenSettings = null;
let queueRefreshInFlight = false;
let queueRefreshQueued = false;
let agentTaskGroupPollTimer = 0;
let agentTaskGroupRefreshInFlight = false;

const imageProviders = computed(() =>
  settings.value.providers.filter((provider) => provider.modelType !== 'chat')
);

const chatProviders = computed(() =>
  settings.value.providers.filter((provider) => provider.modelType === 'chat')
);

const imageProviderOptions = computed(() =>
  imageProviders.value.map((provider) => ({
    label: modelOptionLabel(provider),
    value: provider.id,
  }))
);

const chatProviderOptions = computed(() =>
  chatProviders.value.map((provider) => ({
    label: modelOptionLabel(provider),
    value: provider.id,
  }))
);

const activeProvider = computed(
  () =>
    imageProviders.value.find((provider) => provider.id === form.providerId) ||
    imageProviders.value.find((provider) => provider.id === settings.value.activeImageProviderId) ||
    imageProviders.value[0]
);

const activeChatProvider = computed(
  () =>
    chatProviders.value.find((provider) => provider.id === form.chatProviderId) ||
    chatProviders.value.find((provider) => provider.id === settings.value.activeChatProviderId) ||
    chatProviders.value[0]
);

const currentAgentSession = computed(
  () => agentSessions.value.find((session) => session.id === currentAgentSessionId.value) || null
);

const currentAgentMessages = computed(() => currentAgentSession.value?.messages || []);

const historyTimeline = computed(() => {
  const byId = new Map();
  for (const task of [...history.value, ...queue.running, ...queue.waiting]) {
    if (task?.id) byId.set(task.id, task);
  }
  return Array.from(byId.values()).sort((left, right) =>
    taskTime(left).localeCompare(taskTime(right))
  );
});

const currentAgentDisplayMessages = computed(() =>
  currentAgentMessages.value.map((message) => {
    const group = message.taskGroup;
    if (!group?.id) return message;
    const taskIds = new Set(group.taskIds || []);
    const tasks = historyTimeline.value.filter(
      (task) => taskIds.has(task.id) || task.taskGroupId === group.id
    );
    const images = tasks.flatMap((task) =>
      (task.outputs || []).map((output) => ({
        ...output,
        title: task.prompt || output.fileName || '生成图片',
        meta: [output.size || task.params?.size, task.model].filter(Boolean).join(' · '),
      }))
    );
    return {
      ...message,
      taskGroup: { ...group, images },
    };
  })
);

onMounted(async () => {
  removeScrollbarVisibility = installAutoHideScrollbars();
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
    unlistenQueueUpdated = await listenEvent('queue-updated', handleQueueUpdatedEvent);
  } catch {
    // 预览环境可能没有事件通道。
  }
  try {
    unlistenAgentProgress = await listenEvent('agent-progress', handleAgentProgressEvent);
  } catch {
    unlistenAgentProgress = api.onAgentEvent((event, payload) => {
      if (event === 'agent-progress') handleAgentProgressEvent({ payload });
    });
  }
  try {
    unlistenAgentTaskGroup = await listenEvent('agent-task-group', handleAgentTaskGroupEvent);
  } catch {
    const unlistenTaskGroup = api.onAgentEvent((event, payload) => {
      if (event === 'agent-task-group') handleAgentTaskGroupEvent({ payload });
    });
    // 合并清理
    const prev = unlistenAgentProgress;
    unlistenAgentProgress = () => {
      prev();
      unlistenTaskGroup();
    };
  }
  try {
    unlistenMenuOpenSettings = await listenEvent('menu-open-settings', () => {
      void openDesign();
    });
  } catch {
    // 预览环境可能没有菜单事件。
  }
  await refreshAll();
  await refreshAgentSessions();
  syncAgentTaskGroupPolling();
});

onUnmounted(() => {
  window.clearInterval(pollTimer);
  window.clearInterval(agentTaskGroupPollTimer);
  unlistenDragDrop?.();
  unlistenQueueUpdated?.();
  unlistenAgentProgress?.();
  unlistenAgentTaskGroup?.();
  unlistenMenuOpenSettings?.();
  unlistenWindowState?.();
  removeScrollbarVisibility?.();
});

// 首次加载或重大变更后，重新拉取设置、历史、队列和模板。
async function refreshAll() {
  try {
    const state = await api.loadAppState();
    applyState(state);
    setStatus('就绪', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function refreshAgentSessions() {
  try {
    const list = await api.listAgentSessions();
    agentSessions.value = sortAgentSessions(Array.isArray(list) ? list : []);
    if (!currentAgentSessionId.value && agentSessions.value[0]) {
      currentAgentSessionId.value = agentSessions.value[0].id;
      form.chatProviderId = agentSessions.value[0].modelProviderId || form.chatProviderId;
    }
    if (!agentSessions.value.length) await createAgentConversation();
    if (currentAgentSessionId.value) {
      await refreshAgentTaskGroups();
      syncAgentTaskGroupPolling();
    } else {
      syncAgentTaskGroupPolling();
    }
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

function sortAgentSessions(sessions) {
  return [...sessions].sort((a, b) => {
    const aTime = a.updatedAt || a.createdAt || '';
    const bTime = b.updatedAt || b.createdAt || '';
    return aTime.localeCompare(bTime);
  });
}

function setAgentSession(session) {
  agentSessions.value = sortAgentSessions([
    ...agentSessions.value.filter((item) => item.id !== session.id),
    session,
  ]);
}

async function createAgentConversation() {
  try {
    const session = await api.createAgentSession(form.chatProviderId || '');
    setAgentSession(session);
    currentAgentSessionId.value = session.id;
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function selectAgentConversation(sessionId) {
  try {
    const session = await api.getAgentSession(sessionId);
    setAgentSession(session);
    currentAgentSessionId.value = session.id;
    form.chatProviderId = session.modelProviderId || form.chatProviderId;
    agentStreamText.value = '';
    agentToolStatus.value = '';
    await refreshAgentTaskGroups();
    syncAgentTaskGroupPolling();
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function deleteAgentConversation(sessionId) {
  // 空会话（没有任何消息）不弹确认框，直接删除
  const target = agentSessions.value.find((item) => item.id === sessionId);
  const isEmptySession = Boolean(target) && !(target.messages || []).length;
  if (!isEmptySession) {
    const confirmed = await requestConfirmation(
      '删除 Agent 对话',
      '确认把这个 Agent 对话移入系统回收站？关联的绘图任务不会删除。'
    );
    if (!confirmed) return;
  }
  try {
    await api.deleteAgentSession(sessionId);
    if (currentAgentSessionId.value === sessionId) currentAgentSessionId.value = '';
    await refreshAgentSessions();
    setStatus(isEmptySession ? '空对话已删除' : 'Agent 对话已移入回收站', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function renameAgentConversation({ sessionId, title }) {
  try {
    const session = await api.renameAgentSession(sessionId, title);
    setAgentSession(session);
    setStatus('对话标题已更新', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

// 乐观显示：发送后立刻把用户消息放到屏幕上，不等模型响应；
// 本轮结束后 setAgentSession / selectAgentConversation 会用后端数据覆盖。
function appendOptimisticUserMessage(content) {
  const sessionId = currentAgentSessionId.value;
  if (!sessionId || !content) return;
  agentSessions.value = agentSessions.value.map((session) =>
    session.id === sessionId
      ? {
          ...session,
          messages: [
            ...(session.messages || []),
            {
              id: `optimistic-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`,
              role: 'user',
              status: 'user',
              content,
              attachments: [],
              toolCall: null,
              questions: [],
              taskGroup: null,
              error: '',
              createdAt: new Date().toISOString(),
            },
          ],
        }
      : session
  );
}

async function sendAgentConversationMessage(payload) {
  const content = typeof payload === 'string' ? payload : payload?.content || '';
  const drawThisTurn = typeof payload !== 'string' && Boolean(payload?.drawThisTurn);
  if (!currentAgentSessionId.value) await createAgentConversation();
  if (!currentAgentSessionId.value) return;
  appendOptimisticUserMessage(content);
  if (drawThisTurn) {
    await createAgentDrawingTask(content);
    return;
  }
  agentBusy.value = true;
  agentStreamText.value = '';
  try {
    const session = await api.sendAgentMessage(
      currentAgentSessionId.value,
      form.chatProviderId,
      content,
      agentAttachments.value.map(({ dataUrl, ...attachment }) => attachment)
    );
    setAgentSession(session);
    agentAttachments.value = [];
    await refreshAgentTaskGroups();
    syncAgentTaskGroupPolling();
  } catch (error) {
    setStatus(String(error), 'error');
    if (currentAgentSessionId.value) {
      await selectAgentConversation(currentAgentSessionId.value);
    }
  } finally {
    agentBusy.value = false;
    agentStreamText.value = '';
    agentToolStatus.value = '';
  }
}

async function createAgentDrawingTask(content) {
  if (!activeProvider.value?.apiKey) {
    setStatus('请先在 API 源里填写生图模型 API Key', 'error');
    showApiDialog.value = true;
    return;
  }
  const attachments = agentAttachments.value.map(({ dataUrl, ...attachment }) => attachment);
  agentBusy.value = true;
  try {
    await api.createAgentDirectImageTask(currentAgentSessionId.value, content, attachments, {
      title: content.split(/\r?\n/, 1)[0].slice(0, 32) || '直接绘画',
      prompt: content,
      providerId: activeProvider.value.id,
      resolution: form.resolution,
      ratio: form.ratio,
      quality: form.quality,
      promptFidelity: form.promptMode,
      referencePolicy: attachments.length ? 'use' : 'none',
      referenceIds: attachments.map((attachment) => attachment.id),
    });
    agentAttachments.value = [];
    await refreshQueueOnly();
    await selectAgentConversation(currentAgentSessionId.value);
    setStatus('绘画任务已加入队列', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
    await selectAgentConversation(currentAgentSessionId.value);
  } finally {
    agentBusy.value = false;
  }
}

async function stopAgentConversation() {
  if (currentAgentSessionId.value) {
    try {
      await api.cancelAgentTurn(currentAgentSessionId.value);
      await selectAgentConversation(currentAgentSessionId.value);
    } catch (error) {
      setStatus(String(error), 'error');
    }
  }
  agentBusy.value = false;
  agentStreamText.value = '';
}

async function addAgentReferenceImages() {
  const selected = await openDialog({
    multiple: true,
    filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
  });
  const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
  await addAgentReferencePaths(paths);
}

async function addAgentReferencePaths(paths) {
  for (const path of paths || []) {
    try {
      const preview = await api.referenceFromPath(path);
      agentAttachments.value.push({
        id: createAgentAttachmentId(),
        path: preview.path,
        fileName: preview.fileName,
        mimeType: preview.mimeType,
        dataUrl: preview.dataUrl,
      });
    } catch (error) {
      setStatus(String(error), 'error');
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
    const preview = await api.referenceFromClipboard();
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

async function handleLibraryReferenceToAgent({ task }) {
  // 添加生成此图时使用的所有参考图（双端字段形态兼容）
  const refPaths = taskReferencePaths(task);
  if (refPaths.length) {
    for (const path of refPaths) {
      try {
        const preview = await api.referenceFromPath(path);
        if (!agentAttachments.value.some((item) => item.path === preview.path)) {
          agentAttachments.value.push({
            id: createAgentAttachmentId(),
            path: preview.path,
            fileName: preview.fileName,
            mimeType: preview.mimeType,
            dataUrl: preview.dataUrl,
          });
        }
      } catch (error) {
        setStatus(`参考图加载失败：${error}`, 'error');
      }
    }
  }
  // 预填提示词到输入框
  agentPrefillPrompt.value = task.prompt || '';
  // 清除预填（避免下次重复触发）
  setTimeout(() => {
    agentPrefillPrompt.value = '';
  }, 100);
  setStatus(
    `已引用到当前对话（${refPaths.length ? refPaths.length + ' 张参考图' : '无参考图'}）`,
    'ok'
  );
}

async function handleLibraryAddToTemplate({ task }) {
  Object.assign(templateDraft, emptyTemplate());
  templateDraft.title = '';
  templateDraft.prompt = task.prompt || '';
  templateDraft.referencePaths = [];
  templateDraftReferences.value = [];
  templateDraftEffectImage.value = null;
  // 添加生成此图时使用的所有参考图（双端字段形态兼容）
  const refPaths = taskReferencePaths(task);
  if (refPaths.length) {
    for (const path of refPaths) {
      try {
        const preview = await api.referenceFromPath(path);
        templateDraftReferences.value.push({ ...preview, previewUrl: preview.dataUrl });
      } catch {
        // 参考图加载失败不阻塞模板创建
      }
    }
  }
  templateEditorMode.value = 'new';
  showTemplateEditor.value = true;
  setStatus('已添加到模板编辑器', 'ok');
}

async function handleApplyTemplate({ template }) {
  if (!template) return;
  const refPaths = template.referencePaths || template.reference_paths || [];
  for (const path of refPaths) {
    try {
      const preview = await api.referenceFromPath(path);
      if (!agentAttachments.value.some((item) => item.path === preview.path)) {
        agentAttachments.value.push({
          id: createAgentAttachmentId(),
          path: preview.path,
          fileName: preview.fileName,
          mimeType: preview.mimeType,
          dataUrl: preview.dataUrl,
        });
      }
    } catch {
      // 模板参考图加载失败不阻塞插入
    }
  }
  setStatus(
    `已插入模板「${template.title || '未命名模板'}」${
      refPaths.length ? `（${refPaths.length} 张参考图）` : ''
    }`,
    'ok'
  );
}

async function handleFillTemplate({ template }) {
  const content = String(template?.content || '').trim();
  if (!content) return;
  templateFillBusy.value = true;
  try {
    const filled = await api.fillPromptTemplate(
      currentAgentSessionId.value || '',
      form.chatProviderId,
      content
    );
    agentPrefillPrompt.value = filled;
    setTimeout(() => {
      agentPrefillPrompt.value = '';
    }, 100);
    setStatus('AI 已填充模板占位符', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  } finally {
    templateFillBusy.value = false;
  }
}

function handleAgentProgressEvent(event) {
  const payload = event?.payload || {};
  if (payload.sessionId !== currentAgentSessionId.value) return;
  if (payload.phase === 'delta') agentStreamText.value += payload.chunk || '';
  if (['tool_delta', 'tool_start', 'tool_result'].includes(payload.phase)) {
    const tool = payload.toolName ? ` · ${payload.toolName}` : '';
    agentToolStatus.value = `${payload.message || '正在执行工具'}${tool}`;
  }
  if (payload.phase === 'error') setStatus(payload.message || 'Agent 调用失败', 'error');
}

function openAgentTaskGroup(group) {
  void group;
  void refreshQueueOnly();
}

function openImageViewer({ items = [], index = 0 } = {}) {
  if (!items.length) return;
  effectViewer.path = '';
  effectViewer.title = '生成图片';
  effectViewer.items = items;
  effectViewer.index = index;
  effectViewer.show = true;
}

async function cancelAgentTaskGroup(group) {
  if (!group?.id) return;
  const confirmed = await requestConfirmation(
    '取消任务组',
    '确认取消这个 Agent 任务组？已完成的任务不会删除，失败项仍可重试。'
  );
  if (!confirmed) return;
  try {
    await api.cancelAgentTaskGroup(group.id);
    await refreshQueueOnly();
    if (currentAgentSessionId.value) await selectAgentConversation(currentAgentSessionId.value);
    await refreshAgentTaskGroups();
    setStatus('Agent 任务组已取消', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function handleRedrawTaskGroup(group) {
  const taskId = group?.taskIds?.[0];
  if (!taskId) return;
  try {
    await api.redrawTask(taskId);
    await refreshQueueOnly();
    if (currentAgentSessionId.value) await selectAgentConversation(currentAgentSessionId.value);
    await refreshAgentTaskGroups();
    setStatus('已按原参数重新排队', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function retryAgentTaskGroup(group) {
  if (!group?.id) return;
  try {
    await api.retryAgentTaskGroup(group.id);
    await refreshQueueOnly();
    if (currentAgentSessionId.value) await selectAgentConversation(currentAgentSessionId.value);
    await refreshAgentTaskGroups();
    setStatus('Agent 失败任务已重新排队', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

function retryAgentMessage(message) {
  const messages = currentAgentMessages.value;
  const index = messages.findIndex((item) => item.id === message?.id);
  const previousUser = messages
    .slice(0, index < 0 ? messages.length : index)
    .reverse()
    .find((item) => item.role === 'user');
  if (previousUser?.content) {
    void sendAgentConversationMessage({ content: previousUser.content, drawThisTurn: false });
  }
}

function updateAgentAnswer({ key, value }) {
  agentAnswers.value = { ...agentAnswers.value, [key]: value };
}

function answerAgentQuestions(message) {
  const content = (message.questions || [])
    .map(
      (question) => `${question.label}：${String(agentAnswers.value[question.key] || '').trim()}`
    )
    .join('\n');
  if (content.trim()) void sendAgentConversationMessage(content);
  agentAnswers.value = {};
}

async function handleAgentTaskGroupEvent(event) {
  const group = event?.payload;
  if (!group || group.sessionId !== currentAgentSessionId.value) return;
  await refreshQueueOnly();
  await refreshAgentTaskGroups();
  syncAgentTaskGroupPolling();
  setStatus(`Agent 已创建 ${group.tasks?.length || 0} 个绘图任务`, 'ok');
}

function singleLine(value) {
  return String(value || '')
    .replace(/\s+/g, ' ')
    .trim();
}

async function refreshAgentTaskGroups({ silent = true } = {}) {
  if (agentTaskGroupRefreshInFlight) return;
  const sessionId = currentAgentSessionId.value;
  const groups = currentAgentTaskGroups();
  if (!sessionId || !groups.length) return;
  agentTaskGroupRefreshInFlight = true;
  try {
    const updates = await Promise.all(
      groups.map(async (group) => {
        try {
          const tasks = await api.getTaskStatus(group.id, '');
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
            status: 'missing',
            taskIds: [],
            titles: [String(error)],
          };
        }
      })
    );
    if (sessionId !== currentAgentSessionId.value) return;
    applyAgentTaskGroupUpdates(sessionId, updates);
  } catch (error) {
    if (!silent) setStatus(String(error), 'error');
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

function summarizeTaskGroupStatus(tasks, fallback = 'queued') {
  const statuses = tasks.map((task) => task.status).filter(Boolean);
  if (!statuses.length) return fallback || 'queued';
  if (statuses.some((status) => status === 'cancelling')) return 'cancelling';
  if (statuses.some((status) => status === 'running')) return 'running';
  if (statuses.some((status) => status === 'queued')) return 'queued';
  if (statuses.every((status) => status === 'completed')) return 'completed';
  if (statuses.some((status) => status === 'failed')) return 'failed';
  if (statuses.some((status) => status === 'cancelled')) return 'cancelled';
  if (statuses.some((status) => status === 'missing')) return 'missing';
  return statuses[0] || fallback || 'queued';
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
  return ['completed', 'failed', 'cancelled', 'missing'].includes(status);
}

// 只在队列活跃期间保留兜底轮询，平时由后端事件驱动刷新。
async function refreshQueueOnly({ silent = true } = {}) {
  if (queueRefreshInFlight) {
    queueRefreshQueued = true;
    return null;
  }
  queueRefreshInFlight = true;
  try {
    const snapshot = await api.queueSnapshot();
    applyQueue(snapshot);
    return snapshot;
  } catch (error) {
    if (!silent) setStatus(String(error), 'error');
    return null;
  } finally {
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
    snapshot.waiting?.length || 0 || snapshot.running?.length || 0 || snapshot.workerActive
  );
}

// 把 Rust 返回的完整状态归一化为前端响应式状态。
function applyState(state) {
  settings.value = normalizeSettingsForUi(state.settings || defaultSettings());
  history.value = state.history || [];
  applyQueue(state.queue || {});
  templates.value = state.templates || [];
  ensureSelectedModels();
}

// 合并队列快照，并用后端 recent 字段刷新左侧历史时间线。
function applyQueue(snapshot) {
  queue.waiting = snapshot.waiting || [];
  queue.running = snapshot.running || [];
  queue.recent = snapshot.recent || [];
  queue.workerActive = Boolean(snapshot.workerActive);
  queue.updatedAt = snapshot.updatedAt || '';
  if (snapshot.recent) {
    history.value = snapshot.recent;
  }
  syncQueuePolling();
}

async function chooseReferenceImages(target, successMessage) {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    await addReferencePaths(target, paths, successMessage);
  } catch (error) {
    setStatus(String(error), 'error');
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
        const preview = await api.referenceFromPath(path);
        if (appendReferencePreview(target, preview)) added += 1;
      } catch (error) {
        lastError = error;
      }
    }
    if (added) setStatus(`${successMessage}：${added} 张`, 'ok');
    if (!added && lastError && !options.silentInvalid) setStatus(String(lastError), 'error');
  } catch (error) {
    setStatus(String(error), 'error');
  }
  return added;
}

function handleReferenceDragDrop(event) {
  const payload = event?.payload || {};
  const target = referenceDropTarget(payload.position) || defaultReferenceDropTarget();
  if (payload.type === 'enter' || payload.type === 'over') {
    setReferenceDragTarget(target);
    return;
  }
  if (payload.type === 'leave') {
    clearReferenceDragTargets();
    return;
  }
  clearReferenceDragTargets();
  if (payload.type === 'drop' && payload.paths?.length) {
    void addDraggedReferencePaths(target, payload.paths);
  }
}

function handleTemplateDraftDropEvent(event) {
  clearReferenceDragTargets();
  const paths = extractDroppedFilePaths(event?.dataTransfer);
  if (paths.length) void addDraggedReferencePaths('template-draft', paths);
}

function addDraggedReferencePaths(target, paths) {
  if (target === 'template-draft') {
    return addReferencePathsWithOptions(templateDraftReferences, paths, '已添加模板参考图', {
      silentInvalid: true,
    });
  }
  return addAgentReferencePaths(paths);
}

function referenceDropTarget(position) {
  const x = Number(position?.x);
  const y = Number(position?.y);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return false;
  const scale = window.devicePixelRatio || 1;
  for (const [left, top] of [
    [x, y],
    [x / scale, y / scale],
  ]) {
    const zone = document.elementFromPoint(left, top)?.closest('[data-reference-drop-target]');
    if (zone?.dataset.referenceDropTarget) return zone.dataset.referenceDropTarget;
  }
  return '';
}

function defaultReferenceDropTarget() {
  if (showTemplateEditor.value && templateEditorMode.value !== 'view') return 'template-draft';
  return 'agent';
}

function setReferenceDragTarget(target) {
  templateDraftDragActive.value = target === 'template-draft';
}

function clearReferenceDragTargets() {
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
  const hasImage = [...items, ...files].some((item) => item.type?.startsWith('image/'));
  const hasFilePayload = files.length > 0 || types.includes('Files');
  const hasText = ['text/plain', 'text/uri-list'].some((type) => {
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
    const preview = await api.referenceFromClipboard();
    if (!preview) {
      setStatus('剪贴板中没有可用图片', 'error');
      return false;
    }
    if (!appendReferencePreview(target, preview)) {
      setStatus('剪贴板图片已经添加', 'busy');
      return false;
    }
    setStatus(successMessage, 'ok');
    return true;
  } catch (error) {
    setStatus(String(error), 'error');
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
      const preview = await api.referenceFromPath(path);
      restored.push({ ...preview, previewUrl: preview.dataUrl });
    } catch {
      missing += 1;
    }
  }
  return { restored, missing };
}

// 删除历史记录，同时由后端负责把对应输出图移入回收站。
async function deleteTask(task) {
  const confirmed = await requestConfirmation(
    '删除历史任务',
    '确认删除这条生成记录？对应图片会移入系统回收站。'
  );
  if (!confirmed) return;
  try {
    await api.deleteTask(task.id);
    setStatus('生成记录已删除', 'ok');
    await refreshAll();
    agentLibraryVersion.value += 1;
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

// 保存 API 源配置后，重新选择可用的生图和对话模型。
async function saveApiSettings(nextSettings) {
  try {
    const saved = await api.saveSettings(nextSettings);
    settings.value = normalizeSettingsForUi(saved);
    ensureSelectedModels(true);
    setStatus('API 源已保存', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function selectApiProvider(kind, providerId) {
  const providers = settings.value.providers.slice();
  const index = providers.findIndex((provider) => provider.id === providerId);
  if (index > 0) {
    const [item] = providers.splice(index, 1);
    // 默认 API = 同类列表第一项，切换时把它挪到最前。
    const insertAt = providers.findIndex((provider) =>
      kind === 'chat' ? provider.modelType === 'chat' : provider.modelType !== 'chat'
    );
    providers.splice(insertAt < 0 ? 0 : insertAt, 0, item);
    settings.value.providers = providers;
  }
  if (kind === 'image') {
    form.providerId = providerId;
    settings.value.activeImageProviderId = providerId;
    settings.value.activeProviderId = providerId;
  } else {
    form.chatProviderId = providerId;
    settings.value.activeChatProviderId = providerId;
  }
  try {
    const saved = await api.saveSettings(deepClone(settings.value));
    settings.value = normalizeSettingsForUi(saved);
    setStatus('当前 API 已切换', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

// 下载输出图到系统 Downloads，并立即在 Finder 中定位。
async function downloadOutput(output) {
  try {
    const savedPath = await api.downloadOutput(output.path);
    setStatus(`已保存到下载目录：${fileName(savedPath)}`, 'ok');
    await reveal(savedPath);
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

// 打开空白模板编辑器。
function newTemplate() {
  Object.assign(templateDraft, emptyTemplate());
  templateDraftReferences.value = [];
  templateDraftEffectImage.value = null;
  templateEditorMode.value = 'new';
  showTemplateEditor.value = true;
}

// 以只读模式查看模板，并在弹窗中高亮占位符。
async function viewTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  const { restored } = await restoreReferencePreviews(template.referencePaths);
  templateDraftReferences.value = restored;
  templateDraftEffectImage.value = await restoreEffectImage(template.effectImagePath);
  templateEditorMode.value = 'view';
  showTemplateEditor.value = true;
}

// 以编辑模式打开模板。
async function editTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  const { restored } = await restoreReferencePreviews(template.referencePaths);
  templateDraftReferences.value = restored;
  templateDraftEffectImage.value = await restoreEffectImage(template.effectImagePath);
  templateEditorMode.value = 'edit';
  showTemplateEditor.value = true;
}

// 保存新增或编辑后的模板，并刷新模板列表。
async function savePromptTemplate() {
  try {
    templateDraft.referencePaths = templateDraftReferences.value.map((item) => item.path);
    templateDraft.effectImagePath = templateDraftEffectImage.value?.path || '';
    templates.value = await api.saveTemplate(deepClone(templateDraft));
    showTemplateEditor.value = false;
    setStatus('模板已保存', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function addTemplateDraftReferenceImages() {
  await chooseReferenceImages(templateDraftReferences, '已添加模板参考图');
}

async function addTemplateDraftEffectImage() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return;
    const preview = await api.referenceFromPath(path);
    templateDraftEffectImage.value = { ...preview, previewUrl: preview.dataUrl };
    setStatus('已添加模板效果图', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function pasteTemplateDraftEffectImage() {
  try {
    const preview = await api.referenceFromClipboard();
    if (!preview) {
      setStatus('剪贴板中没有可用图片', 'error');
      return;
    }
    templateDraftEffectImage.value = { ...preview, previewUrl: preview.dataUrl };
    setStatus('已从剪贴板添加模板效果图', 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function handleTemplateDraftPaste(event) {
  await pasteReferenceImage(event, templateDraftReferences, '已从剪贴板添加模板参考图');
}

// 让用户选择保存位置，并导出包含 Markdown 和参考图资源的模板 ZIP。
// Web 版没有保存对话框，直接生成与桌面版同构的 ZIP 并触发浏览器下载。
async function exportPromptTemplates() {
  if (!templates.value.length) {
    setStatus('没有可导出的模板', 'error');
    return;
  }
  try {
    if (isWeb) {
      const savedName = await api.exportTemplates('ImageForge-templates.zip');
      setStatus(`模板已导出：${fileName(savedName)}`, 'ok');
      return;
    }
    const destination = await saveDialog({
      defaultPath: 'ImageForge-templates.zip',
      filters: [{ name: 'ZIP 压缩包', extensions: ['zip'] }],
    });
    if (!destination) return;
    const savedPath = await api.exportTemplates(destination);
    setStatus(`模板已导出：${fileName(savedPath)}`, 'ok');
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

// Web 版用浏览器文件选择器挑 ZIP；桌面版走系统文件对话框。
function pickWebTemplateArchive() {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.zip';
    input.onchange = () => resolve(input.files && input.files.length ? input.files[0] : null);
    input.click();
  });
}

// 从 Image Forge 模板包导入提示词和参考图，重复模板由后端自动跳过。
async function importPromptTemplates() {
  try {
    let archivePath = null;
    if (isWeb) {
      archivePath = await pickWebTemplateArchive();
    } else {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'Image Forge 模板包', extensions: ['zip'] }],
      });
      archivePath = Array.isArray(selected) ? selected[0] : selected;
    }
    if (!archivePath) return;
    const result = await api.importTemplates(archivePath);
    templates.value = result.templates || [];
    const message = `导入 ${result.importedCount || 0} 个，重复 ${result.skippedCount || 0} 个。`;
    await showNotice('模板导入结果', message);
    setStatus(message, 'ok');
  } catch (error) {
    const message = String(error);
    await showNotice('模板导入失败', message);
    setStatus(message, 'error');
  }
}

// 删除模板维护列表中的指定模板。
async function deletePromptTemplate(id) {
  const confirmed = await requestConfirmation(
    '删除模板',
    '确认删除这个模板？未被其它记录引用的参考图会移入系统回收站。'
  );
  if (!confirmed) return;
  try {
    templates.value = await api.deleteTemplate(id);
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

// 交换当前模板与搜索结果中相邻模板的位置，并持久化完整模板顺序。
async function movePromptTemplate({ templateId, targetTemplateId }) {
  try {
    templates.value = await api.moveTemplate(templateId, targetTemplateId);
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

async function restoreEffectImage(path) {
  if (!path) return null;
  try {
    const preview = await api.referenceFromPath(path);
    return { ...preview, previewUrl: preview.dataUrl };
  } catch {
    return null;
  }
}

function showTemplateEffect(template) {
  if (!template?.effectImagePath) return;
  effectViewer.items = [];
  effectViewer.index = 0;
  effectViewer.path = template.effectImagePath;
  effectViewer.title = `${template.title || '模板'} · 效果图`;
  effectViewer.show = true;
}

async function openDesign() {
  showDesignDialog.value = true;
  try {
    aboutInfo.value = await api.aboutInfo();
  } catch {
    aboutInfo.value = {
      version: '',
      buildTime: '',
    };
  }
}

async function openRuntimeLogs() {
  try {
    runtimeLogText.value = await api.runtimeLogs();
  } catch (error) {
    runtimeLogText.value = `读取运行日志失败：${String(error)}`;
  }
  showRuntimeLogDialog.value = true;
}

async function openCleanup() {
  cleanupCandidates.value = [];
  cleanupError.value = '';
  cleanupLoading.value = true;
  showCleanupDialog.value = true;
  try {
    cleanupCandidates.value = await api.scanCleanupCandidates();
  } catch (error) {
    cleanupError.value = String(error);
  } finally {
    cleanupLoading.value = false;
  }
}

function openExportData() {
  showDesignDialog.value = false;
  dataTransferMode.value = 'export';
  showDataTransfer.value = true;
}

function openImportData() {
  showDesignDialog.value = false;
  dataTransferMode.value = 'import';
  showDataTransfer.value = true;
}

async function confirmCleanup() {
  cleanupConfirming.value = true;
  cleanupError.value = '';
  try {
    const removed = await api.cleanupDataFiles();
    cleanupCandidates.value = [];
    showCleanupDialog.value = false;
    setStatus(`已清理 ${removed.length} 个孤岛文件`, 'ok');
  } catch (error) {
    cleanupError.value = String(error);
  } finally {
    cleanupConfirming.value = false;
  }
}

// 调用系统文件管理器定位文件或目录。
async function reveal(path) {
  try {
    await api.revealPath(path);
  } catch (error) {
    setStatus(String(error), 'error');
  }
}

function setStatus(message, tone = 'idle') {
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

function showNotice(title, message, buttonText = '确认') {
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
  return `${provider.name} · ${provider.imageModel || '未设置模型'}`;
}

function taskTime(task) {
  return task.createdAt || task.updatedAt || task.completedAt || '';
}

// 根据设置、历史成功任务和当前列表，保证模型选择始终可用。
function ensureSelectedModels(preferSaved = false) {
  if (preferSaved || !imageProviders.value.some((provider) => provider.id === form.providerId)) {
    form.providerId = preferSaved
      ? settings.value.activeImageProviderId ||
        settings.value.activeProviderId ||
        imageProviders.value[0]?.id ||
        ''
      : lastSuccessfulImageProviderId() || imageProviders.value[0]?.id || '';
  }
  if (preferSaved || !chatProviders.value.some((provider) => provider.id === form.chatProviderId)) {
    form.chatProviderId = settings.value.activeChatProviderId || chatProviders.value[0]?.id || '';
  }
}

// 从最近成功任务中找回生图模型，作为工作台默认选择。
function lastSuccessfulImageProviderId() {
  for (const task of [...history.value].reverse()) {
    if (task.status !== 'completed') continue;
    const provider =
      imageProviders.value.find((item) => item.id === task.providerId) ||
      imageProviders.value.find(
        (item) => item.name === task.providerName && item.imageModel === task.model
      ) ||
      imageProviders.value.find((item) => item.imageModel === task.model);
    if (provider) return provider.id;
  }
  return '';
}
</script>
