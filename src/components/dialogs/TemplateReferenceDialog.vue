<template>
  <n-modal v-model:show="show" preset="card" title="引用模板" class="template-reference-modal">
    <template #header-extra>
      <n-select
        :value="chatProviderId"
        :options="chatProviderOptions"
        size="small"
        class="reference-chat-select"
        placeholder="选择对话模型"
        :disabled="!chatProviderOptions.length"
        @update:value="$emit('update:chat-provider-id', $event)"
      />
    </template>

    <div class="template-reference-layout">
      <section class="template-reference-list">
        <n-input v-model:value="query" clearable placeholder="搜索模板或 ID">
          <template #prefix><Search :size="15" /></template>
        </n-input>
        <div class="template-table-wrap reference-table-wrap">
          <table class="template-table reference-template-table">
            <thead>
              <tr>
                <th>id</th>
                <th>模板</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="template in templates"
                :key="template.id"
                :class="{ selected: template.id === selectedTemplateId }"
                @click="$emit('select-template', template)"
              >
                <td :title="template.id">{{ template.id }}</td>
                <td class="template-content-cell" :title="template.content">{{ singleLine(template.content) }}</td>
              </tr>
              <tr v-if="!templates.length">
                <td colspan="2" class="template-empty-cell">没有模板</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section class="template-reference-preview">
        <div class="template-fill-editor">
          <div ref="fillHighlightRef" class="template-fill-highlight" v-html="highlightedContent"></div>
          <textarea
            ref="fillTextareaRef"
            :value="content"
            spellcheck="false"
            placeholder="选择模板后在这里预览或调整内容"
            @input="$emit('update:content', $event.target.value)"
            @scroll="syncFillScroll"
          ></textarea>
        </div>
        <div class="dialog-actions template-fill-actions">
          <n-button size="small" secondary :loading="filling" @click="$emit('ai-fill')">AI 填充</n-button>
          <n-button size="small" type="primary" @click="$emit('insert')">引用模板</n-button>
        </div>
      </section>
    </div>
  </n-modal>
</template>

<script setup>
import { computed, ref } from "vue";
import { Search } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });
const content = defineModel("content", { type: String, default: "" });

const props = defineProps({
  templates: { type: Array, default: () => [] },
  selectedTemplateId: { type: String, default: "" },
  chatProviderId: { type: String, default: "" },
  chatProviderOptions: { type: Array, default: () => [] },
  filledRanges: { type: Array, default: () => [] },
  filling: { type: Boolean, default: false },
});

defineEmits([
  "update:chat-provider-id",
  "select-template",
  "update:content",
  "ai-fill",
  "insert",
]);

const fillHighlightRef = ref(null);
const fillTextareaRef = ref(null);
const highlightedContent = computed(() => highlightTemplateContent(props.content, props.filledRanges));

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

function syncFillScroll() {
  if (!fillHighlightRef.value || !fillTextareaRef.value) return;
  fillHighlightRef.value.scrollTop = fillTextareaRef.value.scrollTop;
  fillHighlightRef.value.scrollLeft = fillTextareaRef.value.scrollLeft;
}
</script>
