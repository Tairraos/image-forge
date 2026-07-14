<template>
  <n-modal v-model:show="show" preset="card" title="模板维护" class="template-manager-modal">
    <div class="template-manager">
      <div class="template-toolbar">
        <n-input v-model:value="query" clearable placeholder="搜索模板或 ID">
          <template #prefix><Search :size="15" /></template>
        </n-input>
        <n-button size="small" type="primary" @click="$emit('create')">
          <template #icon><Plus :size="15" /></template>
          新增
        </n-button>
      </div>

      <div class="template-table-wrap">
        <table class="template-table">
          <thead>
            <tr>
              <th>参考图</th>
              <th>模板</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="template in templates" :key="template.id">
              <td>{{ referenceCount(template) }}</td>
              <td class="template-content-cell" :title="template.content">{{ singleLine(template.content) }}</td>
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
    </div>
  </n-modal>
</template>

<script setup>
import { Plus, Search } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

defineProps({
  templates: { type: Array, default: () => [] },
});

defineEmits(["create", "view", "edit", "delete"]);

function singleLine(value) {
  return String(value || "").replace(/\s+/g, " ").trim();
}

function referenceCount(template) {
  const count = template.referencePaths?.length || 0;
  return count ? `${count} 个参考图` : "";
}
</script>
