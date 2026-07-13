<template>
  <aside class="queue-column">
    <div class="section-head">
      <div>
        <h2>生成历史</h2>
        <p>{{ historyQuery ? "搜索结果" : "从旧到新" }}</p>
      </div>
      <n-button circle quaternary size="small" @click="$emit('refresh')">
        <template #icon><RotateCcw :size="16" /></template>
      </n-button>
    </div>

    <n-input
      :value="historyQuery"
      size="small"
      clearable
      placeholder="搜索提示词或任务 ID"
      @update:value="$emit('update:history-query', $event)"
    >
      <template #prefix><Search :size="15" /></template>
    </n-input>

    <div ref="historyListRef" class="task-stack history-stack">
      <TaskCard
        v-for="task in filteredHistory"
        :key="task.id"
        :task="task"
        :selected="selectedTaskId === task.id"
        @select="$emit('select-task', task.id)"
        @retry="$emit('retry', $event)"
        @delete="$emit('delete', $event)"
        @download-output="$emit('download-output', $event)"
      />
      <div v-if="!filteredHistory.length" class="empty-panel compact">没有生成历史</div>
    </div>
  </aside>
</template>

<script setup>
import { nextTick, ref, watch } from "vue";
import { RotateCcw, Search } from "@lucide/vue";
import TaskCard from "./TaskCard.vue";

const props = defineProps({
  filteredHistory: { type: Array, default: () => [] },
  selectedTaskId: { type: String, default: "" },
  historyQuery: { type: String, default: "" },
});

defineEmits([
  "refresh",
  "select-task",
  "update:history-query",
  "retry",
  "delete",
  "download-output",
]);

const historyListRef = ref(null);

watch(
  () => [props.filteredHistory.length, props.filteredHistory.at(-1)?.id, props.historyQuery],
  async () => {
    await nextTick();
    if (historyListRef.value) {
      historyListRef.value.scrollTop = historyListRef.value.scrollHeight;
    }
  },
  { immediate: true },
);
</script>
