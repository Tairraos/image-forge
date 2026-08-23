<template>
  <div class="api-manager split">
    <section class="provider-editor-pane">
      <template v-if="selectedProvider">
        <n-form class="provider-form" label-placement="top" :show-feedback="false">
          <n-form-item label="名称">
            <n-input v-model:value="selectedProvider.name" placeholder="例如 OpenAI / Azure / 自建服务" />
          </n-form-item>
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
          <n-form-item label="代理地址">
            <n-input
              v-model:value="selectedProvider.proxyUrl"
              placeholder="可选，例如 http://127.0.0.1:7890"
            />
          </n-form-item>
          <n-form-item label="模型">
            <div class="model-select-row">
              <n-select
                :value="selectedProvider.imageModel"
                filterable
                tag
                :options="modelOptions"
                placeholder="选择或输入模型 ID"
                @update:value="updateSelectedModel"
              />
              <n-button secondary :loading="loadingModels" @click="fetchModels">
                获取
              </n-button>
            </div>
          </n-form-item>
          <!-- 绘图 API：可编辑模型类型；对话 API：只读展示「对话模型」 -->
          <n-form-item label="模型类型">
            <n-select
              v-if="kind === 'image'"
              :value="selectedProvider.modelType"
              :options="imageModelTypeOptions"
              placeholder="选择模型类型"
              :consistent-menu-width="false"
              @update:value="updateSelectedModelType"
            />
            <n-input
              v-else
              :value="chatModelTypeLabel"
              readonly
              disabled
            />
          </n-form-item>
          <p v-if="modelFetchMessage" class="model-fetch-message" :data-tone="modelFetchTone">
            {{ modelFetchMessage }}
          </p>
        </n-form>
      </template>
      <p v-else class="provider-empty">还没有{{ kindLabel }}源，先新增一个。</p>

      <div class="api-dialog-footer">
        <div class="api-dialog-footer-actions" :class="{ 'paste-shake': pasteShake }">
          <n-button size="small" @click="addProvider">+ 新增</n-button>
          <n-button size="small" @click="pasteProvider">粘贴</n-button>
          <n-button size="small" type="primary" @click="save">保存</n-button>
          <n-button size="small" :disabled="!selectedProvider" @click="copyProvider">
            <template #icon><Copy :size="15" /></template>
            克隆
          </n-button>
        </div>
      </div>
    </section>

    <div class="provider-split-line" aria-hidden="true"></div>

    <section class="provider-list provider-list-vertical" aria-label="API 源列表">
      <div class="provider-card-grid vertical" data-persistent-scrollbar>
        <article
          v-for="(provider, index) in visibleProviders"
          :key="provider.id"
          class="provider-card"
          :class="[
            { active: selectedId === provider.id, default: index === 0 },
            providerTypeClass(provider.modelType),
            { dragging: dragId === provider.id, 'drag-over': dragOverId === provider.id },
          ]"
          :style="dragOverId === provider.id ? { '--drag-h': `${dragHeight}px` } : undefined"
          draggable="true"
          @click="selectProvider(provider.id)"
          @dragstart="onDragStart(provider.id, $event)"
          @dragover.prevent="onDragOver(provider.id)"
          @drop.prevent="onDrop(provider.id)"
          @dragend="onDragEnd"
        >
          <button
            type="button"
            class="provider-card-main"
            @click.stop="selectProvider(provider.id)"
          >
            <strong :title="provider.name || '未命名 API 源'">
              {{ provider.name || "未命名 API 源" }}
            </strong>
            <span :title="provider.imageModel || '未设置模型'">
              {{ provider.imageModel || "未设置模型" }}
            </span>
            <small>{{ maskedApiKey(provider.apiKey) }}</small>
            <em v-if="index === 0" class="provider-default-badge">默认</em>
          </button>
          <div class="provider-card-actions">
            <button
              type="button"
              title="上移"
              :disabled="index === 0"
              @click.stop="moveProvider(provider.id, -1)"
            >
              <ArrowUp :size="13" />
            </button>
            <button
              type="button"
              title="下移"
              :disabled="index === visibleProviders.length - 1"
              @click.stop="moveProvider(provider.id, 1)"
            >
              <ArrowDown :size="13" />
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
        <p v-if="!visibleProviders.length" class="provider-empty">暂无{{ kindLabel }}源</p>
      </div>
    </section>
  </div>

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
import { ArrowDown, ArrowUp, Copy, Trash2 } from "@lucide/vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import { invoke } from "../../tauri";
import * as api from "../../api/index.js";
import {
  createProviderId,
  deepClone,
  DEFAULT_CHAT_MODEL,
  DEFAULT_IMAGE_MODEL,
  defaultProvider,
  defaultSettings,
  IMAGE_MODEL_TYPE_OPTIONS,
  isImageModelType,
  normalizeModelType,
  normalizeSettingsForUi,
  parseClipboardProvider,
  recommendImageModelType,
} from "../../lib/models";

