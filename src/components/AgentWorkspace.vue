<template>
  <section class="agent-workspace">
    <aside class="agent-sidebar">
      <div class="agent-sidebar-head">
        <strong>对话</strong>
        <n-button size="tiny" type="primary" @click="$emit('create')">新建</n-button>
      </div>
      <div class="agent-session-list">
        <div
          v-for="session in sessions"
          :key="session.id"
          class="agent-session-row"
          :class="{ active: session.id === currentSession?.id }"
        >
          <button
            type="button"
            class="agent-session-item"
            @click="$emit('select', session.id)"
          >
            <span class="agent-session-title">{{ session.title || "新对话" }}</span>
            <small>{{ formatTime(session.updatedAt || session.createdAt) }}</small>
          </button>
          <button
            type="button"
            class="agent-session-delete"
            title="删除对话"
            aria-label="删除对话"
            @click.stop="$emit('delete-session', session.id)"
          >
            <Trash2 :size="14" />
          </button>
        </div>
      </div>
    </aside>

    <div class="agent-chat">
      <header class="agent-chat-head">
        <div>
          <strong>{{ currentSession?.title || "Agent" }}</strong>
        </div>
        <div class="agent-model-selects">
          <n-select
            :value="imageProviderId"
            :options="imageProviderOptions"
            placeholder="选择生图模型"
            aria-label="生图模型"
            class="agent-provider-select"
            @update:value="$emit('update:image-provider-id', $event)"
          />
          <n-select
            :value="providerId"
            :options="providerOptions"
            placeholder="选择对话模型"
            aria-label="对话模型"
            class="agent-provider-select"
            @update:value="$emit('update:provider-id', $event)"
          />
        </div>
      </header>

      <AgentMessageList
        :messages="messages"
        :busy="busy"
        :stream-text="streamText"
        :tool-status-text="toolStatusText"
        :answers="answers"
        @open-task-group="$emit('open-task-group', $event)"
        @cancel-task-group="$emit('cancel-task-group', $event)"
        @retry-task-group="$emit('retry-task-group', $event)"
        @retry="$emit('retry', $event)"
        @update-answer="$emit('update-answer', $event)"
        @answer-questions="$emit('answer-questions', $event)"
      />
      <AgentComposer
        :provider-id="providerId"
        :image-provider-id="imageProviderId"
        :busy="busy"
        :attachments="attachments"
        @send="$emit('send', $event)"
        @stop="$emit('stop')"
        @add-reference="$emit('add-reference')"
        @paste-reference="$emit('paste-reference', $event)"
        @drop-reference="$emit('drop-reference', $event)"
        @remove-attachment="$emit('remove-attachment', $event)"
      />
    </div>
  </section>
</template>

<script setup>
import AgentComposer from "./AgentComposer.vue";
import AgentMessageList from "./AgentMessageList.vue";
import { Trash2 } from "@lucide/vue";

defineProps({
  sessions: { type: Array, default: () => [] },
  currentSession: { type: Object, default: null },
  messages: { type: Array, default: () => [] },
  providerOptions: { type: Array, default: () => [] },
  providerId: { type: String, default: "" },
  imageProviderOptions: { type: Array, default: () => [] },
  imageProviderId: { type: String, default: "" },
  busy: Boolean,
  streamText: { type: String, default: "" },
  attachments: { type: Array, default: () => [] },
  toolStatusText: { type: String, default: "" },
  answers: { type: Object, default: () => ({}) },
});
const emit = defineEmits([
  "create", "select", "send", "stop", "add-reference", "remove-attachment", "update:provider-id", "update:image-provider-id",
  "open-task-group", "cancel-task-group", "retry-task-group", "retry", "paste-reference", "drop-reference", "update-answer", "answer-questions",
  "delete-session",
]);

function formatTime(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}
</script>
