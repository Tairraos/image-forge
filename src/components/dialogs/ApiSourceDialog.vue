<template>
  <n-modal v-model:show="visible" preset="card" title="API 源管理" class="api-modal">
    <div class="api-manager">
      <aside class="provider-list">
        <div class="provider-list-actions">
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
            复制
          </n-button>
        </div>

        <button
          v-for="provider in draft.providers"
          :key="provider.id"
          type="button"
          class="provider-card"
          :class="{ active: selectedId === provider.id }"
          @click="selectProvider(provider.id)"
        >
          <strong>{{ provider.name }}</strong>
          <span>{{ modelTypeLabel(provider.modelType) }} · {{ provider.imageModel }}</span>
          <small>{{ provider.apiKey ? "API Key 已保存" : "未填写 API Key" }}</small>
        </button>
      </aside>

      <section v-if="selectedProvider" class="provider-editor">
        <div class="provider-editor-head">
          <h3>Provider</h3>
          <div class="provider-editor-actions">
            <n-button size="tiny" quaternary @click="moveProvider(-1)">
              <template #icon><ArrowUp :size="14" /></template>
            </n-button>
            <n-button size="tiny" quaternary type="error" @click="deleteProvider">
              <template #icon><Trash2 :size="14" /></template>
            </n-button>
          </div>
        </div>

        <n-form label-placement="top" :show-feedback="false">
          <div class="two-col">
            <n-form-item label="名称">
              <n-input v-model:value="selectedProvider.name" />
            </n-form-item>
            <n-form-item label="类型">
              <n-select v-model:value="selectedProvider.modelType" :options="modelTypeOptions" />
            </n-form-item>
          </div>
          <n-form-item label="Base URL">
            <n-input v-model:value="selectedProvider.baseUrl" />
          </n-form-item>
          <n-form-item label="API Key">
            <n-input v-model:value="selectedProvider.apiKey" type="password" show-password-on="click" />
          </n-form-item>
          <div class="two-col">
            <n-form-item label="模型 ID">
              <n-input v-model:value="selectedProvider.imageModel" />
            </n-form-item>
            <n-form-item label="并发">
              <n-input-number
                v-model:value="selectedProvider.imagesConcurrency"
                :min="1"
                :max="32"
              />
            </n-form-item>
          </div>
          <n-form-item label="备注">
            <n-input v-model:value="selectedProvider.notes" type="textarea" :autosize="{ minRows: 3 }" />
          </n-form-item>
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
      <div class="dialog-actions">
        <n-button size="small" @click="visible = false">取消</n-button>
        <n-button size="small" type="primary" @click="save">保存 API 源</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { computed, reactive, ref, watch } from "vue";
import { ArrowUp, Copy, Plus, Trash2, Upload } from "@lucide/vue";
import { createProviderId, deepClone, defaultProvider, defaultSettings, normalizeSettingsForUi } from "../../lib/models";

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
const modelTypeOptions = [
  { label: "生图模型", value: "image" },
  { label: "对话模型", value: "chat" },
];

const selectedProvider = computed(() =>
  draft.providers.find((provider) => provider.id === selectedId.value) || draft.providers[0],
);

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    Object.assign(draft, normalizeSettingsForUi(deepClone(props.settings)));
    selectedId.value = draft.activeProviderId;
  },
  { immediate: true },
);

function selectProvider(id) {
  selectedId.value = id;
  draft.activeProviderId = id;
}

function addProvider() {
  const provider = defaultProvider(draft.providers.length + 1);
  draft.providers.push(provider);
  selectProvider(provider.id);
}

function copyProvider() {
  const source = selectedProvider.value;
  if (!source) return;
  const provider = deepClone(source);
  provider.id = createProviderId();
  provider.name = `${source.name} Copy`;
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
      imageModel: item.openAiModelId || item.imageModel || "gpt-image-2",
    });
  }
  if (!imported.length) {
    importError.value = "没有可导入的 API 源";
    return;
  }

  for (const provider of imported) {
    const existing = draft.providers.findIndex((item) => item.id === provider.id);
    if (existing >= 0) {
      draft.providers[existing] = provider;
    } else {
      draft.providers.push(provider);
    }
  }
  selectedId.value = imported[imported.length - 1].id;
  draft.activeProviderId = selectedId.value;
  showImport.value = false;
  importText.value = "";
}

function deleteProvider() {
  if (draft.providers.length <= 1) return;
  draft.providers = draft.providers.filter((provider) => provider.id !== selectedId.value);
  selectedId.value = draft.providers[0].id;
  draft.activeProviderId = selectedId.value;
}

function moveProvider(offset) {
  const index = draft.providers.findIndex((provider) => provider.id === selectedId.value);
  const nextIndex = index + offset;
  if (index < 0 || nextIndex < 0 || nextIndex >= draft.providers.length) return;
  const [item] = draft.providers.splice(index, 1);
  draft.providers.splice(nextIndex, 0, item);
}

function save() {
  if (selectedProvider.value?.modelType === "chat") {
    draft.activeChatProviderId = selectedId.value;
  } else {
    draft.activeImageProviderId = selectedId.value;
  }
  draft.activeProviderId = draft.activeImageProviderId || selectedId.value;
  emit("save", deepClone(draft));
}

function modelTypeLabel(value) {
  return value === "chat" ? "对话模型" : "生图模型";
}

function providerNameFromImportKey(key) {
  return String(key).split("-")[0].trim() || "Imported";
}

</script>
