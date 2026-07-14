<template>
  <aside class="result-column">
    <div v-if="selectedTask" class="selected-task">
      <div class="selected-task-head">
        <span class="selected-task-meta">
          状态：{{ statusLabel(selectedTask.status) }}，api 源：{{ selectedTask.providerName || "未配置" }}
        </span>
        <div class="selected-task-actions">
          <n-button
            v-if="previewOutput"
            secondary
            size="small"
            @click="$emit('copy-output', previewOutput)"
          >
            复制
          </n-button>
          <n-button secondary size="small" @click="$emit('show-detail')">详情</n-button>
          <n-button secondary size="small" @click="$emit('reuse', selectedTask)">重用</n-button>
        </div>
      </div>
      <strong>{{ selectedTask.prompt || "空提示词" }}</strong>
      <span>{{ selectedTask.providerName }} · {{ selectedTask.model }}</span>
      <p v-if="selectedTask.error">{{ selectedTask.error }}</p>
    </div>

    <div v-if="currentOutputs.length" class="output-grid">
      <article v-for="output in currentOutputs" :key="output.path" class="output-card">
        <div class="output-image-frame">
          <img :src="fileUrl(output.path)" :alt="output.fileName" />
        </div>
        <footer class="output-status-bar">
          <span>{{ output.size || selectedTask?.params?.size }}</span>
          <span>{{ output.outputFormat }}</span>
          <span>{{ outputDate }}</span>
        </footer>
      </article>
    </div>
    <div v-else-if="isWaitingForOutput" class="generation-timer-panel">
      <span>任务正在进行中</span>
    </div>
    <div v-else class="empty-panel">选择已完成任务后预览结果</div>
  </aside>
</template>

<script setup>
import { computed } from "vue";
import { fileUrl, statusLabel } from "../lib/formatters";

const props = defineProps({
  selectedTask: { type: Object, default: null },
  currentOutputs: { type: Array, default: () => [] },
});

defineEmits(["show-detail", "reuse", "copy-output"]);

const isWaitingForOutput = computed(() => {
  const task = props.selectedTask;
  return Boolean(
    task
      && ["queued", "running", "cancelling"].includes(task.status)
      && !(task.outputs?.length),
  );
});

const outputDate = computed(() => {
  const value = props.selectedTask?.completedAt || props.selectedTask?.updatedAt || props.selectedTask?.createdAt;
  if (!value) return "未完成";
  return new Date(value).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
});

const previewOutput = computed(() => props.currentOutputs[0] || null);
</script>
