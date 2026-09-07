<template>
  <div class="agent-composer-area">
    <div
      class="agent-composer"
      data-reference-drop-target="agent"
      :class="{ 'reference-drop-active': dragActive, 'is-busy': busy }"
      @dragover.prevent="dragActive = true"
      @dragleave="dragActive = false"
      @drop.prevent="dropFiles"
    >
      <div v-if="attachments.length" class="agent-reference-overlay">
        <div v-for="attachment in attachments" :key="attachment.id" class="agent-reference-thumb">
          <img :src="attachment.dataUrl" :alt="attachment.fileName" />
          <button
            type="button"
            title="移除参考图"
            aria-label="移除参考图"
            @click.stop="$emit('remove-attachment', attachment.id)"
          >
            <X :size="12" />
          </button>
        </div>
      </div>
      <textarea
        ref="promptInput"
        v-model="draft"
        class="agent-prompt-input"
        rows="3"
        aria-label="提示词"
        placeholder="描述你的想法，或粘贴、拖入参考图…"
        :disabled="busy"
        @paste="handlePaste"
        @keydown="handleKeydown"
      ></textarea>
      <footer class="agent-composer-footer">
        <div class="agent-composer-toolbar">
          <button
            type="button"
            class="agent-toolbar-btn icon-button"
            :disabled="busy"
            title="添加参考图"
            aria-label="添加参考图"
            @click="$emit('add-reference')"
          >
            <ImagePlus :size="18" />
          </button>
          <details
            ref="templateMenu"
            class="template-picker-menu"
            @keydown.esc.prevent.stop="closeTemplateMenu"
          >
            <summary
              class="agent-toolbar-btn"
              :aria-disabled="busy"
              @click="busy && $event.preventDefault()"
            >
              <LayoutTemplate :size="16" /><span>模板</span><ChevronDown :size="12" />
            </summary>
            <div class="template-picker">
              <div class="template-picker-heading">提示词模板</div>
              <div v-if="!templates.length" class="template-picker-empty">
                还没有模板，在设置的模板库中创建后即可使用。
              </div>
              <div v-for="template in templates" :key="template.id" class="template-picker-item">
                <div class="template-picker-main">
                  <strong>{{ template.title || '未命名模板' }}</strong
                  ><span>{{ template.content }}</span>
                </div>
                <div class="template-picker-actions">
                  <button
                    type="button"
                    class="button button-small"
                    @click.stop="insertTemplate(template)"
                  >
                    插入
                  </button>
                  <button
                    v-if="hasPlaceholders(template.content)"
                    type="button"
                    class="button button-small"
                    :disabled="templateFillBusy"
                    @click.stop="fillTemplate(template)"
                  >
                    {{ templateFillBusy ? '填充中…' : 'AI 填充' }}
                  </button>
                </div>
              </div>
            </div>
          </details>
          <span class="composer-toolbar-divider" aria-hidden="true"></span>
          <label class="agent-param-group" title="图片比例">
            <Ratio :size="15" />
            <select
              class="agent-select agent-ratio-select"
              aria-label="图片比例"
              :value="ratio"
              :disabled="busy"
              @change="$emit('update:ratio', $event.target.value)"
            >
              <option v-for="value in RATIO_LIST" :key="value" :value="value">{{ value }}</option>
            </select>
          </label>
          <label class="agent-param-group" title="图片分辨率">
            <select
              class="agent-select agent-resolution-select"
              aria-label="图片分辨率"
              :value="resolution"
              :disabled="busy"
              @change="$emit('update:resolution', $event.target.value)"
            >
              <option
                v-for="option in currentResolutionOptions"
                :key="option.value"
                :value="option.value"
              >
                {{ option.label }}
              </option>
            </select>
          </label>
          <label
            class="composer-draw-toggle"
            :class="{ active: drawThisTurn }"
            title="将提示词直接发送给绘图模型"
          >
            <input v-model="drawThisTurn" type="checkbox" :disabled="busy" />
            <span>直接绘画</span>
          </label>
        </div>
        <button
          v-if="busy"
          type="button"
          class="agent-send-button agent-stop-button"
          title="停止生成"
          aria-label="停止生成"
          @click="$emit('stop')"
        >
          <Square :size="14" fill="currentColor" /><span class="sr-only">停止</span>
        </button>
        <button
          v-else
          type="button"
          class="agent-send-button"
          title="发送 · Enter"
          aria-label="发送"
          :disabled="!draft.trim() || (drawThisTurn ? !imageProviderId : !providerId)"
          @click="send"
        >
          <ArrowUp :size="19" /><span class="sr-only">发送</span>
        </button>
      </footer>
    </div>
    <p class="agent-composer-hint">
      <span>{{ drawThisTurn ? '直接绘画模式' : '与 Agent 一起构思与创作' }}</span
      ><span>Enter 发送 · Shift + Enter 换行</span>
    </p>
  </div>
