<template>
  <n-config-provider :theme-overrides="themeOverrides" component-size="small">
    <n-global-style />
    <main class="app">
      <AppTopbar
        @show-api="showApiDialog = true"
        @show-template-manager="showTemplateManagerDialog = true"
        @show-about="openAbout"
      />

      <section class="workspace" :style="workspaceStyle">
        <QueuePanel
          :filtered-history="filteredHistory"
          :selected-task-id="selectedTaskId"
          :history-query="historyQuery"
          :scroll-request="historyScrollRequest"
          @select-task="selectedTaskId = $event"
          @update:history-query="historyQuery = $event"
          @reuse="reuseTask"
          @refresh-task="refreshTask"
          @retry="retryTask"
          @delete="deleteTask"
          @copy-output="copyOutput"
          @download-output="downloadOutput"
          @reveal-output="reveal($event.path)"
        />

        <div
          class="panel-resizer"
          role="separator"
          aria-label="调整队列和结果预览宽度"
          @pointerdown="startPanelResize('queue', $event)"
        ></div>

        <ResultPanel
          :selected-task="selectedTask"
          :current-outputs="currentOutputs"
          @show-detail="showTaskDetail = true"
          @reuse="reuseTask"
          @copy-output="copyOutput"
        />

        <div
          class="panel-resizer"
          role="separator"
          aria-label="调整结果预览和工作台宽度"
          @pointerdown="startPanelResize('composer', $event)"
        ></div>

        <ComposerPanel
          :form="form"
          :image-provider-options="imageProviderOptions"
          :references="references"
          :submitting="submitting"
          :reference-drag-active="referenceDragActive"
          @submit="submitTask"
          @show-template="showTemplateReferenceDialog = true"
          @save-template="saveWorkbenchAsTemplate"
          @clear-prompt="clearPrompt"
          @prompt-focus="capturePromptCursor"
          @prompt-cursor="capturePromptCursor"
          @prompt-paste="handlePromptPaste"
          @add-reference="addReferenceImages"
          @remove-reference="removeReference(references, $event)"
          @reference-drag-over="referenceDragActive = true"
          @reference-drag-leave="referenceDragActive = false"
          @drop-reference="handleReferenceDropEvent"
        />
      </section>

      <footer class="status-bar">
        <span class="status-pill" :data-tone="statusTone">{{ statusText }}</span>
        <div class="status-summary">
          <span class="status-meta">当前 API：{{ activeProvider?.name || "未配置" }} · Images API</span>
          <span class="status-count">{{ queue.running.length }} 运行</span>
          <span class="status-count">{{ queue.waiting.length }} 排队</span>
          <span v-if="activeProvider && !activeProvider.apiKey" class="warn-text">API Key 未设置</span>
        </div>
      </footer>

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
        @update:chat-provider-id="form.chatProviderId = $event"
        @update:source-content="updateTemplateReferenceSource"
        @update:generated-content="updateTemplateReferenceGenerated"
        @select-template="selectReferenceTemplate"
        @ai-fill="fillReferenceTemplate"
        @insert="insertReferenceTemplate"
        @add-reference="addTemplateCallReferenceImages"
        @remove-reference="removeReference(templateReferenceReferences, $event)"
      />

      <TemplateEditorDialog
        v-model:show="showTemplateEditor"
        :template="templateDraft"
          :mode="templateEditorMode"
          :references="templateDraftReferences"
          :reference-drag-active="templateDraftDragActive"
          @save="savePromptTemplate"
          @add-reference="addTemplateDraftReferenceImages"
          @remove-reference="removeReference(templateDraftReferences, $event)"
          @paste-reference="handleTemplateDraftPaste"
        @reference-drag-over="templateDraftDragActive = true"
        @reference-drag-leave="templateDraftDragActive = false"
        @drop-reference="handleTemplateDraftDropEvent"
        @update:show="templateDraftDragActive = false"
      />

      <TaskDetailDialog
        v-model:show="showTaskDetail"
        :task="selectedTask"
        @reuse="reuseTask"
      />

      <AboutDialog
        v-model:show="showAboutDialog"
        :info="aboutInfo"
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
    </main>
  </n-config-provider>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import AppTopbar from "./components/AppTopbar.vue";
