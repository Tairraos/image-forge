<template>
  <n-drawer v-model:show="show" :width="620" placement="right">
    <n-drawer-content title="提示词模板">
      <template #header-extra>
        <n-button size="small" type="primary" @click="$emit('new')">
          <template #icon><Plus :size="15" /></template>
          新建
        </n-button>
      </template>
      <n-input v-model:value="query" clearable placeholder="搜索模板" class="drawer-search">
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <div class="template-list">
        <article v-for="template in templates" :key="template.id" class="template-card">
          <header>
            <strong>{{ template.title }}</strong>
            <n-tag size="small">{{ template.category }}</n-tag>
          </header>
          <p>{{ template.content }}</p>
          <footer>
            <n-button size="tiny" secondary @click="$emit('insert', template, false)">插入</n-button>
            <n-button size="tiny" secondary @click="$emit('insert', template, true)">替换</n-button>
            <n-button size="tiny" quaternary @click="$emit('edit', template)">编辑</n-button>
            <n-button size="tiny" quaternary type="error" @click="$emit('delete', template.id)">删</n-button>
          </footer>
        </article>
      </div>
    </n-drawer-content>
  </n-drawer>
</template>

<script setup>
import { Plus, Search } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

defineProps({
  templates: { type: Array, default: () => [] },
});

defineEmits(["new", "insert", "edit", "delete"]);
</script>
