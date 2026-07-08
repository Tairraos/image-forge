<template>
  <n-config-provider :theme-overrides="themeOverrides" component-size="small">
    <n-global-style />
    <main class="app">
      <AppTopbar
        :data-dir="dataDir"
        :form="form"
        :image-provider-options="imageProviderOptions"
        :chat-provider-options="chatProviderOptions"
        :queue="queue"
        @show-api="showApiDialog = true"
        @show-gallery="showGalleryDrawer = true"
        @show-template="showTemplateDrawer = true"
        @show-snippet="showSnippetModal = true"
        @show-settings="showSettingsDialog = true"
      />

      <section class="status-row">
        <span class="status-pill" :data-tone="statusTone">{{ statusText }}</span>
        <span>当前 API：{{ activeProvider?.name || "未配置" }} · Images API</span>
        <span v-if="activeProvider && !activeProvider.apiKey" class="warn-text">API Key 未设置</span>
      </section>

      <section class="workspace" :style="workspaceStyle">
        <QueuePanel
          :queue="queue"
          :running-and-waiting="runningAndWaiting"
          :filtered-history="filteredHistory"
          :selected-task-id="selectedTaskId"
          :history-query="historyQuery"
          @refresh="refreshAll"
          @select-task="selectedTaskId = $event"
          @update:history-query="historyQuery = $event"
          @cancel="cancelTask"
          @retry="retryTask"
          @promote="promoteTask"
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
          @reveal="reveal"
          @save-output="saveOutputToGallery"
        />

        <div
          class="panel-resizer"
          role="separator"
          aria-label="调整结果预览和工作台宽度"
          @pointerdown="startPanelResize('composer', $event)"
        ></div>

        <ComposerPanel
          :form="form"
          :references="references"
          :submitting="submitting"
          @submit="submitTask"
          @show-template="showTemplateDrawer = true"
          @show-snippet="showSnippetModal = true"
          @show-gallery="showGalleryDrawer = true"
          @add-reference="addReferenceImages"
          @remove-reference="references.splice($event, 1)"
        />
      </section>

      <ApiSourceDialog
        v-model:show="showApiDialog"
        :settings="settings"
        @save="saveApiSettings"
      />

      <GalleryDrawer
        v-model:show="showGalleryDrawer"
        v-model:query="galleryQuery"
        :items="filteredGallery"
        @add="addGalleryImages"
        @use="useGalleryItem"
        @edit="editGalleryItem"
        @delete="deleteGallery"
      />

      <TemplateDrawer
        v-model:show="showTemplateDrawer"
        v-model:query="templateQuery"
        :templates="filteredTemplates"
        @new="newTemplateFromPrompt"
        @insert="insertTemplate"
        @edit="editTemplate"
        @delete="deletePromptTemplate"
      />

      <SnippetDialog
        v-model:show="showSnippetModal"
        v-model:query="snippetQuery"
        :snippets="filteredSnippets"
        @new="newSnippet"
        @insert="insertText"
        @edit="editSnippet"
        @delete="deletePromptSnippet"
      />

      <SettingsDialog
        v-model:show="showSettingsDialog"
        :settings="settingsDraft"
        @choose-output-dir="chooseOutputDir"
        @save="saveStorageSettings"
      />

      <SnippetEditorDialog
        v-model:show="showSnippetEditor"
        :snippet="snippetDraft"
        @save="savePromptSnippet"
      />

      <TemplateEditorDialog
        v-model:show="showTemplateEditor"
        :template="templateDraft"
        @save="savePromptTemplate"
      />

      <GalleryEditorDialog
        v-model:show="showGalleryEditor"
        :item="galleryDraft"
        @save="saveGalleryEdit"
      />

      <TaskDetailDialog v-model:show="showTaskDetail" :task="selectedTask" />
    </main>
  </n-config-provider>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import AppTopbar from "./components/AppTopbar.vue";
