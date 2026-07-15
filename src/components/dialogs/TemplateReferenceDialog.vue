<template>
  <n-modal v-model:show="show" preset="card" title="引用模板" class="template-reference-modal">
    <div class="template-reference-layout">
      <div class="template-reference-toolbar">
        <n-input v-model:value="query" clearable placeholder="搜索标题、模板或 ID">
          <template #prefix><Search :size="15" /></template>
        </n-input>
        <n-select
          :value="selectedTemplateId"
          :options="templateOptions"
          placeholder="选择模板"
          :disabled="!templateOptions.length"
          @update:value="selectTemplate"
        />
      </div>

      <div class="template-reference-editors">
        <div class="template-fill-editor">
          <div
            ref="sourceHighlightRef"
            class="template-fill-highlight"
            v-html="sourceHighlightedContent"
          ></div>
          <textarea
            ref="sourceTextareaRef"
            :value="sourceContent"
            spellcheck="false"
            aria-label="原始模板"
            placeholder="选择模板后在这里预览或编辑原始内容"
            @input="$emit('update:source-content', $event.target.value)"
            @scroll="syncEditorScroll(sourceHighlightRef, sourceTextareaRef)"
          ></textarea>
        </div>
        <div class="template-fill-editor">
          <div
            ref="generatedHighlightRef"
            class="template-fill-highlight"
            v-html="generatedHighlightedContent"
          ></div>
          <textarea
            ref="generatedTextareaRef"
            :value="generatedContent"
            spellcheck="false"
            aria-label="AI 生成内容"
            placeholder="AI 填充后的内容会显示在这里，也可以继续编辑"
            @input="$emit('update:generated-content', $event.target.value)"
            @scroll="syncEditorScroll(generatedHighlightRef, generatedTextareaRef)"
          ></textarea>
        </div>
      </div>

      <div class="reference-strip template-call-reference-strip">
        <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
          <img :src="item.previewUrl" :alt="item.fileName" />
          <button
            type="button"
            title="移除参考图"
            @click.stop="$emit('remove-reference', index)"
          >
            <X :size="14" />
          </button>
        </div>
        <button class="reference-add" type="button" @click="$emit('add-reference')">
          <Plus :size="18" />
          <span>参考图</span>
        </button>
      </div>
    </div>

    <template #footer>
      <div class="template-reference-footer">
        <n-select
          :value="chatProviderId"
          :options="chatProviderOptions"
          size="small"
          class="reference-chat-select"
          placeholder="选择对话模型"
          :disabled="!chatProviderOptions.length"
          @update:value="$emit('update:chat-provider-id', $event)"
        />
        <n-button size="small" secondary :loading="filling" @click="$emit('ai-fill')">AI 填充</n-button>
        <n-button size="small" type="primary" @click="$emit('insert')">引用模板</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { computed, ref } from "vue";
import { Plus, Search, X } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });
const sourceContent = defineModel("sourceContent", { type: String, default: "" });
const generatedContent = defineModel("generatedContent", { type: String, default: "" });

const props = defineProps({
  templates: { type: Array, default: () => [] },
  selectedTemplateId: { type: String, default: "" },
  chatProviderId: { type: String, default: "" },
  chatProviderOptions: { type: Array, default: () => [] },
  filledRanges: { type: Array, default: () => [] },
  filling: { type: Boolean, default: false },
  references: { type: Array, default: () => [] },
});

const emit = defineEmits([
  "update:chat-provider-id",
  "select-template",
  "update:source-content",
  "update:generated-content",
  "ai-fill",
  "insert",
  "add-reference",
  "remove-reference",
]);

const sourceHighlightRef = ref(null);
const sourceTextareaRef = ref(null);
const generatedHighlightRef = ref(null);
const generatedTextareaRef = ref(null);
const templateOptions = computed(() =>
  props.templates.map((template) => ({
    label: template.title || singleLine(template.content),
    value: template.id,
  })),
);
const sourceHighlightedContent = computed(() => highlightTemplateContent(sourceContent.value, []));
const generatedHighlightedContent = computed(() =>
  highlightTemplateContent(generatedContent.value, props.filledRanges),
);

function selectTemplate(templateId) {
  const template = props.templates.find((item) => item.id === templateId);
  if (template) emit("select-template", template);
}

function highlightTemplateContent(value, filledRanges) {
  if (filledRanges.length) {
    return highlightRanges(value, filledRanges);
  }
  return escapeHtml(value).replace(/\{[^{}]+\}/g, (match) => `<span class="template-token">${match}</span>`);
}

function highlightRanges(value, ranges) {
  const text = String(value || "");
  const normalized = [...ranges]
    .filter((range) => Number.isFinite(range.start) && Number.isFinite(range.end) && range.end > range.start)
    .sort((left, right) => left.start - right.start);
  let cursor = 0;
  let html = "";
  for (const range of normalized) {
    const start = Math.max(cursor, range.start);
    const end = Math.min(text.length, range.end);
    html += escapeHtml(text.slice(cursor, start));
    html += `<span class="template-token">${escapeHtml(text.slice(start, end))}</span>`;
    cursor = end;
  }
  html += escapeHtml(text.slice(cursor));
  return html;
}

function singleLine(value) {
  return String(value || "").replace(/\s+/g, " ").trim();
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function syncEditorScroll(highlightRef, textareaRef) {
  if (!highlightRef || !textareaRef) return;
  highlightRef.scrollTop = textareaRef.scrollTop;
  highlightRef.scrollLeft = textareaRef.scrollLeft;
}
</script>
