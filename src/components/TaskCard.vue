<template>
  <article class="task-card" :class="{ selected }" @click="$emit('select', task)">
    <header>
      <strong>{{ task.prompt || "空提示词" }}</strong>
      <span>{{ shortId(task.id) }}</span>
    </header>
    <p>{{ task.providerName || "API" }} · {{ task.model || "" }}</p>
    <footer>
      <span class="task-status" :class="task.status">{{ statusLabel(task.status) }}</span>
      <div class="task-actions">
        <button
          v-if="task.status === 'queued'"
          type="button"
          title="移到队首"
          @click.stop="$emit('promote', task)"
        >
          <ArrowUp :size="14" />
        </button>
        <button
          v-if="task.status === 'queued' || task.status === 'running'"
          type="button"
          title="取消任务"
          @click.stop="$emit('cancel', task)"
        >
          <XCircle :size="14" />
        </button>
        <button
          v-if="task.status === 'failed' || task.status === 'cancelled'"
          type="button"
          title="重试任务"
          @click.stop="$emit('retry', task)"
        >
          <RotateCcw :size="14" />
        </button>
      </div>
    </footer>
  </article>
</template>

<script setup>
import { ArrowUp, RotateCcw, XCircle } from "@lucide/vue";
import { shortId, statusLabel } from "../lib/formatters";

defineProps({
  task: { type: Object, required: true },
  selected: { type: Boolean, default: false },
});

defineEmits(["select", "cancel", "retry", "promote"]);
</script>