import ComposerPanel from "./components/ComposerPanel.vue";
import QueuePanel from "./components/QueuePanel.vue";
import ResultPanel from "./components/ResultPanel.vue";
import ApiSourceDialog from "./components/dialogs/ApiSourceDialog.vue";
import GalleryEditorDialog from "./components/dialogs/GalleryEditorDialog.vue";
import SettingsDialog from "./components/dialogs/SettingsDialog.vue";
import SnippetDialog from "./components/dialogs/SnippetDialog.vue";
import SnippetEditorDialog from "./components/dialogs/SnippetEditorDialog.vue";
import TaskDetailDialog from "./components/dialogs/TaskDetailDialog.vue";
import TemplateEditorDialog from "./components/dialogs/TemplateEditorDialog.vue";
import GalleryDrawer from "./components/drawers/GalleryDrawer.vue";
import TemplateDrawer from "./components/drawers/TemplateDrawer.vue";
import { clamp, fileName, fileUrl } from "./lib/formatters";
import { deepClone, defaultSettings, emptySnippet, emptyTemplate, normalizeSettingsForUi } from "./lib/models";
import { themeOverrides } from "./lib/theme";
import { invoke, openDialog } from "./tauri";

const statusText = ref("启动中");
const statusTone = ref("busy");
const dataDir = ref("");
const settings = ref(defaultSettings());
const settingsDraft = reactive(defaultSettings());
const history = ref([]);
const queue = reactive({ waiting: [], running: [], recent: [], workerActive: false, updatedAt: "" });
const gallery = reactive({ items: [], categories: [] });
const snippets = ref([]);
const templates = ref([]);
const references = ref([]);
const selectedTaskId = ref("");
const submitting = ref(false);
const historyQuery = ref("");
const galleryQuery = ref("");
const snippetQuery = ref("");
const templateQuery = ref("");

const showApiDialog = ref(false);
const showGalleryDrawer = ref(false);
const showTemplateDrawer = ref(false);
const showSnippetModal = ref(false);
const showSettingsDialog = ref(false);
const showSnippetEditor = ref(false);
const showTemplateEditor = ref(false);
const showGalleryEditor = ref(false);
const showTaskDetail = ref(false);

const snippetDraft = reactive(emptySnippet());
const templateDraft = reactive(emptyTemplate());
const galleryDraft = reactive({ id: "", name: "", category: "", note: "" });

