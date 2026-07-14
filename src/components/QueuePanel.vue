<template>
  <aside class="queue-column">
    <div class="history-toolbar">
      <n-input
        :value="historyQuery"
        size="small"
        clearable
        placeholder="搜索提示词或任务 ID"
        @update:value="$emit('update:history-query', $event)"
      >
        <template #prefix><Search :size="15" /></template>
      </n-input>
    </div>

    <div ref="historyListRef" class="task-stack history-stack">
      <TaskCard
        v-for="task in filteredHistory"
        :key="task.id"
        :task="task"
        :selected="selectedTaskId === task.id"
        @select="$emit('select-task', task.id)"
        @reuse="$emit('reuse', $event)"
        @refresh="$emit('refresh-task', $event)"
        @retry="$emit('retry', $event)"
        @delete="$emit('delete', $event)"
        @copy-output="$emit('copy-output', $event)"
        @download-output="$emit('download-output', $event)"
        @reveal-output="$emit('reveal-output', $event)"
      />
      <div v-if="!filteredHistory.length" class="empty-panel compact">没有生成历史</div>
    </div>
  </aside>
</template>

<script setup>
import { nextTick, ref, watch } from "vue";
import { Search } from "@lucide/vue";
import TaskCard from "./TaskCard.vue";

const props = defineProps({
  filteredHistory: { type: Array, default: () => [] },
  selectedTaskId: { type: String, default: "" },
  historyQuery: { type: String, default: "" },
});

defineEmits([
  "select-task",
  "update:history-query",
  "reuse",
  "refresh-task",
  "retry",
  "delete",
  "copy-output",
  "download-output",
  "reveal-output",
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