import ComposerPanel from "./components/ComposerPanel.vue";
import QueuePanel from "./components/QueuePanel.vue";
import ResultPanel from "./components/ResultPanel.vue";
import AboutDialog from "./components/dialogs/AboutDialog.vue";
import ApiSourceDialog from "./components/dialogs/ApiSourceDialog.vue";
import ConfirmDialog from "./components/dialogs/ConfirmDialog.vue";
import NoticeDialog from "./components/dialogs/NoticeDialog.vue";
import TaskDetailDialog from "./components/dialogs/TaskDetailDialog.vue";
import TemplateEditorDialog from "./components/dialogs/TemplateEditorDialog.vue";
import TemplateManagerDialog from "./components/dialogs/TemplateManagerDialog.vue";
import TemplateReferenceDialog from "./components/dialogs/TemplateReferenceDialog.vue";
import { clamp, fileName, statusLabel } from "./lib/formatters";
import { deepClone, defaultSettings, emptyTemplate, normalizeSettingsForUi } from "./lib/models";
import { extractClipboardFilePaths, extractDroppedFilePaths } from "./lib/referenceFiles";
import { installAutoHideScrollbars } from "./lib/scrollbarVisibility";
import {
  DEFAULT_PROMPT_MODE,
  DEFAULT_RATIO,
  orientationForRatio,
  sizeForPreset,
} from "./lib/options";
import { themeOverrides } from "./lib/theme";
import { invoke, listenDragDrop, openDialog, saveDialog } from "./tauri";

const statusText = ref("启动中");
const statusTone = ref("busy");
const settings = ref(defaultSettings());
const history = ref([]);
const queue = reactive({ waiting: [], running: [], recent: [], workerActive: false, updatedAt: "" });
const templates = ref([]);
const references = ref([]);
const referenceDragActive = ref(false);
const templateDraftReferences = ref([]);
const templateDraftDragActive = ref(false);
const templateReferenceReferences = ref([]);
const selectedTaskId = ref("");
const historyScrollRequest = ref(0);
const submitting = ref(false);
const historyQuery = ref("");
const templateQuery = ref("");
const templateReferenceQuery = ref("");
const templateReferenceSourceContent = ref("");
const templateReferenceGeneratedContent = ref("");
const selectedReferenceTemplateId = ref("");
const templateFilledRanges = ref([]);
const templateFilling = ref(false);
const promptCursor = ref(0);

const showApiDialog = ref(false);
const showTemplateManagerDialog = ref(false);
const showTemplateReferenceDialog = ref(false);
const showTemplateEditor = ref(false);
const showTaskDetail = ref(false);
const showAboutDialog = ref(false);
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

const templateDraft = reactive(emptyTemplate());
const templateEditorMode = ref("edit");
const aboutInfo = ref({ version: "", buildTime: "", logs: "" });

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
let removeScrollbarVisibility = null;
let unlistenDragDrop = null;

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
  const items = historyTimeline.value;
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

onMounted(async () => {
  removeScrollbarVisibility = installAutoHideScrollbars();
  try {
    unlistenDragDrop = await listenDragDrop(handleReferenceDragDrop);
  } catch {
    // 浏览器预览没有 Tauri 拖放事件，保留 HTML5 drop 作为兼容路径。
  }
  await refreshAll();
  historyScrollRequest.value += 1;
  pollTimer = window.setInterval(refreshQueueOnly, 1600);
});

onUnmounted(() => {
  window.clearInterval(pollTimer);
  unlistenDragDrop?.();
  removeScrollbarVisibility?.();
});

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