const form = reactive({
  providerId: "",
  chatProviderId: "",
  prompt: "",
  size: "1024x1024",
  quality: "auto",
  outputFormat: "png",
  count: 1,
  background: "",
  outputCompression: null,
  inputFidelity: "",
  moderation: "",
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

const runningAndWaiting = computed(() => [...queue.running, ...queue.waiting]);

const filteredHistory = computed(() => {
  const query = historyQuery.value.trim().toLowerCase();
  const items = history.value;
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
  history.value.find((task) => task.id === selectedTaskId.value)
  || queue.running.find((task) => task.id === selectedTaskId.value)
  || queue.waiting.find((task) => task.id === selectedTaskId.value)
  || null,
);

const currentOutputs = computed(() => selectedTask.value?.outputs || []);

const filteredGallery = computed(() => {
  const query = galleryQuery.value.trim().toLowerCase();
  if (!query) return gallery.items;
  return gallery.items.filter((item) =>
    [item.name, item.category, item.note].join(" ").toLowerCase().includes(query),
  );
});

const filteredSnippets = computed(() => {
  const query = snippetQuery.value.trim().toLowerCase();
  if (!query) return snippets.value;
  return snippets.value.filter((item) =>
    [item.tag, item.title, item.category, item.content].join(" ").toLowerCase().includes(query),
  );
});

const filteredTemplates = computed(() => {
  const query = templateQuery.value.trim().toLowerCase();
  if (!query) return templates.value;
  return templates.value.filter((item) =>
    [item.title, item.shortTitle, item.category, item.content, item.notes].join(" ").toLowerCase().includes(query),
  );
});

watch(showSettingsDialog, (show) => {
  if (show) Object.assign(settingsDraft, deepClone(settings.value));
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
  ensureSelectedModels();
  history.value = state.history || [];
  applyQueue(state.queue || {});
  Object.assign(gallery, state.gallery || { items: [], categories: [] });
  snippets.value = state.snippets || [];
  templates.value = state.templates || [];
  dataDir.value = state.dataDir || "";
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
  const next = queue.running[0] || queue.waiting[0] || history.value[0];
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
      size: form.size,
      quality: form.quality,
      outputFormat: form.outputFormat,
      count: Number(form.count) || 1,
      background: form.background || "",
      outputCompression: form.outputCompression ?? null,
      inputFidelity: form.inputFidelity || "",
      moderation: form.moderation || "",
    };
    const task = await invoke("enqueue_generation", { request });
    selectedTaskId.value = task.id;
    setStatus("任务已加入队列", "ok");
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

async function promoteTask(task) {
  try {
    const snapshot = await invoke("promote_task", { taskId: task.id });
    applyQueue(snapshot);
    setStatus("已移到队首", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function saveApiSettings(nextSettings) {
  try {
    const saved = await invoke("save_settings", { settings: nextSettings });
    settings.value = normalizeSettingsForUi(saved);
    ensureSelectedModels(true);
    showApiDialog.value = false;
    setStatus("API 源已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function chooseOutputDir() {
  const selected = await openDialog({
    directory: true,
    multiple: false,
    defaultPath: settingsDraft.outputDir || undefined,
    canCreateDirectories: true,
  });
  if (typeof selected === "string") {
    settingsDraft.outputDir = selected;
  }
}

async function saveStorageSettings() {
  try {
    const saved = await invoke("save_settings", { settings: deepClone(settingsDraft) });
    settings.value = normalizeSettingsForUi(saved);
    ensureSelectedModels(true);
    showSettingsDialog.value = false;
    setStatus("设置已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function addGalleryImages() {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    for (const path of paths) {
      const next = await invoke("add_gallery_item", {
        payload: { path, category: "默认", name: fileName(path), note: "" },
      });
      Object.assign(gallery, next);
    }
    if (paths.length) setStatus("图片已加入图库", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function useGalleryItem(item) {
  if (!references.value.some((refItem) => refItem.path === item.path)) {
    references.value.push({
      path: item.path,
      fileName: item.name,
      mimeType: item.mimeType,
      previewUrl: fileUrl(item.path),
    });
  }
  setStatus("图库图片已加入参考图", "ok");
}

function editGalleryItem(item) {
  Object.assign(galleryDraft, {
    id: item.id,
    name: item.name,
    category: item.category,
    note: item.note,
  });
  showGalleryEditor.value = true;
}

async function saveGalleryEdit() {
  try {
    const next = await invoke("update_gallery_item", { payload: deepClone(galleryDraft) });
    Object.assign(gallery, next);
    showGalleryEditor.value = false;
    setStatus("图库已更新", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deleteGallery(id) {
  if (!window.confirm("删除这个图库条目？")) return;
  try {
    const next = await invoke("delete_gallery_item", { itemId: id });
    Object.assign(gallery, next);
    setStatus("图库条目已删除", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function saveOutputToGallery(output) {
  try {
    const next = await invoke("add_gallery_item", {
      payload: { path: output.path, category: "生成结果", name: output.fileName, note: selectedTask.value?.prompt || "" },
    });
    Object.assign(gallery, next);
    setStatus("结果已保存到图库", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function newSnippet() {
  Object.assign(snippetDraft, emptySnippet());
  showSnippetEditor.value = true;
}

function editSnippet(snippet) {
  Object.assign(snippetDraft, deepClone(snippet));
  showSnippetEditor.value = true;
}

async function savePromptSnippet() {
  try {
    snippets.value = await invoke("save_snippet", { snippet: deepClone(snippetDraft) });
    showSnippetEditor.value = false;
    setStatus("片段已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deletePromptSnippet(id) {
  try {
    snippets.value = await invoke("delete_snippet", { snippetId: id });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function newTemplateFromPrompt() {
  Object.assign(templateDraft, {
    ...emptyTemplate(),
    title: form.prompt.trim().slice(0, 24) || "新模板",
    content: form.prompt,
  });
  showTemplateEditor.value = true;
}

function editTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
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
  try {
    templates.value = await invoke("delete_template", { templateId: id });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function insertTemplate(template, replace) {
  if (replace) {
    form.prompt = template.content;
  } else {
    insertText(template.content);
  }
  templates.value = await invoke("mark_template_used", { templateId: template.id });
}

function insertText(text) {
  const glue = form.prompt.trim() ? "\n" : "";
  form.prompt = `${form.prompt}${glue}${text}`;
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

function ensureSelectedModels(preferSaved = false) {
  if (preferSaved || !imageProviders.value.some((provider) => provider.id === form.providerId)) {
    form.providerId = settings.value.activeImageProviderId || settings.value.activeProviderId || imageProviders.value[0]?.id || "";
  }
  if (preferSaved || !chatProviders.value.some((provider) => provider.id === form.chatProviderId)) {
    form.chatProviderId = settings.value.activeChatProviderId || chatProviders.value[0]?.id || "";
  }
}
</script>
