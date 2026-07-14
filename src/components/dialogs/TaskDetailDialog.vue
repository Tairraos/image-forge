<template>
  <n-modal v-model:show="show" preset="card" title="任务详情" class="wide-modal">
    <div v-if="task" class="detail-grid">
      <div>
        <h3>{{ task.providerName }}</h3>
        <p>{{ task.prompt }}</p>
        <n-descriptions :column="2" size="small" bordered>
          <n-descriptions-item label="状态">{{ statusLabel(task.status) }}</n-descriptions-item>
          <n-descriptions-item label="模型">{{ task.model }}</n-descriptions-item>
          <n-descriptions-item label="提示词模式">{{ task.params?.promptFidelity }}</n-descriptions-item>
          <n-descriptions-item label="分辨率">{{ task.params?.resolution }}</n-descriptions-item>
          <n-descriptions-item label="比例">{{ task.params?.ratio }}</n-descriptions-item>
          <n-descriptions-item label="方向">{{ task.params?.orientation }}</n-descriptions-item>
          <n-descriptions-item label="尺寸">{{ task.params?.size }}</n-descriptions-item>
          <n-descriptions-item label="质量">{{ task.params?.quality }}</n-descriptions-item>
          <n-descriptions-item label="数量">{{ task.params?.count }}</n-descriptions-item>
          <n-descriptions-item label="格式">{{ task.params?.outputFormat }}</n-descriptions-item>
        </n-descriptions>
      </div>
      <div class="detail-output-list">
        <img
          v-for="output in task.outputs"
          :key="output.path"
          :src="fileUrl(output.path)"
          :alt="output.fileName"
        />
      </div>
    </div>
    <template #footer>
      <div class="dialog-actions">
        <n-button type="primary" @click="reuseTask">重用</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { fileUrl, statusLabel } from "../../lib/formatters";

const show = defineModel("show", { type: Boolean, default: false });

const props = defineProps({
  task: { type: Object, default: null },
});

const emit = defineEmits(["reuse"]);

function reuseTask() {
  if (!props.task) return;
  emit("reuse", props.task);
  show.value = false;
}
</script>
