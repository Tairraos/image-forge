<template>
  <div class="api-manager">
    <header class="api-manager-head">
      <div class="api-manager-title">
        <span>API 源</span>
        <span class="api-manager-count">{{ visibleProviders.length }} 项</span>
        <span
          v-if="listMessage"
          class="api-manager-message"
          :data-tone="listMessageTone"
          role="status"
        >
          {{ listMessage }}
        </span>
      </div>
      <div class="api-manager-actions">
        <button type="button" class="button button-small" @click="pasteProvider">
          <ClipboardPaste :size="15" />粘贴配置
        </button>
        <button
          type="button"
          class="button button-small"
          :disabled="!expandedProvider"
          @click="cloneProvider"
        >
          <Copy :size="14" />
          克隆
        </button>
        <button type="button" class="button button-small button-primary" @click="addProvider">
          <Plus :size="15" />新增 API
        </button>
      </div>
    </header>

    <p class="api-manager-hint">排在首位的 API 作为默认来源，可拖动或用箭头调整顺序。</p>
    <div class="api-list" data-persistent-scrollbar>
      <div
        v-for="(provider, index) in visibleProviders"
        :key="provider.id"
        class="api-item"
        :class="{
          expanded: expandedId === provider.id,
          'drag-over': dragOverId === provider.id,
          dragging: dragId === provider.id,
        }"
        @dragover.prevent="onDragOver(provider.id)"
        @drop.prevent="onDrop(provider.id)"
      >
        <div class="api-row" :class="{ active: expandedId === provider.id }">
          <button
            type="button"
            class="api-row-handle"
            title="拖动调整顺序"
            aria-label="拖动调整顺序"
            draggable="true"
            @dragstart="onDragStart(provider.id, $event)"
            @dragend="onDragEnd"
          >
            <GripVertical :size="15" />
          </button>
          <button
            type="button"
            class="api-row-main"
            :aria-expanded="expandedId === provider.id"
            :aria-controls="`api-details-${provider.id}`"
            :title="expandedId === provider.id && expandedMode === 'edit' ? '' : '点击查看详情'"
            @click="toggleDetail(provider.id)"
          >
            <strong>
              <span class="api-row-name" :title="provider.name || '未命名 API 源'">
                {{ provider.name || '未命名 API 源' }}
              </span>
              <em v-if="index === 0" class="provider-default-badge">默认</em>
            </strong>
            <span class="api-row-model" :title="provider.imageModel || '未设置模型'">
              {{ provider.imageModel || '未设置模型' }}
            </span>
          </button>
          <div class="api-row-actions">
            <button
              type="button"
              title="上移"
              aria-label="上移"
              :disabled="index === 0"
              @click="moveProvider(provider.id, -1)"
            >
              <ArrowUp :size="14" />
            </button>
            <button
              type="button"
              title="下移"
              aria-label="下移"
              :disabled="index === visibleProviders.length - 1"
              @click="moveProvider(provider.id, 1)"
            >
              <ArrowDown :size="14" />
            </button>
            <button
              v-if="expandedId === provider.id && expandedMode === 'edit'"
              type="button"
              title="保存"
              aria-label="保存"
              class="primary"
              @click="saveEdits"
            >
              <Save :size="14" />
            </button>
            <button
              v-else
              type="button"
              title="编辑"
              aria-label="编辑"
              @click="openEdit(provider.id)"
            >
              <Pencil :size="14" />
            </button>
            <button
              type="button"
              title="删除"
              aria-label="删除"
              class="danger"
              :disabled="draft.providers.length <= 1"
              @click="deleteProvider(provider.id)"
            >
              <Trash2 :size="14" />
            </button>
          </div>
        </div>

        <div
          v-if="expandedId === provider.id"
          :id="`api-details-${provider.id}`"
          class="api-drawer"
        >
          <!-- 只读详情 -->
          <dl v-if="expandedMode !== 'edit'" class="api-drawer-readonly">
            <div>
              <dt>名称</dt>
              <dd>{{ provider.name || '未命名 API 源' }}</dd>
            </div>
            <div>
              <dt>Base URL</dt>
              <dd>{{ provider.baseUrl || '未设置' }}</dd>
            </div>
            <div>
              <dt>API Key</dt>
              <dd>{{ maskedApiKey(provider.apiKey) }}</dd>
            </div>
            <div>
              <dt>代理地址</dt>
              <dd>{{ provider.proxyUrl || '未设置' }}</dd>
            </div>
            <div>
              <dt>模型</dt>
              <dd>{{ provider.imageModel || '未设置模型' }}</dd>
            </div>
            <div>
              <dt>模型类型</dt>
              <dd>{{ modelTypeLabel(provider.modelType) }}</dd>
            </div>
            <div v-if="kind === 'chat'">
              <dt>视觉输入</dt>
              <dd>{{ provider.chatVision ? '支持' : '不支持' }}</dd>
            </div>
          </dl>

          <!-- 编辑表单 -->
          <form v-else class="provider-form" @submit.prevent="saveEdits">
            <label class="form-field"
              ><span>名称</span
              ><input
                v-model="provider.name"
                class="form-control"
                placeholder="例如 OpenAI / 自建服务"
            /></label>
            <label class="form-field"
              ><span>Base URL</span
              ><input
                v-model="provider.baseUrl"
                class="form-control"
                placeholder="https://api.openai.com/v1"
                spellcheck="false"
            /></label>
            <label class="form-field"
              ><span>API Key</span
              ><span class="password-field"
                ><input
                  v-model="provider.apiKey"
                  class="form-control"
                  :type="passwordVisible ? 'text' : 'password'"
                  aria-label="API Key"
                  placeholder="sk-..."
                  autocomplete="off"
                  spellcheck="false" /><button
                  type="button"
                  class="icon-button"
                  :title="passwordVisible ? '隐藏 API Key' : '显示 API Key'"
                  :aria-label="passwordVisible ? '隐藏 API Key' : '显示 API Key'"
                  :aria-pressed="passwordVisible"
                  @click="passwordVisible = !passwordVisible"
                >
                  <EyeOff v-if="passwordVisible" :size="16" /><Eye
                    v-else
                    :size="16"
                  /></button></span
            ></label>
            <label class="form-field"
              ><span>代理地址 <small>可选</small></span
              ><input
                v-model="provider.proxyUrl"
                class="form-control"
                placeholder="http://127.0.0.1:7890"
                spellcheck="false"
            /></label>
            <div class="form-field form-field-wide">
              <label :for="`${modelListId}-input`">模型</label>
              <div class="model-select-row">
                <input
                  :id="`${modelListId}-input`"
                  :value="provider.imageModel"
                  class="form-control"
                  :list="modelListId"
                  placeholder="输入模型 ID，或从列表选择"
                  spellcheck="false"
                  @input="updateSelectedModel($event.target.value)"
                />
                <datalist :id="modelListId">
                  <option
                    v-for="option in modelOptions"
                    :key="option.value"
                    :value="option.value"
                  ></option>
                </datalist>
                <button
                  type="button"
                  class="button"
                  :disabled="loadingModels"
                  :aria-busy="loadingModels"
                  @click="fetchModels"
                >
                  <RefreshCw :size="14" :class="{ spinning: loadingModels }" />{{
                    loadingModels ? '获取中' : '获取模型'
                  }}
                </button>
              </div>
            </div>
            <label class="form-field"
              ><span>模型类型</span>
              <select
                v-if="kind === 'image'"
                class="form-control"
                :value="provider.modelType"
                @change="updateSelectedModelType($event.target.value)"
              >
                <option
                  v-for="option in imageModelTypeOptions"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </option>
              </select>
              <input v-else class="form-control" :value="chatModelTypeLabel" readonly disabled />
            </label>
            <label v-if="kind === 'chat'" class="checkbox-field provider-vision"
              ><input v-model="provider.chatVision" type="checkbox" /><span
                >支持图片理解<small>向对话模型发送参考图</small></span
              ></label
            >
            <p
              v-if="modelFetchMessage"
              class="model-fetch-message form-field-wide"
              :data-tone="modelFetchTone"
              role="status"
            >
              {{ modelFetchMessage }}
            </p>
            <div class="provider-form-actions form-field-wide">
              <button type="submit" class="button button-primary">
                <Save :size="15" />保存配置
              </button>
            </div>
          </form>
        </div>
      </div>
      <div v-if="!visibleProviders.length" class="provider-empty">
        <Plug :size="28" /><strong>连接你的{{ kindLabel }}</strong>
        <p>添加 API 地址与密钥，即可开始创作。</p>
        <button type="button" class="button button-primary" @click="addProvider">
          <Plus :size="15" />添加 API 源
        </button>
      </div>
    </div>
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
import { computed, reactive, ref, useId, watch } from 'vue';
import {
  ArrowDown,
  ArrowUp,
  ClipboardPaste,
  Copy,
  Eye,
  EyeOff,
  GripVertical,
  Pencil,
  Plug,
  Plus,
  RefreshCw,
  Save,
  Trash2,
} from '@lucide/vue';
import ConfirmDialog from './ConfirmDialog.vue';
import * as api from '../../api/index.js';
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
} from '../../lib/models';

