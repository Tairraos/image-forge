<template>
  <article class="task-card" :class="[task.status, { selected }]" @click="$emit('select', task)">
    <header>
      <strong>{{ task.prompt || "空提示词" }}</strong>
      <span>{{ createdTime }}</span>
    </header>
    <p>{{ task.providerName || "API" }} · {{ task.model || "" }}</p>

    <div v-if="task.outputs?.length" class="task-output-list">
      <figure v-for="output in task.outputs" :key="output.path" class="task-output-thumb">
        <img :src="fileUrl(output.path)" :alt="output.fileName" />
      </figure>
    </div>
    <div v-else-if="isWaitingForOutput" class="task-timer-preview" :class="{ timeout: isTimedOut }">
      <span>{{ label }}</span>
      <strong>{{ elapsedText }}</strong>
      <small v-if="isTimedOut">等待后端标记失败…</small>
    </div>

    <footer>
      <div class="task-actions">
        <button type="button" @click.stop="$emit('reuse', task)">重用</button>
        <button
          v-if="downloadableOutputs.length"
          type="button"
          @click.stop="$emit('copy-output', downloadableOutputs[0])"
        >
          复制
        </button>
        <button v-if="isActive" type="button" @click.stop="$emit('refresh', task)">刷新</button>
        <button
          v-for="output in downloadableOutputs"
          :key="output.path"
          type="button"
          @click.stop="$emit('download-output', output)"
        >
          下载
        </button>
        <button
          v-for="output in downloadableOutputs"
          :key="`${output.path}-reveal`"
          type="button"
          @click.stop="$emit('reveal-output', output)"
        >
          定位
        </button>
        <button v-if="canRetry" type="button" @click.stop="$emit('retry', task)">重试</button>
        <button type="button" @click.stop="$emit('delete', task)">删除</button>
      </div>
    </footer>
  </article>
</template>

<script setup>
import { computed, toRef } from "vue";
import { fileUrl } from "../lib/formatters";
import { useGenerationTimer } from "../lib/generationTimer";

const props = defineProps({
  task: { type: Object, required: true },
  selected: { type: Boolean, default: false },
});

defineEmits([
  "select",
  "reuse",
  "refresh",
  "retry",
  "delete",
  "copy-output",
  "download-output",
  "reveal-output",
]);

const isActive = computed(() => ["queued", "running", "cancelling"].includes(props.task.status));
const isCompleted = computed(() => props.task.status === "completed");
const isFailed = computed(() => props.task.status === "failed" || props.task.status === "cancelled");
const canRetry = computed(() => isFailed.value);
const downloadableOutputs = computed(() => (isCompleted.value ? props.task.outputs || [] : []));
const {
  elapsedText,
  isTimedOut,
  isWaitingForOutput,
  label,
} = useGenerationTimer(toRef(props, "task"));
const createdTime = computed(() => {
  const value = props.task.completedAt || props.task.createdAt || props.task.updatedAt;
  if (!value) return "";
  return new Date(value).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
});
</script>
