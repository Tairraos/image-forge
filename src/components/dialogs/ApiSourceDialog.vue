<template>
  <n-modal v-model:show="visible" preset="card" title="API 源管理" class="api-modal">
    <div class="api-manager stacked">
      <section class="provider-list provider-list-horizontal" aria-label="API 源列表">
        <div class="provider-card-grid">
          <article
            v-for="(provider, index) in draft.providers"
            :key="provider.id"
            class="provider-card"
            :class="{ active: selectedId === provider.id }"
            @click="selectProvider(provider.id)"
          >
            <button
              type="button"
              class="provider-card-main"
              @click.stop="selectProvider(provider.id)"
            >
              <strong :title="provider.name || '未命名 API 源'">
                {{ provider.name || "未命名 API 源" }}
              </strong>
              <span>{{ modelTypeLabel(provider.modelType) }}</span>
              <span :title="provider.imageModel || '未设置模型'">
                {{ provider.imageModel || "未设置模型" }}
              </span>
              <small>{{ maskedApiKey(provider.apiKey) }}</small>
            </button>
            <div class="provider-card-actions">
              <button
                type="button"
                title="左移"
                :disabled="index === 0"
                @click.stop="moveProvider(provider.id, -1)"
              >
                <ArrowLeft :size="13" />
              </button>
              <button
                type="button"
                title="右移"
                :disabled="index === draft.providers.length - 1"
                @click.stop="moveProvider(provider.id, 1)"
              >
                <ArrowRight :size="13" />
              </button>
              <button
                type="button"
                title="删除"
                class="danger"
                :disabled="draft.providers.length <= 1"
                @click.stop="deleteProvider(provider.id)"
              >
                <Trash2 :size="13" />
              </button>
            </div>
          </article>
        </div>
      </section>

      <section v-if="selectedProvider" class="provider-editor">
        <n-form class="provider-form" label-placement="top" :show-feedback="false">
          <div class="provider-form-row provider-identity-row">
            <n-form-item label="名称">
              <n-input v-model:value="selectedProvider.name" placeholder="例如 OpenAI / Azure / 自建服务" />
            </n-form-item>
            <n-form-item label="类型">
              <n-select v-model:value="selectedProvider.modelType" :options="modelTypeOptions" />
            </n-form-item>
          </div>
          <div class="provider-form-row provider-credentials-row">
            <n-form-item label="Base URL">
              <n-input v-model:value="selectedProvider.baseUrl" placeholder="https://api.openai.com/v1" />
            </n-form-item>
            <n-form-item label="API Key">
              <n-input
                v-model:value="selectedProvider.apiKey"
                type="password"
                show-password-on="click"
                placeholder="sk-..."
              />
            </n-form-item>
          </div>
          <div class="provider-form-row provider-model-row">
            <n-form-item label="代理地址">
              <n-input
                v-model:value="selectedProvider.proxyUrl"
                placeholder="可选，例如 http://127.0.0.1:7890"
              />
            </n-form-item>
            <n-form-item label="模型">
              <div class="model-select-row">
                <n-select
                  v-model:value="selectedProvider.imageModel"
                  filterable
                  tag
                  :options="modelOptions"
                  placeholder="选择或输入模型 ID"
                />
                <n-button secondary :loading="loadingModels" @click="fetchModels">
                  获取
                </n-button>
              </div>
            </n-form-item>
          </div>
          <p v-if="modelFetchMessage" class="model-fetch-message" :data-tone="modelFetchTone">
            {{ modelFetchMessage }}
          </p>
        </n-form>
      </section>
    </div>

    <n-modal v-model:show="showImport" preset="card" title="导入 API 源" class="editor-modal">
      <div
        class="api-import-drop-zone"
        :class="{ 'reference-drop-active': importDragActive }"
        data-api-import-drop-zone
        @dragover.prevent="importDragActive = true"
        @dragleave="importDragActive = false"
        @drop.prevent="handleImportFileDrop"
      >
        <n-input
          v-model:value="importText"
          type="textarea"
          :autosize="{ minRows: 12, maxRows: 12 }"
          :resizable="false"
          placeholder="粘贴 JSON 配置或者拖入 JSON 文件"
        />
        <small v-if="readingImportFile">正在读取 JSON 文件…</small>
      </div>
      <p v-if="importError" class="import-error">{{ importError }}</p>
      <template #footer>
        <div class="dialog-actions">
          <n-button size="small" @click="showImport = false">取消</n-button>
          <n-button size="small" type="primary" @click="importProviders">导入</n-button>
        </div>
      </template>
    </n-modal>

    <template #footer>
      <div class="api-dialog-footer">
        <div class="api-dialog-footer-actions">
          <n-button size="small" type="primary" @click="addProvider">
            <template #icon><Plus :size="15" /></template>
            新增
          </n-button>
          <n-button size="small" secondary @click="openImportDialog">
            <template #icon><Download :size="15" /></template>
            导入
          </n-button>
          <n-button size="small" secondary :loading="exportingProviders" @click="exportProviders">
            <template #icon><Upload :size="15" /></template>
            导出
          </n-button>
          <n-button size="small" secondary @click="copyProvider">
            <template #icon><Copy :size="15" /></template>
            克隆
          </n-button>
        </div>
        <div class="dialog-actions">
          <n-button size="small" type="primary" @click="save">保存 API 源</n-button>
          <n-button size="small" @click="visible = false">关闭</n-button>
        </div>
      </div>
    </template>
  </n-modal>

  <ConfirmDialog
    v-model:show="showDeleteConfirmation"
    title="删除 API 源"
    :message="deleteConfirmationMessage"
    @confirm="confirmDeleteProvider"
    @cancel="cancelDeleteProvider"
  />

  <NoticeDialog
    v-model:show="showImportResult"
    title="API 源导入结果"
    :message="importResultMessage"
  />
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { ArrowLeft, ArrowRight, Copy, Download, Plus, Trash2, Upload } from "@lucide/vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import NoticeDialog from "./NoticeDialog.vue";
import { extractDroppedFilePaths } from "../../lib/referenceFiles";
import { invoke, listenDragDrop, saveDialog } from "../../tauri";
import {
  createProviderId,
  deepClone,
  defaultProvider,
  defaultSettings,
  normalizeSettingsForUi,
} from "../../lib/models";