const props = defineProps({
  show: { type: Boolean, default: false },
  settings: { type: Object, required: true },
  kind: { type: String, default: 'image' },
});

const emit = defineEmits(['close', 'save']);

const draft = reactive(defaultSettings());
const modelListId = useId();
const passwordVisible = ref(false);
/** 抽屉展开的 API 项 id；空串表示全部收起 */
const expandedId = ref('');
/** 抽屉模式：readonly 只读详情 / edit 编辑表单 */
const expandedMode = ref('readonly');
const providerModels = reactive({});
const loadingModels = ref(false);
const modelFetchMessage = ref('');
const modelFetchTone = ref('idle');
const showDeleteConfirmation = ref(false);
const pendingDeleteProviderId = ref('');
const dragId = ref('');
const dragOverId = ref('');
const listMessage = ref('');
const listMessageTone = ref('idle');
let listMessageTimer = 0;

const kindLabel = computed(() => (props.kind === 'chat' ? '对话 API' : '绘图 API'));
/** 绘图模型类型下拉：菜单显示 label，绑定 value（如 image-gpt） */
const imageModelTypeOptions = IMAGE_MODEL_TYPE_OPTIONS;
/**
 * 对话模型类型只读展示。
 * UI 文案（label）固定为「对话模型」；概念 value 为 text。
 * 持久化字段 modelType 仍写 "chat"，与后端/过滤逻辑兼容。
 */
