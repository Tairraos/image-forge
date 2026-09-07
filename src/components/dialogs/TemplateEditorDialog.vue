<template>
  <NativeDialog v-model:show="show" :title="dialogTitle" class="template-editor-modal">
    <div class="template-edit-body">
      <label class="template-title-field">
        <span>标题</span>
        <!-- eslint-disable vue/no-mutating-props -->
        <input
          v-model="template.title"
          class="form-control"
          :readonly="readonly"
          maxlength="80"
          placeholder="留空时使用内容第一行，最多 24 个字"
        />
        <!-- eslint-enable vue/no-mutating-props -->
      </label>
      <div v-if="readonly" class="template-highlight-box" v-html="highlightedContent"></div>
      <div
        v-else
        class="template-editor-drop-zone"
        :class="{ 'reference-drop-active': referenceDragActive }"
        data-reference-drop-target="template-draft"
        @dragover.prevent="$emit('reference-drag-over')"
        @dragleave="$emit('reference-drag-leave')"
        @drop.prevent="$emit('drop-reference', $event)"
      >
        <!-- eslint-disable vue/no-mutating-props -->
        <textarea
          v-model="template.content"
          class="form-control template-content-input"
          aria-label="模板提示词"
          :readonly="readonly"
          rows="12"
          placeholder="输入模板内容，可使用 {这里写需要 AI 填充的描述}"
          @paste="$emit('paste-reference', $event)"
        ></textarea>
        <!-- eslint-enable vue/no-mutating-props -->
      </div>
      <div class="template-editor-media-row">
        <div class="reference-strip template-editor-reference-strip">
          <ClipboardImageMenu v-if="!readonly" v-slot="{ open }" @paste="$emit('paste-reference')">
            <button
              class="reference-add"
              :class="{ 'reference-drop-active': referenceDragActive }"
              data-reference-drop-target="template-draft"
              type="button"
              title="点击添加，右键粘贴剪贴板图片"
              @dragover.prevent="$emit('reference-drag-over')"
              @dragleave="$emit('reference-drag-leave')"
              @drop.prevent="$emit('drop-reference', $event)"
              @click="$emit('add-reference')"
              @contextmenu="open"
            >
              <Plus :size="18" />
              <span>参考图</span>
            </button>
          </ClipboardImageMenu>
          <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
            <img :src="item.previewUrl" :alt="item.fileName" />
            <button
              v-if="!readonly"
              type="button"
              title="移除参考图"
              @click.stop="$emit('remove-reference', index)"
            >
              <X :size="14" />
            </button>
          </div>
        </div>
        <div class="template-editor-media-actions">
          <div v-if="effectImage" class="reference-tile template-effect-tile">
            <img :src="effectImage.previewUrl" :alt="effectImage.fileName || '模板效果图'" />
            <button
              v-if="!readonly"
              type="button"
              title="移除效果图"
              @click.stop="$emit('remove-effect-image')"
            >
              <X :size="14" />
            </button>
          </div>
          <ClipboardImageMenu
            v-else-if="!readonly"
            v-slot="{ open }"
            @paste="$emit('paste-effect-image')"
          >
            <button
              class="reference-add"
              type="button"
              title="点击添加，右键粘贴剪贴板图片"
              @click="$emit('add-effect-image')"
              @contextmenu="open"
            >
              <Plus :size="18" />
              <span>效果图</span>
            </button>
          </ClipboardImageMenu>
        </div>
      </div>
    </div>
    <template #footer>
      <div class="dialog-actions">
        <button type="button" class="button button-small" @click="show = false">
          {{ readonly ? '关闭' : '取消' }}
        </button>
        <button
          v-if="!readonly"
          type="button"
          class="button button-small button-primary"
          @click="$emit('save')"
        >
          保存
        </button>
      </div>
    </template>
  </NativeDialog>
</template>

<script setup>
import NativeDialog from './NativeDialog.vue';
import { Plus, X } from '@lucide/vue';
import { computed } from 'vue';
import ClipboardImageMenu from '../ClipboardImageMenu.vue';

const show = defineModel('show', { type: Boolean, default: false });

const props = defineProps({
  template: { type: Object, required: true },
  mode: { type: String, default: 'edit' },
  references: { type: Array, default: () => [] },
  effectImage: { type: Object, default: null },
  referenceDragActive: { type: Boolean, default: false },
});

defineEmits([
  'save',
  'add-reference',
  'remove-reference',
  'add-effect-image',
  'remove-effect-image',
  'paste-reference',
  'paste-effect-image',
  'reference-drag-over',
  'reference-drag-leave',
  'drop-reference',
]);

const readonly = computed(() => props.mode === 'view');
const dialogTitle = computed(() => {
  if (props.mode === 'new') return '新增模板';
  if (props.mode === 'view') return '查看模板';
  return '编辑模板';
});
const highlightedContent = computed(() => highlightTemplateText(props.template.content || ''));

function highlightTemplateText(value) {
  return escapeHtml(value).replace(
    /\{[^{}]+\}/g,
    (match) => `<span class="template-token">${match}</span>`
  );
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}
</script>