const props = defineProps({
  show: { type: Boolean, default: false },
  settings: { type: Object, required: true },
});

const emit = defineEmits(["update:show", "save"]);

const visible = computed({
  get: () => props.show,
  set: (value) => emit("update:show", value),
});

const draft = reactive(defaultSettings());
const selectedId = ref("default");
const showImport = ref(false);
const importText = ref("");
const importError = ref("");
const importDragActive = ref(false);
const readingImportFile = ref(false);
const exportingProviders = ref(false);
const providerModels = reactive({});
const loadingModels = ref(false);
const modelFetchMessage = ref("");
const modelFetchTone = ref("idle");
const showDeleteConfirmation = ref(false);
const pendingDeleteProviderId = ref("");
const showImportResult = ref(false);
const importResultMessage = ref("");
let unlistenImportDragDrop = null;
const modelTypeOptions = [
  { label: "生图模型", value: "image" },
  { label: "对话模型", value: "chat" },
];

const selectedProvider = computed(() =>
  draft.providers.find((provider) => provider.id === selectedId.value) || draft.providers[0],
);

const modelOptions = computed(() => {
  const provider = selectedProvider.value;
  const models = new Set(providerModels[provider?.id] || []);
  if (provider?.imageModel) models.add(provider.imageModel);
  return Array.from(models).map((model) => ({ label: model, value: model }));
});

const deleteConfirmationMessage = computed(() => {
  const provider = draft.providers.find((item) => item.id === pendingDeleteProviderId.value);
  return `确认删除 API 源「${provider?.name || "未命名 API 源"}」？`;
});

watch(
  () => props.show,
  (show) => {
    if (!show) {
      cancelDeleteProvider();
      return;
    }
    Object.assign(draft, normalizeSettingsForUi(deepClone(props.settings)));
    selectedId.value = draft.activeImageProviderId || draft.activeProviderId || draft.providers[0]?.id || "";
    modelFetchMessage.value = "";
  },
  { immediate: true },
);

watch(showImport, (show) => {
  if (!show) importDragActive.value = false;
});

onMounted(async () => {
  try {
    unlistenImportDragDrop = await listenDragDrop(handleNativeImportDragDrop);
  } catch {
    // 浏览器预览没有 Tauri 原生拖放事件，保留 HTML5 drop 读取文件。
  }
});

onUnmounted(() => {
  unlistenImportDragDrop?.();
});

function selectProvider(id) {
  selectedId.value = id;
  const provider = selectedProvider.value;
  if (provider?.modelType === "chat") {
    draft.activeChatProviderId = id;
  } else {
    draft.activeImageProviderId = id;
    draft.activeProviderId = id;
  }
  modelFetchMessage.value = "";
}

function addProvider() {
  const provider = defaultProvider(draft.providers.length + 1);
  provider.imagesConcurrency = 1;
  provider.notes = "";
  draft.providers.push(provider);
  selectProvider(provider.id);
}