const chatModelTypeLabel = '对话模型';

const visibleProviders = computed(() =>
  draft.providers.filter((provider) => matchesKind(provider.modelType))
);

const expandedProvider = computed(
  () => visibleProviders.value.find((provider) => provider.id === expandedId.value) || null
);

const modelOptions = computed(() => {
  const provider = expandedProvider.value;
  const models = new Set(providerModels[provider?.id] || []);
  if (provider?.imageModel) models.add(provider.imageModel);
  return Array.from(models).map((model) => ({ label: model, value: model }));
});

const deleteConfirmationMessage = computed(() => {
  const provider = draft.providers.find((item) => item.id === pendingDeleteProviderId.value);
  return `确认删除 API 源「${provider?.name || '未命名 API 源'}」？`;
});

watch(
  () => [props.show, props.kind],
  ([show]) => {
    if (!show) {
      cancelDeleteProvider();
      return;
    }
    Object.assign(draft, normalizeSettingsForUi(deepClone(props.settings)));
    expandedId.value = '';
    expandedMode.value = 'readonly';
    modelFetchMessage.value = '';
    listMessage.value = '';
    dragId.value = '';
    dragOverId.value = '';
  },
  { immediate: true }
);

function matchesKind(modelType) {
  return props.kind === 'chat' ? modelType === 'chat' : isImageModelType(modelType);
}

