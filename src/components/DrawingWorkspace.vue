<template>
  <section class="workspace" :style="workspaceStyle">
    <QueuePanel
      :filtered-history="filteredHistory"
      :selected-task-id="selectedTaskId"
      :history-query="historyQuery"
      :history-scope="historyScope"
      :scroll-request="historyScrollRequest"
      @select-task="$emit('select-task', $event)"
      @update:history-query="$emit('update:history-query', $event)"
      @update:history-scope="$emit('update:history-scope', $event)"
      @reuse="$emit('reuse', $event)"
      @refresh-task="$emit('refresh-task', $event)"
      @retry="$emit('retry', $event)"
      @delete="$emit('delete', $event)"
      @copy-output="$emit('copy-output', $event)"
      @download-output="$emit('download-output', $event)"
      @reveal-output="$emit('reveal-output', $event)"
    />

    <div
      class="panel-resizer"
      role="separator"
      aria-label="调整队列和结果预览宽度"
      @pointerdown="$emit('start-panel-resize', 'queue', $event)"
    ></div>

    <ResultPanel
      :selected-task="selectedTask"
      :current-outputs="currentOutputs"
      @show-detail="$emit('show-detail')"
      @reuse="$emit('reuse', $event)"
      @copy-output="$emit('copy-output', $event)"
      @model-template="$emit('model-template', $event)"
    />

    <div
      class="panel-resizer"
      role="separator"
      aria-label="调整结果预览和工作台宽度"
      @pointerdown="$emit('start-panel-resize', 'composer', $event)"
    ></div>

    <ComposerPanel
      :form="form"
      :image-provider-options="imageProviderOptions"
      :chat-provider-options="chatProviderOptions"
      :references="references"
      :submitting="submitting"
      :reference-drag-active="referenceDragActive"
      @submit="$emit('submit')"
      @show-template="$emit('show-template')"
      @clear-prompt="$emit('clear-prompt')"
      @prompt-focus="$emit('prompt-focus', $event)"
      @prompt-cursor="$emit('prompt-cursor', $event)"
      @prompt-paste="$emit('prompt-paste', $event)"
      @paste-reference="$emit('paste-reference')"
      @add-reference="$emit('add-reference')"
      @remove-reference="$emit('remove-reference', $event)"
      @reference-drag-over="$emit('reference-drag-over')"
      @reference-drag-leave="$emit('reference-drag-leave')"
      @drop-reference="$emit('drop-reference', $event)"
    />
  </section>
</template>

<script setup>
import ComposerPanel from "./ComposerPanel.vue";
import QueuePanel from "./QueuePanel.vue";
import ResultPanel from "./ResultPanel.vue";

defineProps({
  filteredHistory: { type: Array, default: () => [] },
  selectedTaskId: { type: String, default: "" },
  historyQuery: { type: String, default: "" },
  historyScope: { type: String, default: "today" },
  historyScrollRequest: { type: Number, default: 0 },
  selectedTask: { type: Object, default: null },
  currentOutputs: { type: Array, default: () => [] },
  form: { type: Object, required: true },
  imageProviderOptions: { type: Array, default: () => [] },
  chatProviderOptions: { type: Array, default: () => [] },
  references: { type: Array, default: () => [] },
  submitting: { type: Boolean, default: false },
  referenceDragActive: { type: Boolean, default: false },
  workspaceStyle: { type: Object, default: () => ({}) },
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
  "start-panel-resize",
  "show-detail",
  "model-template",
  "submit",
  "show-template",
  "clear-prompt",
  "prompt-focus",
  "prompt-cursor",
  "prompt-paste",
  "paste-reference",
  "add-reference",
  "remove-reference",
  "reference-drag-over",
  "reference-drag-leave",
  "drop-reference",
]);
</script>
