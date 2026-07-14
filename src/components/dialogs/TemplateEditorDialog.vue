<template>
  <n-modal v-model:show="show" preset="card" :title="dialogTitle" class="template-editor-modal">
    <div class="template-edit-body">
      <div v-if="readonly" class="template-highlight-box" v-html="highlightedContent"></div>
      <n-input
        v-else
        v-model:value="template.content"
        type="textarea"
        class="template-content-input"
        :autosize="{ minRows: 10, maxRows: 18 }"
        placeholder="输入模板内容，可使用 {这里写需要 AI 填充的描述}"
      />
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
import { computed } from "vue";

const show = defineModel("show", { type: Boolean, default: false });

const props = defineProps({
  template: { type: Object, required: true },
  mode: { type: String, default: "edit" },
});

defineEmits(["save"]);

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