const props = defineProps({
  show: { type: Boolean, default: false },
  settings: { type: Object, required: true },
  kind: { type: String, default: "image" },
});

const emit = defineEmits(["close", "save"]);

const draft = reactive(defaultSettings());
const selectedId = ref("");
const providerModels = reactive({});
const loadingModels = ref(false);
const modelFetchMessage = ref("");
const modelFetchTone = ref("idle");
const showDeleteConfirmation = ref(false);
const pendingDeleteProviderId = ref("");
const dragId = ref("");
const dragOverId = ref("");
const dragHeight = ref(0);
const pasteShake = ref(false);
let pasteShakeTimer = 0;

const kindLabel = computed(() => (props.kind === "chat" ? "对话 API" : "绘图 API"));
/** 绘图模型类型下拉：菜单显示 label，绑定 value（如 image-gpt） */
const imageModelTypeOptions = IMAGE_MODEL_TYPE_OPTIONS;
/**
 * 对话模型类型只读展示。
 * UI 文案（label）固定为「对话模型」；概念 value 为 text。
 * 持久化字段 modelType 仍写 "chat"，与后端/过滤逻辑兼容。
 */
const chatModelTypeLabel = "对话模型";

const visibleProviders = computed(() =>
  draft.providers.filter((provider) => matchesKind(provider.modelType)),
);

const selectedProvider = computed(
  () =>
    visibleProviders.value.find((provider) => provider.id === selectedId.value)
    || visibleProviders.value[0]
    || null,
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
  () => [props.show, props.kind],
  ([show]) => {
    if (!show) {
      cancelDeleteProvider();
      return;
    }
    Object.assign(draft, normalizeSettingsForUi(deepClone(props.settings)));
    syncActiveFromOrder();
    selectedId.value =
      (props.kind === "chat" ? draft.activeChatProviderId : draft.activeImageProviderId)
      || visibleProviders.value[0]?.id
      || "";
    modelFetchMessage.value = "";
    dragId.value = "";
    dragOverId.value = "";
  },
  { immediate: true },
);

function matchesKind(modelType) {
  return props.kind === "chat" ? modelType === "chat" : isImageModelType(modelType);
}

function selectProvider(id) {
  selectedId.value = id;
  modelFetchMessage.value = "";
}

function addProvider() {
  const provider = defaultProvider(
    draft.providers.length + 1,
    props.kind === "chat" ? "chat" : "image-gpt",
  );
  if (props.kind === "chat") {
    provider.imageModel = provider.imageModel || DEFAULT_CHAT_MODEL;
  }
  provider.imagesConcurrency = 1;
  provider.notes = "";
  draft.providers.push(provider);
  selectProvider(provider.id);
  syncActiveFromOrder();
  return provider;
}