// 定时轻量刷新队列快照，让历史列表同步运行状态。
async function refreshQueueOnly() {
  try {
    const snapshot = await invoke("queue_snapshot");
    applyQueue(snapshot);
  } catch {
    // 轮询失败保持静默，用户主动操作时再展示错误。
  }
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
      panelSizes.queue = clamp(startWidth + moveEvent.clientX - startX, 260, Math.min(500, max));
    } else {
      const max = Math.max(360, total - panelSizes.queue - resultMin);
      panelSizes.composer = clamp(startWidth - (moveEvent.clientX - startX), 360, Math.min(560, max));
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
    const request = {
      providerId: form.providerId || settings.value.activeImageProviderId || settings.value.activeProviderId,
      prompt: form.prompt,
      referencePaths: references.value.map((item) => item.path),
      size: sizeForPreset(form.resolution, form.ratio),
      resolution: form.resolution,
      ratio: form.ratio,
      orientation: orientationForRatio(form.ratio),
      quality: form.quality,
      outputFormat: "png",
      count: 1,
      promptFidelity: form.promptMode,
    };
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

// 从文件选择器导入参考图，并转换为可预览的数据 URL。
async function addReferenceImages() {
  await chooseReferenceImages(references, "已添加参考图");
}

// 在提示词框粘贴图片时，把剪贴板图片保存为参考图。
async function handlePromptPaste(event) {
  await pasteReferenceImage(event, references, "已从剪贴板添加参考图");
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
  event.preventDefault();
  try {
    const preview = await invoke("reference_from_clipboard");
    if (preview && appendReferencePreview(target, preview)) {
      setStatus(successMessage, "ok");
    }
  } catch (error) {
    setStatus(String(error), "error");
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
  try {
    const snapshot = await invoke("queue_snapshot");
    applyQueue(snapshot);
    const refreshed = historyTimeline.value.find((item) => item.id === task.id);
    setStatus(
      refreshed ? `任务状态：${statusLabel(refreshed.status)}` : "任务已不在历史记录中",
      refreshed ? "ok" : "busy",
    );
  } catch (error) {
    setStatus(String(error), "error");
  }
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
  templateEditorMode.value = "new";
  showTemplateEditor.value = true;
}

// 以只读模式查看模板，并在弹窗中高亮占位符。
async function viewTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  const { restored } = await restoreReferencePreviews(template.referencePaths);
  templateDraftReferences.value = restored;
  templateEditorMode.value = "view";
  showTemplateEditor.value = true;
}

// 以编辑模式打开模板。
async function editTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  const { restored } = await restoreReferencePreviews(template.referencePaths);
  templateDraftReferences.value = restored;
  templateEditorMode.value = "edit";
  showTemplateEditor.value = true;
}

// 保存新增或编辑后的模板，并刷新模板列表。
async function savePromptTemplate() {
  try {
    templateDraft.referencePaths = templateDraftReferences.value.map((item) => item.path);
    templates.value = await invoke("save_template", { template: deepClone(templateDraft) });
    showTemplateEditor.value = false;
    setStatus("模板已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function addTemplateDraftReferenceImages() {
  await chooseReferenceImages(templateDraftReferences, "已添加模板参考图");
}

async function handleTemplateDraftPaste(event) {
  await pasteReferenceImage(event, templateDraftReferences, "已从剪贴板添加模板参考图");
}

async function saveWorkbenchAsTemplate() {
  if (!form.prompt.trim()) {
    await showNotice("无法保存模板", "提示词为空，无法保存模板");
    return;
  }
  const template = {
    ...emptyTemplate(),
    content: form.prompt,
    referencePaths: references.value.map((item) => item.path),
  };
  try {
    templates.value = await invoke("save_template", { template });
    await showNotice("模板保存成功", "模板已经保存。");
    setStatus("模板保存成功", "ok");
  } catch (error) {
    const message = String(error);
    await showNotice("保存模板失败", message);
    setStatus(message, "error");
  }
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
  if (missing) setStatus(`${missing} 张模板参考图已不存在`, "busy");
}

async function addTemplateCallReferenceImages() {
  await chooseReferenceImages(templateReferenceReferences, "已添加本次调用参考图");
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
  setStatus("AI 正在填充模板…", "busy");
  try {
    const original = templateReferenceSourceContent.value;
    const filled = await invoke("fill_prompt_template", {
      providerId: form.chatProviderId,
      template: original,
    });
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

async function openAbout() {
  showAboutDialog.value = true;
  try {
    aboutInfo.value = await invoke("about_info");
  } catch (error) {
    aboutInfo.value = {
      version: "",
      buildTime: "",
      logs: `读取关于信息失败：${String(error)}`,
    };
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
