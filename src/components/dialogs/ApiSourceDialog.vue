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
              <strong>{{ provider.name || "未命名 API 源" }}</strong>
              <span>{{ modelTypeLabel(provider.modelType) }} · {{ provider.imageModel || "未设置模型" }}</span>
              <small>{{ provider.apiKey ? "API Key 已保存" : "未填写 API Key" }}</small>
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
      <n-input
        v-model:value="importText"
        type="textarea"
        :autosize="{ minRows: 12 }"
        placeholder="粘贴 JSON 配置"
      />
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
          <n-button size="small" secondary @click="showImport = true">
            <template #icon><Upload :size="15" /></template>
            导入
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
</template>

<script setup>
import { computed, reactive, ref, watch } from "vue";
import { ArrowLeft, ArrowRight, Copy, Plus, Trash2, Upload } from "@lucide/vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import { invoke } from "../../tauri";
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
const providerModels = reactive({});
const loadingModels = ref(false);
const modelFetchMessage = ref("");
const modelFetchTone = ref("idle");
const showDeleteConfirmation = ref(false);
const pendingDeleteProviderId = ref("");
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

function importProviders() {
  importError.value = "";
  let value;
  try {
    value = JSON.parse(importText.value);
  } catch (error) {
    importError.value = `JSON 解析失败：${error.message}`;
    return;
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    importError.value = "请粘贴对象格式的 API 配置 JSON";
    return;
  }

  const imported = [];
  for (const [key, item] of Object.entries(value)) {
    if (!item || typeof item !== "object") {
      importError.value = `「${key}」不是有效配置`;
      return;
    }
    imported.push({
      ...defaultProvider(draft.providers.length + imported.length + 1),
      id: createProviderId(),
      name: providerNameFromImportKey(key),
      modelType: item.modelType === "chat" ? "chat" : "image",
      baseUrl: item.openAiBaseUrl || item.baseUrl || "",
      apiKey: item.openAiApiKey || item.apiKey || "",
      proxyUrl: item.proxyUrl || "",
      imageModel: item.openAiModelId || item.imageModel || "gpt-image-2",
      imagesConcurrency: 1,
      notes: "",
    });
  }
  if (!imported.length) {
    importError.value = "没有可导入的 API 源";
    return;
  }

  for (const provider of imported) {
    draft.providers.push(provider);
  }
  selectProvider(imported[imported.length - 1].id);
  showImport.value = false;
  importText.value = "";
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

function providerNameFromImportKey(key) {
  return String(key).split("-")[0].trim() || "导入源";
}
</script>