function modelTypeLabel(value) {
  if (value === 'chat') return chatModelTypeLabel;
  const normalized = normalizeModelType(value);
  return (
    IMAGE_MODEL_TYPE_OPTIONS.find((option) => option.value === normalized)?.label || normalized
  );
}

function toggleDetail(id) {
  // 编辑中的抽屉不响应标题点击，避免误触丢失编辑焦点。
  if (expandedId.value === id && expandedMode.value === 'edit') return;
  expandedId.value = expandedId.value === id ? '' : id;
  expandedMode.value = 'readonly';
  modelFetchMessage.value = '';
}

function openEdit(id) {
  passwordVisible.value = false;
  expandedId.value = id;
  expandedMode.value = 'edit';
  modelFetchMessage.value = '';
}

function persistDraft() {
  draft.providers = draft.providers.map((provider) => normalizeProviderForSave(provider));
  syncActiveFromOrder();
  emit('save', deepClone(draft));
}

function saveEdits() {
  persistDraft();
  expandedId.value = '';
  expandedMode.value = 'readonly';
}

function addProvider() {
  const provider = defaultProvider(
    draft.providers.length + 1,
    props.kind === 'chat' ? 'chat' : 'image-gpt'
  );
  if (props.kind === 'chat') {
    provider.imageModel = provider.imageModel || DEFAULT_CHAT_MODEL;
  }
  provider.imagesConcurrency = 1;
  provider.notes = '';
  draft.providers.push(provider);
  syncActiveFromOrder();
  openEdit(provider.id);
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
  let text;
  try {
    // 桌面端走 Tauri 原生剪贴板，避免浏览器权限限制
    text = await api.readClipboardText();
  } catch {
    showListMessage('读取剪贴板失败', 'error');
    return;
  }

  // 智能解析：JSON 配置 或 自由文本（http(s)/sk-/名称/image 模型）
  const parsed = parseClipboardProvider(text);
  if (!parsed) {
    showListMessage('剪贴板中没有可识别的 API 配置', 'error');
    return;
  }

  const provider = addProvider();
  // 名称为空时保留 addProvider 生成的默认名（如「供应商 N」）
  if (parsed.name) provider.name = parsed.name;
  provider.baseUrl = parsed.baseUrl;
  provider.apiKey = parsed.apiKey;

  if (props.kind === 'chat') {
    provider.modelType = 'chat';
    // 对话源：剪贴板若带了模型名也写入，否则留给 fetchModels 自动选
    provider.imageModel = parsed.imageModel || '';
  } else {
    // 绘图源：写入模型名，并据此（+ baseUrl）推测模型类型
    provider.imageModel = parsed.imageModel || '';
    provider.modelType = recommendImageModelType(provider.imageModel, provider.baseUrl);
  }

  // 剪贴板已给出模型名则保留；否则拉取列表后自动选第一个
  await fetchModels({ autoSelectFirst: !provider.imageModel });
}

function updateSelectedModel(value) {
  const provider = expandedProvider.value;
  if (!provider || expandedMode.value !== 'edit') return;
  provider.imageModel = String(value || '');
  // 切换模型时自动推荐类型；用户仍可通过「模型类型」下拉手动覆盖
  if (props.kind === 'image') {
    provider.modelType = recommendImageModelType(provider.imageModel, provider.baseUrl);
  } else {
    provider.modelType = 'chat';
  }
}

/** 绘图 API：用户手动选择模型类型（不再被其它逻辑强制改回，除非再次改模型名） */
function updateSelectedModelType(value) {
  const provider = expandedProvider.value;
  if (!provider || props.kind !== 'image' || expandedMode.value !== 'edit') return;
  provider.modelType = normalizeModelType(value, provider.imageModel, provider.baseUrl);
}

