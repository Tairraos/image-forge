<template>
  <n-modal v-model:show="show" preset="card" :title="dialogTitle" class="template-editor-modal">
    <div class="template-edit-body">
      <label class="template-title-field">
        <span>标题</span>
        <n-input
          v-model:value="template.title"
          :readonly="readonly"
          maxlength="80"
          show-count
          placeholder="留空时使用内容第一行，最多 24 个字"
        />
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
        <n-input
          v-model:value="template.content"
          type="textarea"
          class="template-content-input"
          :autosize="{ minRows: 16, maxRows: 16 }"
          :resizable="false"
          placeholder="输入模板内容，可使用 {这里写需要 AI 填充的描述}"
          @paste="$emit('paste-reference', $event)"
        />
      </div>
      <div class="template-editor-media-row">
        <div class="reference-strip template-editor-reference-strip">
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
          <button
            v-if="!readonly"
            class="reference-add"
            :class="{ 'reference-drop-active': referenceDragActive }"
            data-reference-drop-target="template-draft"
            type="button"
            @dragover.prevent="$emit('reference-drag-over')"
            @dragleave="$emit('reference-drag-leave')"
            @drop.prevent="$emit('drop-reference', $event)"
            @click="$emit('add-reference')"
          >
            <Plus :size="18" />
            <span>参考图</span>
          </button>
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
          <button
            v-else-if="!readonly"
            class="reference-add"
            type="button"
            @click="$emit('add-effect-image')"
          >
            <Plus :size="18" />
            <span>效果图</span>
          </button>
        </div>
      </div>
    </div>
    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" @click="show = false">{{ readonly ? "关闭" : "取消" }}</n-button>
        <n-button v-if="!readonly" size="small" type="primary" @click="$emit('save')">保存</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { Plus, X } from "@lucide/vue";
import { computed } from "vue";

const show = defineModel("show", { type: Boolean, default: false });

const props = defineProps({
  template: { type: Object, required: true },
  mode: { type: String, default: "edit" },
  references: { type: Array, default: () => [] },
  effectImage: { type: Object, default: null },
  referenceDragActive: { type: Boolean, default: false },
});

defineEmits([
  "save",
  "add-reference",
  "remove-reference",
  "add-effect-image",
  "remove-effect-image",
  "paste-reference",
  "reference-drag-over",
  "reference-drag-leave",
  "drop-reference",
]);

const readonly = computed(() => props.mode === "view");
const dialogTitle = computed(() => {
  if (props.mode === "new") return "新增模板";
  if (props.mode === "view") return "查看模板";
  return "编辑模板";
});
const highlightedContent = computed(() => highlightTemplateText(props.template.content || ""));

function highlightTemplateText(value) {
  return escapeHtml(value).replace(/\{[^{}]+\}/g, (match) => `<span class="template-token">${match}</span>`);
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}
</script>
