<template>
  <n-config-provider :theme-overrides="themeOverrides" component-size="small">
    <n-global-style />
    <main class="app">
      <AppTopbar
        @show-api="showApiDialog = true"
        @show-template-manager="showTemplateManagerDialog = true"
      />

      <section class="workspace" :style="workspaceStyle">
        <QueuePanel
          :filtered-history="filteredHistory"
          :selected-task-id="selectedTaskId"
          :history-query="historyQuery"
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
          @submit="submitTask"
          @show-template="showTemplateReferenceDialog = true"
          @clear-prompt="clearPrompt"
          @prompt-focus="capturePromptCursor"
          @prompt-cursor="capturePromptCursor"
          @prompt-paste="handlePromptPaste"
          @add-reference="addReferenceImages"
          @remove-reference="references.splice($event, 1)"
        />
      </section>

      <footer class="status-bar">
        <span class="status-pill" :data-tone="statusTone">{{ statusText }}</span>
        <span class="status-meta">当前 API：{{ activeProvider?.name || "未配置" }} · Images API</span>
        <span class="status-count">{{ queue.running.length }} 运行</span>
        <span class="status-count">{{ queue.waiting.length }} 排队</span>
        <span v-if="activeProvider && !activeProvider.apiKey" class="warn-text">API Key 未设置</span>
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
      />

      <TemplateReferenceDialog
        v-model:show="showTemplateReferenceDialog"
        v-model:query="templateReferenceQuery"
        v-model:content="templateReferenceContent"
        :templates="filteredReferenceTemplates"
        :selected-template-id="selectedReferenceTemplateId"
        :chat-provider-id="form.chatProviderId"
        :chat-provider-options="chatProviderOptions"
        :filled-ranges="templateFilledRanges"
        :filling="templateFilling"
        @update:chat-provider-id="form.chatProviderId = $event"
        @select-template="selectReferenceTemplate"
        @ai-fill="fillReferenceTemplate"
        @insert="insertReferenceTemplate"
      />

      <TemplateEditorDialog
        v-model:show="showTemplateEditor"
        :template="templateDraft"
        :mode="templateEditorMode"
        @save="savePromptTemplate"
      />

      <TaskDetailDialog
        v-model:show="showTaskDetail"
        :task="selectedTask"
        @reuse="reuseTask"
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
import ApiSourceDialog from "./components/dialogs/ApiSourceDialog.vue";
import TaskDetailDialog from "./components/dialogs/TaskDetailDialog.vue";
import TemplateEditorDialog from "./components/dialogs/TemplateEditorDialog.vue";
import TemplateManagerDialog from "./components/dialogs/TemplateManagerDialog.vue";
import TemplateReferenceDialog from "./components/dialogs/TemplateReferenceDialog.vue";
import { clamp, fileName, statusLabel } from "./lib/formatters";
import { deepClone, defaultSettings, emptyTemplate, normalizeSettingsForUi } from "./lib/models";
import {
  DEFAULT_PROMPT_MODE,
  DEFAULT_RATIO,
  orientationForRatio,
  sizeForPreset,
} from "./lib/options";
import { themeOverrides } from "./lib/theme";
import { invoke, openDialog } from "./tauri";

const statusText = ref("启动中");
const statusTone = ref("busy");
const dataDir = ref("");
const settings = ref(defaultSettings());
const history = ref([]);
const queue = reactive({ waiting: [], running: [], recent: [], workerActive: false, updatedAt: "" });
const templates = ref([]);
const references = ref([]);
const selectedTaskId = ref("");
const submitting = ref(false);
const historyQuery = ref("");
const templateQuery = ref("");
const templateReferenceQuery = ref("");
const templateReferenceContent = ref("");
const selectedReferenceTemplateId = ref("");
const templateFilledRanges = ref([]);
const templateFilling = ref(false);
const promptCursor = ref(0);

const showApiDialog = ref(false);
const showTemplateManagerDialog = ref(false);
const showTemplateReferenceDialog = ref(false);
const showTemplateEditor = ref(false);
const showTaskDetail = ref(false);

const templateDraft = reactive(emptyTemplate());
const templateEditorMode = ref("edit");

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
    [item.id, item.content].join(" ").toLowerCase().includes(query),
  );
});

const filteredReferenceTemplates = computed(() => {
  const query = templateReferenceQuery.value.trim().toLowerCase();
  if (!query) return templates.value;
  return templates.value.filter((item) =>
    [item.id, item.content].join(" ").toLowerCase().includes(query),
  );
});

onMounted(async () => {
  await refreshAll();
  pollTimer = window.setInterval(refreshQueueOnly, 1600);
});

onUnmounted(() => {
  window.clearInterval(pollTimer);
});

