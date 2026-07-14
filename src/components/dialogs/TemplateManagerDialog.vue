<template>
  <n-modal v-model:show="show" preset="card" title="模板维护" class="template-manager-modal">
    <div class="template-manager">
      <n-form label-placement="top" :show-feedback="false">
        <n-form-item label="对话模型">
          <n-select
            :value="chatProviderId"
            :options="chatProviderOptions"
            size="small"
            placeholder="选择对话模型"
            :disabled="!chatProviderOptions.length"
            @update:value="$emit('update:chat-provider-id', $event)"
          />
        </n-form-item>
      </n-form>

      <n-input v-model:value="query" clearable placeholder="搜索模板">
        <template #prefix><Search :size="15" /></template>
      </n-input>

      <div class="template-table-wrap">
        <table class="template-table">
          <thead>
            <tr>
              <th>id</th>
              <th>模板</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="template in templates" :key="template.id">
              <td :title="template.id">{{ template.id }}</td>
              <td :title="template.content || template.title">
                {{ truncateText(template.content || template.title || "") }}
              </td>
              <td>
                <div class="template-table-actions">
                  <n-button size="tiny" secondary @click="$emit('view', template)">查看</n-button>
                  <n-button size="tiny" secondary @click="$emit('edit', template)">编辑</n-button>
                  <n-button size="tiny" quaternary type="error" @click="$emit('delete', template.id)">
                    删除
                  </n-button>
                </div>
              </td>
            </tr>
            <tr v-if="!templates.length">
              <td colspan="3" class="template-empty-cell">没有模板</td>
            </tr>
          </tbody>
        </table>
      </div>

      <n-input
        v-model:value="selectedContent"
        type="textarea"
        :autosize="{ minRows: 6, maxRows: 10 }"
        placeholder="查看或输入完整模板内容"
      />

      <div class="dialog-actions">
        <n-button size="small" type="primary" @click="$emit('create', selectedContent)">新增</n-button>
        <n-button size="small" secondary @click="$emit('insert', contentAsTemplate, false)">引用</n-button>
        <n-button size="small" secondary @click="$emit('ai-fill')">AI 填充</n-button>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { computed } from "vue";
import { Search } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });
const selectedContent = defineModel("selectedContent", { type: String, default: "" });

const props = defineProps({
  chatProviderId: { type: String, default: "" },
  chatProviderOptions: { type: Array, default: () => [] },
  templates: { type: Array, default: () => [] },
});

const emit = defineEmits([
  "update:chat-provider-id",
  "view",
  "edit",
  "delete",
  "create",
  "insert",
  "ai-fill",
]);

const contentAsTemplate = computed(() => ({
  id: "",
  title: "当前模板内容",
  content: selectedContent.value,
}));

function truncateText(value) {
  const text = String(value || "");
  return text.length > 20 ? `${text.slice(0, 20)}...` : text;
}
</script>
