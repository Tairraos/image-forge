<template>
  <section class="agent-workspace">
    <aside class="function-bar">
      <div class="function-bar-head">
        <div class="function-bar-titlebar" data-tauri-drag-region="deep"></div>
        <div class="function-bar-brand">
          <img :src="logoUrl" alt="Image Forge" />
        </div>
        <nav class="function-bar-nav" aria-label="功能栏">
          <button type="button" class="function-bar-item" @click="$emit('create')">
            <MessageSquarePlus :size="16" />
            <span>新对话</span>
          </button>
          <button type="button" class="function-bar-item" @click="$emit('open-library')">
            <Images :size="16" />
            <span>图片库</span>
          </button>
          <button type="button" class="function-bar-item" @click="$emit('open-settings')">
            <Settings :size="16" />
            <span>设置</span>
          </button>
        </nav>
      </div>

      <div class="function-bar-sessions">
        <div v-if="sessions.length" class="agent-session-list">
          <div
            v-for="session in sessions"
            :key="session.id"
            class="agent-session-row"
            :class="{ active: session.id === currentSession?.id }"
          >
            <button
              type="button"
              class="agent-session-item"
              :title="session.title || '新对话'"
              @click="$emit('select', session.id)"
            >
              <span class="agent-session-title">{{ session.title || "新对话" }}</span>
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
        <p v-else class="function-bar-empty">还没有对话</p>
      </div>
    </aside>

    <div class="info-area">
      <header class="info-area-titlebar" data-tauri-drag-region="deep">
        <strong
          class="info-area-title"
          :class="{ 'is-empty': !currentSession }"
        >{{ currentSession ? currentSession.title || "新对话" : "开始新对话" }}</strong>
      </header>
      <AgentMessageList
        :messages="messages"
        :busy="busy"
        :stream-text="streamText"
        :tool-status-text="toolStatusText"
        :answers="answers"
        @open-task-group="$emit('open-task-group', $event)"
        @preview-images="$emit('preview-images', $event)"
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
import { Images, MessageSquarePlus, Settings, Trash2 } from "@lucide/vue";
import AgentComposer from "./AgentComposer.vue";
import AgentMessageList from "./AgentMessageList.vue";
import logoUrl from "../assets/title.png";

defineProps({
  sessions: { type: Array, default: () => [] },
  currentSession: { type: Object, default: null },
  messages: { type: Array, default: () => [] },
  providerId: { type: String, default: "" },
  imageProviderId: { type: String, default: "" },
  busy: Boolean,
  streamText: { type: String, default: "" },
  attachments: { type: Array, default: () => [] },
  toolStatusText: { type: String, default: "" },
  answers: { type: Object, default: () => ({}) },
});
const emit = defineEmits([
  "create", "select", "send", "stop", "add-reference", "remove-attachment",
  "open-task-group", "preview-images", "cancel-task-group", "retry-task-group", "retry", "paste-reference", "drop-reference", "update-answer", "answer-questions",
  "delete-session",
  "open-library", "open-settings",
]);
</script>
