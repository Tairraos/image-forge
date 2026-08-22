<template>
  <section class="agent-workspace">
    <aside class="function-bar">
      <div class="function-bar-head">
        <div class="function-bar-titlebar" data-tauri-drag-region="deep"></div>
        <div class="function-bar-brand">
          <img :src="logoUrl" alt="Image Forge" />
        </div>
        <nav class="function-bar-nav" aria-label="功能栏">
          <button type="button" class="function-bar-item" @click="panel = 'chat'; $emit('create')">
            <AppIcon :raw="newChatIcon" :size="16" />
            <span>新对话</span>
          </button>
          <button
            type="button"
            class="function-bar-item"
            :class="{ active: panel === 'library' }"
            @click="panel = panel === 'library' ? 'chat' : 'library'"
          >
            <AppIcon :raw="libraryIcon" :size="16" />
            <span>图片库</span>
          </button>
          <button type="button" class="function-bar-item" @click="$emit('open-settings')">
            <AppIcon :raw="settingsIcon" :size="16" />
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
            <div
              class="agent-session-item"
              role="button"
              tabindex="0"
              :title="session.title || '新对话'"
              @click="selectSession(session.id)"
              @keydown.enter="selectSession(session.id)"
            >
              <input
                v-if="renaming?.id === session.id && renaming?.where === 'bar'"
                :ref="(el) => setRenameInput(el)"
                v-model="titleDraft"
                class="agent-session-rename"
                aria-label="对话标题"
                @click.stop
                @keydown.enter.prevent="commitRename"
                @keydown.esc.prevent="cancelRename"
                @blur="commitRename"
              />
              <span
                v-else
                class="agent-session-title"
                @dblclick.stop="startRename(session, 'bar')"
              >{{ session.title || "新对话" }}</span>
            </div>
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
        <template v-if="panel === 'chat'">
          <input
            v-if="renaming?.id === currentSession?.id && renaming?.where === 'top'"
            :ref="(el) => setRenameInput(el)"
            v-model="titleDraft"
            class="info-area-title-rename"
            aria-label="对话标题"
            data-tauri-drag-region="none"
            @keydown.enter.prevent="commitRename"
            @keydown.esc.prevent="cancelRename"
            @blur="commitRename"
          />
          <strong
            v-else
            class="info-area-title"
            :class="{ 'is-empty': !currentSession }"
            data-tauri-drag-region="none"
            @click="currentSession && startRename(currentSession, 'top')"
          >{{ currentSession ? currentSession.title || "新对话" : "开始新对话" }}</strong>
        </template>
        <strong v-else class="info-area-title">图片库</strong>
      </header>
      <AgentLibraryPanel
        v-if="panel === 'library'"
        :version="agentLibraryVersion"
        @preview-images="$emit('preview-images', $event)"
        @delete-task="$emit('delete-task', $event)"
        @download-output="$emit('download-output', $event)"
        @reveal-output="$emit('reveal-output', $event)"
      />
      <template v-else>
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
          :ratio="ratio"
          :resolution="resolution"
          @send="$emit('send', $event)"
          @stop="$emit('stop')"
          @add-reference="$emit('add-reference')"
          @paste-reference="$emit('paste-reference', $event)"
          @drop-reference="$emit('drop-reference', $event)"
          @remove-attachment="$emit('remove-attachment', $event)"
          @select-template="$emit('select-template')"
          @update:ratio="$emit('update:ratio', $event)"
          @update:resolution="$emit('update:resolution', $event)"
        />
      </template>
    </div>
  </section>
</template>

<script setup>
import { nextTick, ref } from "vue";
import { Trash2 } from "@lucide/vue";
import libraryIcon from "../assets/图片库.svg?raw";
import newChatIcon from "../assets/新对话.svg?raw";
import settingsIcon from "../assets/设置.svg?raw";
import logoUrl from "../assets/title.png";
import AppIcon from "./snippets/AppIcon.vue";
import AgentLibraryPanel from "./AgentLibraryPanel.vue";
import AgentComposer from "./AgentComposer.vue";
import AgentMessageList from "./AgentMessageList.vue";

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
  agentLibraryVersion: { type: Number, default: 0 },
  ratio: { type: String, default: "1:1" },
  resolution: { type: String, default: "standard" },
});
const emit = defineEmits([
  "create", "select", "send", "stop", "add-reference", "remove-attachment",
  "open-task-group", "preview-images", "delete-task", "download-output", "reveal-output",
  "cancel-task-group", "retry-task-group", "retry", "paste-reference", "drop-reference", "update-answer", "answer-questions",
  "delete-session",
  "open-settings",
  "rename-session",
  "select-template",
  "update:ratio",
  "update:resolution",
]);

const panel = ref("chat");
const renaming = ref(null);
const titleDraft = ref("");
let renameInputEl = null;

function selectSession(id) {
  panel.value = "chat";
  emit("select", id);
}

function setRenameInput(el) {
  renameInputEl = el;
}

function startRename(session, where) {
  renaming.value = { id: session.id, where };
  titleDraft.value = session.title || "新对话";
  nextTick(() => renameInputEl?.select?.());
}

function commitRename() {
  const target = renaming.value;
  if (!target) return;
  renaming.value = null;
  const title = titleDraft.value.trim();
  if (title) {
    emit("rename-session", { sessionId: target.id, title });
  }
}

function cancelRename() {
  renaming.value = null;
}
</script>
