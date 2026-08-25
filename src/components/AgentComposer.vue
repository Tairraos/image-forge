<template>
  <div
    class="agent-composer"
    data-reference-drop-target="agent"
    :class="{ 'reference-drop-active': dragActive }"
    @dragover.prevent="dragActive = true"
    @dragleave="dragActive = false"
    @drop.prevent="dropFiles"
  >
    <div class="agent-composer-body">
      <div class="agent-composer-input-wrap" :class="{ 'has-reference': attachments.length }">
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
        <n-input
          v-model:value="draft"
          type="textarea"
          :autosize="{ minRows: 2, maxRows: 5 }"
          placeholder="输入消息；可粘贴或拖入参考图"
          :disabled="busy"
          @paste="handlePaste"
          @keydown="handleKeydown"
        />
      </div>
    </div>
    <footer class="agent-composer-footer">
      <div class="agent-composer-toolbar">
        <button
          type="button"
          class="agent-toolbar-btn"
          :disabled="busy"
          @click="$emit('add-reference')"
        >
          <ImagePlus :size="15" />
          <span>参考图</span>
        </button>
        <button
          type="button"
          class="agent-toolbar-btn"
          :disabled="busy"
          @click="$emit('select-template')"
        >
          <LayoutTemplate :size="15" />
          <span>模板</span>
        </button>

        <div class="agent-param-group">
          <span class="agent-param-label">比例</span>
          <n-select
            class="agent-select agent-ratio-select"
            size="small"
            :value="ratio"
            :options="ratioOptions"
            :render-label="renderRatioLabel"
            :disabled="busy"
            @update:value="$emit('update:ratio', $event)"
          />
        </div>

        <div class="agent-param-group">
          <span class="agent-param-label">分辨率</span>
          <n-select
            class="agent-select agent-resolution-select"
            size="small"
            :value="resolution"
            :options="currentResolutionOptions"
            :disabled="busy"
            @update:value="$emit('update:resolution', $event)"
          />
        </div>
      </div>
      <div class="agent-send-stack">
        <n-button v-if="busy" size="small" type="error" secondary @click="$emit('stop')"
          >停止</n-button
        >
        <n-button
          v-else
          class="agent-send-button"
          size="small"
          type="primary"
          :disabled="!draft.trim() || (drawThisTurn ? !imageProviderId : !providerId)"
          @click="send"
          >发送</n-button
        >
        <n-checkbox v-model:checked="drawThisTurn" :disabled="busy">直接绘画</n-checkbox>
      </div>
    </footer>
  </div>
</template>

<script setup>
import { ImagePlus, LayoutTemplate, X } from '@lucide/vue';
import { computed, h, ref, watch } from 'vue';
import { extractDroppedFilePaths } from '../lib/referenceFiles';
import { imageSizePresets } from '../lib/options';

const RATIO_LIST = ['1:1', '9:16', '2:3', '3:4', '4:3', '3:2', '16:9'];
const RESOLUTION_LIST = [
  { value: 'standard', label: '1k' },
  { value: '2k', label: '2k' },
  { value: '3k', label: '3k' },
  { value: '4k', label: '4k' },
];

function ratioSvg(ratio) {
  const [w, h] = ratio.split(':').map(Number);
  const maxDim = w === h ? 14 : 18;
  let rw, rh;
  if (w >= h) {
    rw = maxDim;
    rh = Math.round((maxDim * h) / w);
  } else {
    rh = maxDim;
    rw = Math.round((maxDim * w) / h);
  }
  const x = ((24 - rw) / 2).toFixed(1);
  const y = ((24 - rh) / 2).toFixed(1);
  return `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="${x}" y="${y}" width="${rw}" height="${rh}" rx="2" stroke="currentColor" stroke-width="1.5" fill="none"/></svg>`;
}

const ratioOptions = RATIO_LIST.map((value) => ({
  value,
  label: value,
  icon: ratioSvg(value),
}));

function renderRatioLabel(option) {
  return h('span', { class: 'agent-ratio-option' }, [
    h('span', { class: 'agent-ratio-icon', innerHTML: option.icon }),
    h('span', { class: 'agent-ratio-text' }, option.label),
  ]);
}

const props = defineProps({
  providerId: { type: String, default: '' },
  imageProviderId: { type: String, default: '' },
  busy: Boolean,
  attachments: { type: Array, default: () => [] },
  ratio: { type: String, default: '1:1' },
  resolution: { type: String, default: 'standard' },
  prefillPrompt: { type: String, default: '' },
});

const emit = defineEmits([
  'send',
  'stop',
  'add-reference',
  'paste-reference',
  'drop-reference',
  'remove-attachment',
  'select-template',
  'update:ratio',
  'update:resolution',
]);

const draft = ref('');
const dragActive = ref(false);
const drawThisTurn = ref(false);

watch(
  () => props.prefillPrompt,
  (value) => {
    if (value) {
      draft.value = value;
    }
  }
);

const currentResolutionOptions = computed(() =>
  RESOLUTION_LIST.map((opt) => {
    const preset = imageSizePresets[opt.value];
    const dims = preset?.[props.ratio] || preset?.['1:1'];
    const label = dims ? `${dims[0]} x ${dims[1]} ${opt.label}` : opt.label;
    return { value: opt.value, label };
  })
);

function send() {
  const content = draft.value.trim();
  const providerId = drawThisTurn.value ? props.imageProviderId : props.providerId;
  if (!content || props.busy || !providerId) return;
  emit('send', { content, drawThisTurn: drawThisTurn.value });
  draft.value = '';
  drawThisTurn.value = false;
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
</script>
