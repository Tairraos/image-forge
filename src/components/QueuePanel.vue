<template>
  <aside class="queue-column">
    <div class="section-head">
      <div>
        <h2>生图队列</h2>
        <p>{{ queue.workerActive ? "后台执行中" : "等待任务" }}</p>
      </div>
      <n-button circle quaternary size="small" @click="$emit('refresh')">
        <template #icon><RotateCcw :size="16" /></template>
      </n-button>
    </div>

    <n-tabs type="segment" animated>
      <n-tab-pane name="queue" tab="队列">
        <div class="task-stack">
          <TaskCard
            v-for="task in runningAndWaiting"
            :key="task.id"
            :task="task"
            :selected="selectedTaskId === task.id"
            @select="$emit('select-task', task.id)"
            @cancel="$emit('cancel', $event)"
            @retry="$emit('retry', $event)"
            @promote="$emit('promote', $event)"
            @download-output="$emit('download-output', $event)"
          />
          <div v-if="!runningAndWaiting.length" class="empty-panel compact">队列为空</div>
        </div>
      </n-tab-pane>
      <n-tab-pane name="history" tab="历史">
        <n-input
          :value="historyQuery"
          size="small"
          clearable
          placeholder="搜索提示词或任务 ID"
          @update:value="$emit('update:history-query', $event)"
        >
          <template #prefix><Search :size="15" /></template>
        </n-input>
        <div class="task-stack history-stack">
          <TaskCard
            v-for="task in filteredHistory"
            :key="task.id"
            :task="task"
            :selected="selectedTaskId === task.id"
            @select="$emit('select-task', task.id)"
            @cancel="$emit('cancel', $event)"
            @retry="$emit('retry', $event)"
            @promote="$emit('promote', $event)"
            @download-output="$emit('download-output', $event)"
          />
          <div v-if="!filteredHistory.length" class="empty-panel compact">没有匹配历史</div>
        </div>
      </n-tab-pane>
    </n-tabs>
  </aside>
</template>

<script setup>
import { RotateCcw, Search } from "@lucide/vue";
import TaskCard from "./TaskCard.vue";

defineProps({
  queue: { type: Object, required: true },
  runningAndWaiting: { type: Array, default: () => [] },
  filteredHistory: { type: Array, default: () => [] },
  selectedTaskId: { type: String, default: "" },
  historyQuery: { type: String, default: "" },
});

defineEmits([
  "refresh",
  "select-task",
  "update:history-query",
  "cancel",
  "retry",
  "promote",
  "download-output",
]);
</script>