/**
 * 「粘贴」按钮入口（绘图 API / 对话 API 共用）。
 *
 * 流程：
 * 1. 通过 Tauri 命令 read_clipboard_text 读取系统剪贴板
 * 2. 调用 lib/models.js 的 parseClipboardProvider 做智能解析
 *    （JSON 优先，失败再走纯文本启发式：url / sk- / 名称 / image 模型名）
 * 3. 解析失败 → shakePaste() 抖动反馈，不新建源
 * 4. 解析成功 → addProvider() 新建一条，填入 name/baseUrl/apiKey/imageModel
 * 5. 绘图面板：用模型名 + baseUrl 自动推测 modelType（用户仍可在下拉框改）
 * 6. 对话面板：强制 modelType = chat
 * 7. 尝试拉取远端模型列表；若剪贴板没给出模型名，则自动选第一个
 *
 * 核心解析实现位置：src/lib/models.js → parseClipboardProvider
 */
async function pasteProvider() {
  let text = "";
  try {
    // 桌面端走 Tauri 原生剪贴板，避免浏览器权限限制
    text = await api.readClipboardText();
  } catch {
    shakePaste();
    return;
  }

  // 智能解析：JSON 配置 或 自由文本（http(s)/sk-/名称/image 模型）
  const parsed = parseClipboardProvider(text);
  if (!parsed) {
    shakePaste();
    return;
  }

  const provider = addProvider();
  // 名称为空时保留 addProvider 生成的默认名（如「供应商 N」）
  if (parsed.name) provider.name = parsed.name;
  provider.baseUrl = parsed.baseUrl;
  provider.apiKey = parsed.apiKey;

  if (props.kind === "chat") {
    provider.modelType = "chat";
    // 对话源：剪贴板若带了模型名也写入，否则留给 fetchModels 自动选
    provider.imageModel = parsed.imageModel || "";
  } else {
    // 绘图源：写入模型名，并据此（+ baseUrl）推测模型类型
    provider.imageModel = parsed.imageModel || "";
    provider.modelType = recommendImageModelType(
      provider.imageModel,
      provider.baseUrl,
    );
  }

  selectProvider(provider.id);
  // 剪贴板已给出模型名则保留；否则拉取列表后自动选第一个
  await fetchModels({ autoSelectFirst: !provider.imageModel });
}

function updateSelectedModel(value) {
  const provider = selectedProvider.value;
  if (!provider) return;
  provider.imageModel = String(value || "");
  // 切换模型时自动推荐类型；用户仍可通过「模型类型」下拉手动覆盖
  if (props.kind === "image") {
    provider.modelType = recommendImageModelType(provider.imageModel, provider.baseUrl);
  } else {
    provider.modelType = "chat";
  }
}

/** 绘图 API：用户手动选择模型类型（不再被其它逻辑强制改回，除非再次改模型名） */
function updateSelectedModelType(value) {
  const provider = selectedProvider.value;
  if (!provider || props.kind !== "image") return;
  provider.modelType = normalizeModelType(value, provider.imageModel, provider.baseUrl);
}

function copyProvider() {
  const source = selectedProvider.value;
  if (!source) return;
  const provider = normalizeProviderForSave(deepClone(source));
  provider.id = createProviderId();
  provider.name = `${source.name || "API 源"} 副本`;
  draft.providers.push(provider);
  selectProvider(provider.id);
  syncActiveFromOrder();
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
  syncActiveFromOrder();
  const next = visibleProviders.value[0];
  selectedId.value = next?.id || "";
}

function cancelDeleteProvider() {
  showDeleteConfirmation.value = false;
  pendingDeleteProviderId.value = "";
}

function moveProvider(id = selectedId.value, offset) {
  const ids = visibleProviders.value.map((provider) => provider.id);
  const index = ids.indexOf(id);
  const nextIndex = index + offset;
  if (index < 0 || nextIndex < 0 || nextIndex >= ids.length) return;
  const ordered = ids.slice();
  const [item] = ordered.splice(index, 1);
  ordered.splice(nextIndex, 0, item);
  applyVisibleOrder(ordered);
  selectedId.value = id;
  syncActiveFromOrder();
}