async function refreshAll() {
  try {
    const state = await invoke("load_app_state");
    applyState(state);
    setStatus("就绪", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function refreshQueueOnly() {
  try {
    const snapshot = await invoke("queue_snapshot");
    applyQueue(snapshot);
  } catch {
    // Polling should stay quiet; explicit actions still surface errors.
  }
}

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

function applyState(state) {
  settings.value = normalizeSettingsForUi(state.settings || defaultSettings());
  history.value = state.history || [];
  applyQueue(state.queue || {});
  templates.value = state.templates || [];
  dataDir.value = state.dataDir || "";
  ensureSelectedModels();
  ensureSelectedTask();
}

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

function ensureSelectedTask() {
  if (selectedTask.value) return;
  const next = historyTimeline.value.at(-1);
  selectedTaskId.value = next?.id || "";
}

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
  } catch (error) {
    setStatus(String(error), "error");
  } finally {
    submitting.value = false;
  }
}

async function addReferenceImages() {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    for (const path of paths) {
      const preview = await invoke("reference_from_path", { path });
      if (!references.value.some((item) => item.path === preview.path)) {
        references.value.push({ ...preview, previewUrl: preview.dataUrl });
      }
    }
    if (paths.length) setStatus(`已添加 ${paths.length} 张参考图`, "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function handlePromptPaste(event) {
  const items = Array.from(event?.clipboardData?.items || []);
  const files = Array.from(event?.clipboardData?.files || []);
  const hasImage = [...items, ...files].some((item) => item.type?.startsWith("image/"));
  if (!hasImage) return;
  event.preventDefault();
  try {
    const preview = await invoke("reference_from_clipboard");
    if (!references.value.some((item) => item.path === preview.path)) {
      references.value.push({ ...preview, previewUrl: preview.dataUrl });
      setStatus("已从剪贴板添加参考图", "ok");
    }
  } catch (error) {
    setStatus(String(error), "error");
  }
}

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

async function cancelTask(task) {
  try {
    await invoke("cancel_task", { taskId: task.id });
    setStatus("任务已取消", "ok");
    await refreshQueueOnly();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

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

async function deleteTask(task) {
  if (!window.confirm("删除这条生成记录？")) return;
  try {
    await invoke("delete_task", { taskId: task.id });
    if (selectedTaskId.value === task.id) selectedTaskId.value = "";
    setStatus("生成记录已删除", "ok");
    await refreshAll();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

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

async function downloadOutput(output) {
  try {
    const savedPath = await invoke("download_output", { path: output.path });
    setStatus(`已保存到下载目录：${fileName(savedPath)}`, "ok");
    await reveal(savedPath);
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function copyOutput(output) {
  try {
    await invoke("copy_image_to_clipboard", { path: output.path });
    setStatus("图片已复制到剪贴板", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function newTemplate() {
  Object.assign(templateDraft, emptyTemplate());
  templateEditorMode.value = "new";
  showTemplateEditor.value = true;
}

function viewTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  templateEditorMode.value = "view";
  showTemplateEditor.value = true;
}

function editTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  templateEditorMode.value = "edit";
  showTemplateEditor.value = true;
}

async function savePromptTemplate() {
  try {
    templates.value = await invoke("save_template", { template: deepClone(templateDraft) });
    showTemplateEditor.value = false;
    setStatus("模板已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deletePromptTemplate(id) {
  if (!window.confirm("删除这个模板？")) return;
  try {
    templates.value = await invoke("delete_template", { templateId: id });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function selectReferenceTemplate(template) {
  selectedReferenceTemplateId.value = template.id;
  templateReferenceContent.value = template.content || "";
  templateFilledRanges.value = [];
}

async function fillReferenceTemplate() {
  if (!templateReferenceContent.value.trim()) {
    setStatus("请先选择或输入模板内容", "error");
    return;
  }
  if (!form.chatProviderId) {
    setStatus("请先选择对话模型", "error");
    return;
  }
  templateFilling.value = true;
  try {
    const original = templateReferenceContent.value;
    const filled = await invoke("fill_prompt_template", {
      providerId: form.chatProviderId,
      template: original,
    });
    templateReferenceContent.value = filled;
    templateFilledRanges.value = mapFilledRanges(original, filled);
    setStatus("模板已填充", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  } finally {
    templateFilling.value = false;
  }
}

async function insertReferenceTemplate() {
  const content = templateReferenceContent.value;
  if (!content.trim()) {
    setStatus("模板内容为空", "error");
    return;
  }
  insertTextAtCursor(content);
  if (selectedReferenceTemplateId.value) {
    try {
      templates.value = await invoke("mark_template_used", { templateId: selectedReferenceTemplateId.value });
    } catch (error) {
      setStatus(String(error), "error");
    }
  }
  showTemplateReferenceDialog.value = false;
  setStatus("模板已引用到提示词", "ok");
}

function clearPrompt() {
  form.prompt = "";
  promptCursor.value = 0;
}

function capturePromptCursor(event) {
  const target = event?.target;
  if (typeof target?.selectionStart === "number") {
    promptCursor.value = target.selectionStart;
  }
}

function insertTextAtCursor(text) {
  const start = clamp(promptCursor.value, 0, form.prompt.length);
  form.prompt = `${form.prompt.slice(0, start)}${text}${form.prompt.slice(start)}`;
  promptCursor.value = start + text.length;
}

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

function templatePlaceholders(value) {
  const matches = [];
  const pattern = /\{[^{}]+\}/g;
  let match;
  while ((match = pattern.exec(value)) !== null) {
    matches.push({ start: match.index, end: match.index + match[0].length });
  }
  return matches;
}

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

function modelOptionLabel(provider) {
  return `${provider.name} · ${provider.imageModel || "未设置模型"}`;
}

function taskTime(task) {
  return task.createdAt || task.updatedAt || task.completedAt || "";
}

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