function copyProvider() {
  const source = selectedProvider.value;
  if (!source) return;
  const provider = normalizeProviderForSave(deepClone(source));
  provider.id = createProviderId();
  provider.name = `${source.name || "API 源"} 副本`;
  draft.providers.push(provider);
  selectedId.value = provider.id;
}

function openImportDialog() {
  importError.value = "";
  showImport.value = true;
}

async function exportProviders() {
  if (!draft.providers.length) return;
  try {
    const destination = await saveDialog({
      defaultPath: "ImageForge-api-sources.json",
      filters: [{ name: "JSON 配置", extensions: ["json"] }],
    });
    if (!destination) return;
    exportingProviders.value = true;
    const savedPath = await invoke("export_api_providers", {
      destination,
      providers: draft.providers.map((provider) =>
        normalizeProviderForSave(deepClone(provider)),
      ),
    });
    modelFetchTone.value = "ok";
    modelFetchMessage.value = `API 源已导出：${savedPath.split(/[\\/]/).at(-1)}`;
  } catch (error) {
    modelFetchTone.value = "error";
    modelFetchMessage.value = String(error);
  } finally {
    exportingProviders.value = false;
  }
}

async function handleImportFileDrop(event) {
  importDragActive.value = false;
  const files = Array.from(event?.dataTransfer?.files || []);
  const file = files.find((item) => isJsonPath(item.name));
  const path = extractDroppedFilePaths(event?.dataTransfer).find(isJsonPath) || file?.path;
  if (path) {
    await loadImportFilePath(path);
    return;
  }
  if (file?.text) {
    await loadImportFileText(() => file.text());
    return;
  }
  importError.value = "只能拖入 JSON 文件";
}

function handleNativeImportDragDrop(event) {
  if (!showImport.value) return;
  const payload = event?.payload || {};
  if (payload.type === "leave") {
    importDragActive.value = false;
    return;
  }
  const overImportBox = importDropTarget(payload.position);
  if (payload.type === "enter" || payload.type === "over") {
    importDragActive.value = overImportBox;
    return;
  }
  importDragActive.value = false;
  if (payload.type !== "drop" || !overImportBox) return;
  const path = (payload.paths || []).find(isJsonPath);
  if (path) void loadImportFilePath(path);
  else importError.value = "只能拖入 JSON 文件";
}

async function loadImportFilePath(path) {
  await loadImportFileText(() => invoke("read_api_providers_file", { path }));
}

async function loadImportFileText(reader) {
  readingImportFile.value = true;
  importError.value = "";
  try {
    const text = await reader();
    JSON.parse(text);
    importText.value = text;
  } catch (error) {
    importError.value = String(error);
  } finally {
    readingImportFile.value = false;
  }
}

function importDropTarget(position) {
  const x = Number(position?.x);
  const y = Number(position?.y);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return false;
  const scale = window.devicePixelRatio || 1;
  return [[x, y], [x / scale, y / scale]].some(([left, top]) =>
    Boolean(document.elementFromPoint(left, top)?.closest("[data-api-import-drop-zone]")),
  );
}

function isJsonPath(path) {
  return String(path || "").toLowerCase().endsWith(".json");
}

function importProviders() {
  importError.value = "";
  let value;
  try {
    value = JSON.parse(importText.value);
  } catch (error) {
    importError.value = `JSON 解析失败：${error.message}`;
    return;
  }
  const entries = importProviderEntries(value);
  if (!entries) {
    importError.value = "请粘贴有效的 API 配置 JSON";
    return;
  }

  if (!entries.length) {
    importError.value = "没有可导入的 API 源";
    return;
  }

  const signatures = new Set(draft.providers.map(providerImportSignature));
  const imported = [];
  let duplicateCount = 0;
  for (const [index, [key, item]] of entries.entries()) {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      importError.value = `「${key}」不是有效配置`;
      return;
    }
    const name = String(item.name || providerNameFromImportKey(key)).trim()
      || `导入源 ${index + 1}`;
    const provider = {
      ...defaultProvider(draft.providers.length + imported.length + 1),
      id: createProviderId(),
      name,
      modelType: item.modelType === "chat" ? "chat" : "image",
      baseUrl: item.openAiBaseUrl || item.baseUrl || "",
      apiKey: item.openAiApiKey || item.apiKey || "",
      proxyUrl: item.proxyUrl || "",
      imageModel: item.openAiModelId || item.imageModel || item.model || "gpt-image-2",
      imagesConcurrency: 1,
      enabled: item.enabled !== false,
      notes: "",
    };
    const signature = providerImportSignature(provider);
    if (signatures.has(signature)) {
      duplicateCount += 1;
      continue;
    }
    signatures.add(signature);
    imported.push(provider);
  }

  for (const provider of imported) {
    draft.providers.push(provider);
  }
  if (imported.length) selectProvider(imported[imported.length - 1].id);
  showImport.value = false;
  importText.value = "";
  importResultMessage.value = `导入 ${imported.length} 个，重复 ${duplicateCount} 个。`;
  showImportResult.value = true;
}

