<template>
  <aside class="result-column">
    <div class="section-head">
      <div>
        <h2>结果预览</h2>
        <p>{{ selectedTask ? shortId(selectedTask.id) : "未选择任务" }}</p>
      </div>
      <n-button
        circle
        quaternary
        size="small"
        :disabled="!selectedTask"
        @click="$emit('show-detail')"
      >
        <template #icon><Eye :size="16" /></template>
      </n-button>
    </div>

    <div v-if="selectedTask" class="selected-task">
      <n-tag :type="statusType(selectedTask.status)" size="small">
        {{ statusLabel(selectedTask.status) }}
      </n-tag>
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
    <div v-else class="empty-panel">选择已完成任务后预览结果</div>
  </aside>
</template>

<script setup>
import { computed } from "vue";
import { Eye } from "@lucide/vue";
import { fileUrl, shortId, statusLabel, statusType } from "../lib/formatters";

const props = defineProps({
  selectedTask: { type: Object, default: null },
  currentOutputs: { type: Array, default: () => [] },
});

defineEmits(["show-detail"]);

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
</script>