</template>

<script setup>
import { ArrowUp, ChevronDown, ImagePlus, LayoutTemplate, Ratio, Square, X } from '@lucide/vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { extractDroppedFilePaths } from '../lib/referenceFiles';
import { imageSizePresets } from '../lib/options';

const RATIO_LIST = ['1:1', '9:16', '2:3', '3:4', '4:3', '3:2', '16:9'];
const RESOLUTION_LIST = [
  { value: 'standard', label: '1k' },
  { value: '2k', label: '2k' },
  { value: '3k', label: '3k' },
  { value: '4k', label: '4k' },
];

const props = defineProps({
  providerId: { type: String, default: '' },
  imageProviderId: { type: String, default: '' },
  busy: Boolean,
  attachments: { type: Array, default: () => [] },
  ratio: { type: String, default: '1:1' },
  resolution: { type: String, default: 'standard' },
  templates: { type: Array, default: () => [] },
  templateFillBusy: Boolean,
});

const emit = defineEmits([
  'send',
  'stop',
  'add-reference',
  'paste-reference',
  'drop-reference',
  'remove-attachment',
  'apply-template',
  'fill-template',
  'update:ratio',
  'update:resolution',
]);

const draft = defineModel('draft', { type: String, default: '' });
const promptInput = ref(null);
const templateMenu = ref(null);
const dragActive = ref(false);
const drawThisTurn = defineModel('drawThisTurn', { type: Boolean, default: false });

const currentResolutionOptions = computed(() =>
  RESOLUTION_LIST.map((opt) => {
    const preset = imageSizePresets[opt.value];
    const dims = preset?.[props.ratio] || preset?.['1:1'];
    const label = dims ? `${opt.label.toUpperCase()} · ${dims[0]} × ${dims[1]}` : opt.label;
    return { value: opt.value, label };
  })
);

function send() {
  const content = draft.value.trim();
  const providerId = drawThisTurn.value ? props.imageProviderId : props.providerId;
  if (!content || props.busy || !providerId) return;
  emit('send', { content, drawThisTurn: drawThisTurn.value });
}

function hasPlaceholders(content) {
  const text = String(content || '');
  const start = text.indexOf('{');
  return start >= 0 && text.indexOf('}', start + 1) > start;
}

function insertTemplate(template) {
  const content = String(template?.content || '').trim();
  if (!content) return;
  draft.value = draft.value.trim() ? `${draft.value.trimEnd()}\n\n${content}` : content;
  emit('apply-template', { template });
  closeTemplateMenu();
  promptInput.value?.focus();
}

function handleKeydown(event) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  send();
}

function handlePaste(event) {
  emit('paste-reference', event);
}

function dropFiles(event) {
  dragActive.value = false;
  emit('drop-reference', extractDroppedFilePaths(event.dataTransfer));
}

function closeTemplateMenu(event) {
  if (templateMenu.value) templateMenu.value.open = false;
  if (event?.key === 'Escape') templateMenu.value?.querySelector('summary')?.focus();
}

function fillTemplate(template) {
  closeTemplateMenu();
  emit('fill-template', { template });
}

function closeOutside(event) {
  if (!templateMenu.value?.contains(event.target)) closeTemplateMenu();
}

function resizePrompt() {
  if (!promptInput.value) return;
  promptInput.value.style.height = 'auto';
  promptInput.value.style.height = `${Math.min(220, Math.max(80, promptInput.value.scrollHeight))}px`;
}

watch(draft, async () => {
  await nextTick();
  resizePrompt();
});
onMounted(() => {
  resizePrompt();
  document.addEventListener('pointerdown', closeOutside);
});
onBeforeUnmount(() => document.removeEventListener('pointerdown', closeOutside));
</script>
