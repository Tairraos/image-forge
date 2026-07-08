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
        <img :src="fileUrl(output.path)" :alt="output.fileName" />
        <footer>
          <span>{{ output.size || selectedTask?.params?.size }} · {{ output.outputFormat }}</span>
          <div>
            <n-button size="tiny" secondary @click="$emit('reveal', output.path)">定位</n-button>
            <n-button size="tiny" secondary @click="$emit('save-output', output)">入图库</n-button>
          </div>
        </footer>
      </article>
    </div>
    <div v-else class="empty-panel">选择已完成任务后预览结果</div>
  </aside>
</template>

<script setup>
import { Eye } from "@lucide/vue";
import { fileUrl, shortId, statusLabel, statusType } from "../lib/formatters";

defineProps({
  selectedTask: { type: Object, default: null },
  currentOutputs: { type: Array, default: () => [] },
});

defineEmits(["show-detail", "reveal", "save-output"]);
</script>
