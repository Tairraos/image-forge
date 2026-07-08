<template>
  <n-drawer v-model:show="show" :width="560" placement="right">
    <n-drawer-content title="图库">
      <template #header-extra>
        <n-button size="small" type="primary" @click="$emit('add')">
          <template #icon><Plus :size="15" /></template>
          添加
        </n-button>
      </template>
      <n-input v-model:value="query" clearable placeholder="搜索图库" class="drawer-search">
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <div class="gallery-grid">
        <article v-for="item in items" :key="item.id" class="gallery-card">
          <img :src="fileUrl(item.path)" :alt="item.name" />
          <div>
            <strong>{{ item.name }}</strong>
            <span>{{ item.category }}</span>
            <p v-if="item.note">{{ item.note }}</p>
          </div>
          <footer>
            <n-button size="tiny" secondary @click="$emit('use', item)">使用</n-button>
            <n-button size="tiny" quaternary @click="$emit('edit', item)">编辑</n-button>
            <n-button size="tiny" quaternary type="error" @click="$emit('delete', item.id)">删</n-button>
          </footer>
        </article>
      </div>
    </n-drawer-content>
  </n-drawer>
</template>

<script setup>
import { Plus, Search } from "@lucide/vue";
import { fileUrl } from "../../lib/formatters";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

defineProps({
  items: { type: Array, default: () => [] },
});

defineEmits(["add", "use", "edit", "delete"]);
</script>