// 忽略随机 ID 和已固定的兼容字段，仅比较会实际保存的 API 配置内容。
function providerImportSignature(provider) {
  return JSON.stringify([
    String(provider.name || "").trim(),
    provider.modelType === "chat" ? "chat" : "image",
    String(provider.baseUrl || "").trim(),
    String(provider.apiKey || "").trim(),
    String(provider.proxyUrl || "").trim(),
    String(provider.imageModel || "gpt-image-2").trim() || "gpt-image-2",
    provider.enabled !== false,
  ]);
}

function importProviderEntries(value) {
  if (Array.isArray(value)) {
    return value.map((item, index) => [item?.name || `API 源 ${index + 1}`, item]);
  }
  if (!value || typeof value !== "object") return null;
  if (Array.isArray(value.providers)) {
    return value.providers.map((item, index) => [item?.name || `API 源 ${index + 1}`, item]);
  }
  return Object.entries(value);
}

function deleteProvider(id = selectedId.value) {
  if (draft.providers.length <= 1) return;
  const index = draft.providers.findIndex((provider) => provider.id === id);
  if (index < 0) return;
  pendingDeleteProviderId.value = id;
  showDeleteConfirmation.value = true;
}

function confirmDeleteProvider() {
  const index = draft.providers.findIndex(
    (provider) => provider.id === pendingDeleteProviderId.value,
  );
  cancelDeleteProvider();
  if (index < 0 || draft.providers.length <= 1) return;
  draft.providers.splice(index, 1);
  const next = draft.providers[Math.min(index, draft.providers.length - 1)] || draft.providers[0];
  selectProvider(next.id);
}

function cancelDeleteProvider() {
  showDeleteConfirmation.value = false;
  pendingDeleteProviderId.value = "";
}

function moveProvider(id = selectedId.value, offset) {
  const index = draft.providers.findIndex((provider) => provider.id === id);
  const nextIndex = index + offset;
  if (index < 0 || nextIndex < 0 || nextIndex >= draft.providers.length) return;
  const [item] = draft.providers.splice(index, 1);
  draft.providers.splice(nextIndex, 0, item);
  selectedId.value = item.id;
}

async function fetchModels() {
  const provider = selectedProvider.value;
  if (!provider) return;
  loadingModels.value = true;
  modelFetchMessage.value = "";
  try {
    const models = await invoke("list_provider_models", {
      provider: normalizeProviderForSave(deepClone(provider)),
    });
    providerModels[provider.id] = models;
    modelFetchTone.value = "ok";
    modelFetchMessage.value = models.length ? `已获取 ${models.length} 个模型` : "模型列表为空";
  } catch (error) {
    modelFetchTone.value = "error";
    modelFetchMessage.value = String(error);
  } finally {
    loadingModels.value = false;
  }
}

function save() {
  draft.providers = draft.providers.map((provider) => normalizeProviderForSave(provider));
  const imageProvider = draft.providers.find((provider) => provider.modelType !== "chat");
  const chatProvider = draft.providers.find((provider) => provider.modelType === "chat");
  if (!draft.providers.some((provider) => provider.id === draft.activeImageProviderId && provider.modelType !== "chat")) {
    draft.activeImageProviderId = imageProvider?.id || "";
  }
  if (!draft.providers.some((provider) => provider.id === draft.activeChatProviderId && provider.modelType === "chat")) {
    draft.activeChatProviderId = chatProvider?.id || "";
  }
  draft.activeProviderId = draft.activeImageProviderId || draft.providers[0]?.id || "";
  emit("save", deepClone(draft));
}

function normalizeProviderForSave(provider) {
  return {
    ...provider,
    modelType: provider.modelType === "chat" ? "chat" : "image",
    proxyUrl: provider.proxyUrl?.trim() || "",
    imageModel: provider.imageModel?.trim() || "gpt-image-2",
    imagesConcurrency: 1,
    notes: "",
  };
}

function modelTypeLabel(value) {
  return value === "chat" ? "对话模型" : "生图模型";
}

function maskedApiKey(value) {
  const key = String(value || "");
  if (!key) return "未填写";
  return `${key.slice(0, 6)}******${key.slice(-6)}`;
}

function providerNameFromImportKey(key) {
  return String(key).split("-")[0].trim() || "导入源";
}
</script>
