<template>
  <aside class="queue-column">
    <div class="history-toolbar">
      <n-input
        :value="historyQuery"
        size="small"
        clearable
        placeholder="搜索提示词或 ID"
        @update:value="$emit('update:history-query', $event)"
      >
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <n-radio-group
        :value="historyScope"
        size="small"
        class="history-scope-control"
        aria-label="任务历史时间范围"
        @update:value="$emit('update:history-scope', $event)"
      >
        <n-radio-button value="today">今天</n-radio-button>
        <n-radio-button value="all">所有</n-radio-button>
      </n-radio-group>
    </div>

    <div ref="historyListRef" class="task-stack history-stack">
      <TaskCard
        v-for="task in filteredHistory"
        :key="task.id"
        :ref="(instance) => setTaskCardRef(task.id, instance)"
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
  historyScope: { type: String, default: "today" },
  scrollRequest: { type: Number, default: 0 },
});

defineEmits([
  "select-task",
  "update:history-query",
  "update:history-scope",
  "reuse",
  "refresh-task",
  "retry",
  "delete",
  "copy-output",
  "download-output",
  "reveal-output",
]);

const historyListRef = ref(null);
const taskCardRefs = new Map();

function setTaskCardRef(taskId, instance) {
  if (instance) {
    taskCardRefs.set(taskId, instance);
  } else {
    taskCardRefs.delete(taskId);
  }
}

function scrollTaskIntoView(taskId) {
  if (!taskId) return;
  const instance = taskCardRefs.get(taskId);
  const element = instance?.$el || instance;
  if (element?.scrollIntoView) {
    element.scrollIntoView({ block: "center", behavior: "smooth" });
  }
}

watch(
  () => props.selectedTaskId,
  async () => {
    await nextTick();
    scrollTaskIntoView(props.selectedTaskId);
  },
  { immediate: true },
);

watch(
  () => props.scrollRequest,
  async () => {
    await nextTick();
    if (historyListRef.value) {
      historyListRef.value.scrollTop = historyListRef.value.scrollHeight;
    }
  },
);
</script>
