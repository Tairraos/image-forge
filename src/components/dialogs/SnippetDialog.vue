<template>
  <n-modal v-model:show="show" preset="card" title="提示词片段" class="wide-modal">
    <div class="snippet-toolbar">
      <n-input v-model:value="query" clearable placeholder="搜索片段">
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <n-button size="small" type="primary" @click="$emit('new')">
        <template #icon><Plus :size="15" /></template>
        新建
      </n-button>
    </div>
    <div class="snippet-grid">
      <article v-for="snippet in snippets" :key="snippet.id" class="snippet-card">
        <header>
          <strong>~{{ snippet.tag }}</strong>
          <n-tag size="small">{{ snippet.category }}</n-tag>
        </header>
        <p>{{ snippet.content }}</p>
        <footer>
          <n-button size="tiny" secondary @click="$emit('insert', snippet.content)">插入</n-button>
          <n-button size="tiny" quaternary @click="$emit('edit', snippet)">编辑</n-button>
          <n-button size="tiny" quaternary type="error" @click="$emit('delete', snippet.id)">删</n-button>
        </footer>
      </article>
    </div>
  </n-modal>
</template>

<script setup>
import { Plus, Search } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

defineProps({
  snippets: { type: Array, default: () => [] },
});

defineEmits(["new", "insert", "edit", "delete"]);
</script>