function cloneProvider() {
  const source = expandedProvider.value;
  if (!source) return;
  const provider = normalizeProviderForSave(deepClone(source));
  provider.id = createProviderId();
  provider.name = `${source.name || 'API 源'} 副本`;
  draft.providers.push(provider);
  syncActiveFromOrder();
  persistDraft();
  openEdit(provider.id);
}

function deleteProvider(id) {
  if (draft.providers.length <= 1) return;
  const index = draft.providers.findIndex((provider) => provider.id === id);
  if (index < 0) return;
  pendingDeleteProviderId.value = id;
  showDeleteConfirmation.value = true;
}

function confirmDeleteProvider() {
  const index = draft.providers.findIndex(
    (provider) => provider.id === pendingDeleteProviderId.value
  );
  cancelDeleteProvider();
  if (index < 0 || draft.providers.length <= 1) return;
  draft.providers.splice(index, 1);
  if (expandedId.value && !draft.providers.some((provider) => provider.id === expandedId.value)) {
    expandedId.value = '';
    expandedMode.value = 'readonly';
  }
  syncActiveFromOrder();
  persistDraft();
}

function cancelDeleteProvider() {
  showDeleteConfirmation.value = false;
  pendingDeleteProviderId.value = '';
}

function moveProvider(id, offset) {
  const ids = visibleProviders.value.map((provider) => provider.id);
  const index = ids.indexOf(id);
  const nextIndex = index + offset;
  if (index < 0 || nextIndex < 0 || nextIndex >= ids.length) return;
  const ordered = ids.slice();
  const [item] = ordered.splice(index, 1);
  ordered.splice(nextIndex, 0, item);
  applyVisibleOrder(ordered);
  syncActiveFromOrder();
  persistDraft();
}

function onDragStart(id, event) {
  dragId.value = id;
  dragOverId.value = '';
  if (event?.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', id);
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
  syncActiveFromOrder();
  persistDraft();
  onDragEnd();
}

function onDragEnd() {
  dragId.value = '';
  dragOverId.value = '';
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
  const chatProvider = draft.providers.find((provider) => provider.modelType === 'chat');
  draft.activeImageProviderId = imageProvider?.id || '';
  draft.activeChatProviderId = chatProvider?.id || '';
  draft.activeProviderId = draft.activeImageProviderId || draft.providers[0]?.id || '';
}

async function fetchModels({ autoSelectFirst = false } = {}) {
  const provider = expandedProvider.value;
  if (!provider || expandedMode.value !== 'edit') return;
  loadingModels.value = true;
  modelFetchMessage.value = '';
  try {
    const models = await api.listProviderModels(normalizeProviderForSave(deepClone(provider)));
    providerModels[provider.id] = models;
    modelFetchTone.value = 'ok';
    modelFetchMessage.value = models.length ? `已获取 ${models.length} 个模型` : '模型列表为空';
    if (autoSelectFirst && models.length) {
      updateSelectedModel(models[0]);
    }
  } catch (error) {
    modelFetchTone.value = 'error';
    modelFetchMessage.value = String(error);
  } finally {
    loadingModels.value = false;
  }
}

function showListMessage(message, tone = 'idle') {
  listMessage.value = message;
  listMessageTone.value = tone;
  window.clearTimeout(listMessageTimer);
  listMessageTimer = window.setTimeout(() => {
    listMessage.value = '';
  }, 2600);
}

function normalizeProviderForSave(provider) {
  const modelType =
    provider.modelType === 'chat'
      ? 'chat'
      : normalizeModelType(provider.modelType, provider.imageModel, provider.baseUrl);
  const fallbackModel = modelType === 'chat' ? DEFAULT_CHAT_MODEL : DEFAULT_IMAGE_MODEL;
  return {
    ...provider,
    modelType,
    proxyUrl: provider.proxyUrl?.trim() || '',
    imageModel: provider.imageModel?.trim() || fallbackModel,
    imagesConcurrency: 1,
    notes: '',
  };
}

function maskedApiKey(value) {
  const key = String(value || '');
  if (!key) return '未填写';
  return `${key.slice(0, 6)}******${key.slice(-6)}`;
}
</script>
