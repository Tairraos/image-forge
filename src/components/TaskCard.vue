<template>
  <article class="task-card" :class="[task.status, { selected }]" @click="$emit('select', task)">
    <header>
      <strong>{{ task.prompt || "空提示词" }}</strong>
      <span>{{ shortId(task.id) }}</span>
    </header>
    <p>{{ task.providerName || "API" }} · {{ task.model || "" }}</p>

    <div v-if="task.outputs?.length" class="task-output-list">
      <figure v-for="output in task.outputs" :key="output.path" class="task-output-thumb">
        <img :src="fileUrl(output.path)" :alt="output.fileName" />
      </figure>
    </div>

    <footer>
      <span>{{ createdTime }}</span>
      <div class="task-actions">
        <button
          v-for="output in downloadableOutputs"
          :key="output.path"
          type="button"
          @click.stop="$emit('download-output', output)"
        >
          下载
        </button>
        <button v-if="canRetry" type="button" @click.stop="$emit('retry', task)">重试</button>
        <button v-if="canDelete" type="button" @click.stop="$emit('delete', task)">删除</button>
      </div>
    </footer>
  </article>
</template>

<script setup>
import { computed } from "vue";
import { fileUrl, shortId } from "../lib/formatters";

const props = defineProps({
  task: { type: Object, required: true },
  selected: { type: Boolean, default: false },
});

defineEmits(["select", "retry", "delete", "download-output"]);

const isCompleted = computed(() => props.task.status === "completed");
const isFailed = computed(() => props.task.status === "failed" || props.task.status === "cancelled");
const canRetry = computed(() => isFailed.value);
const canDelete = computed(() => isCompleted.value || isFailed.value);
const downloadableOutputs = computed(() => (isCompleted.value ? props.task.outputs || [] : []));
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