function onDragStart(id, event) {
  dragId.value = id;
  dragOverId.value = "";
  dragHeight.value = event?.currentTarget?.offsetHeight || 0;
  if (event?.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", id);
  }
}

function onDragOver(id) {
  if (!dragId.value || dragId.value === id) return;
  dragOverId.value = id;
}

function onDrop(id) {
  if (!dragId.value || dragId.value === id) {
    onDragEnd();
    return;
  }
  const ids = visibleProviders.value.map((provider) => provider.id);
  const from = ids.indexOf(dragId.value);
  const to = ids.indexOf(id);
  if (from < 0 || to < 0) {
    onDragEnd();
    return;
  }
  const ordered = ids.slice();
  const [item] = ordered.splice(from, 1);
  ordered.splice(to, 0, item);
  applyVisibleOrder(ordered);
  selectedId.value = item;
  syncActiveFromOrder();
  onDragEnd();
}

function onDragEnd() {
  dragId.value = "";
  dragOverId.value = "";
}

function applyVisibleOrder(orderedIds) {
  const queue = orderedIds.slice();
  draft.providers = draft.providers.map((provider) => {
    if (!matchesKind(provider.modelType)) return provider;
    const id = queue.shift();
    return draft.providers.find((item) => item.id === id) || provider;
  });
}

function syncActiveFromOrder() {
  const imageProvider = draft.providers.find((provider) => isImageModelType(provider.modelType));
  const chatProvider = draft.providers.find((provider) => provider.modelType === "chat");
  draft.activeImageProviderId = imageProvider?.id || "";
  draft.activeChatProviderId = chatProvider?.id || "";
  draft.activeProviderId = draft.activeImageProviderId || draft.providers[0]?.id || "";
}

async function fetchModels({ autoSelectFirst = false } = {}) {
  const provider = selectedProvider.value;
  if (!provider) return;
  loadingModels.value = true;
  modelFetchMessage.value = "";
  try {
    const models = await api.listProviderModels(
      normalizeProviderForSave(deepClone(provider)),
    );
    providerModels[provider.id] = models;
    modelFetchTone.value = "ok";
    modelFetchMessage.value = models.length ? `已获取 ${models.length} 个模型` : "模型列表为空";
    if (autoSelectFirst && models.length) {
      updateSelectedModel(models[0]);
    }
  } catch (error) {
    modelFetchTone.value = "error";
    modelFetchMessage.value = String(error);
  } finally {
    loadingModels.value = false;
  }
}

function shakePaste() {
  pasteShake.value = false;
  requestAnimationFrame(() => {
    pasteShake.value = true;
    window.clearTimeout(pasteShakeTimer);
    pasteShakeTimer = window.setTimeout(() => {
      pasteShake.value = false;
    }, 420);
  });
}

function save() {
  draft.providers = draft.providers.map((provider) => normalizeProviderForSave(provider));
  syncActiveFromOrder();
  emit("save", deepClone(draft));
}

function normalizeProviderForSave(provider) {
  const modelType =
    provider.modelType === "chat"
      ? "chat"
      : normalizeModelType(provider.modelType, provider.imageModel, provider.baseUrl);
  const fallbackModel = modelType === "chat" ? DEFAULT_CHAT_MODEL : DEFAULT_IMAGE_MODEL;
  return {
    ...provider,
    modelType,
    proxyUrl: provider.proxyUrl?.trim() || "",
    imageModel: provider.imageModel?.trim() || fallbackModel,
    imagesConcurrency: 1,
    notes: "",
  };
}

function providerTypeClass(value) {
  return `provider-card--${normalizeModelType(value).replace("image-", "")}`;
}

function maskedApiKey(value) {
  const key = String(value || "");
  if (!key) return "未填写";
  return `${key.slice(0, 6)}******${key.slice(-6)}`;
}
</script>
