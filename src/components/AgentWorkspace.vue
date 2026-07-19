<template>
  <section class="agent-workspace">
    <aside class="agent-sidebar">
      <div class="agent-sidebar-head">
        <strong>对话</strong>
        <n-button size="tiny" type="primary" @click="$emit('create')">新建</n-button>
      </div>
      <div class="agent-session-list">
        <button
          v-for="session in sessions"
          :key="session.id"
          type="button"
          class="agent-session-item"
          :class="{ active: session.id === currentSession?.id }"
          @click="$emit('select', session.id)"
        >
          <span>{{ session.title || "新对话" }}</span>
          <small>{{ formatTime(session.updatedAt) }}</small>
        </button>
      </div>
    </aside>

    <div class="agent-chat">
      <header class="agent-chat-head">
        <div>
          <strong>{{ currentSession?.title || "Agent" }}</strong>
          <span>普通聊天、Skill 与绘图任务</span>
        </div>
        <n-select
          :value="providerId"
          :options="providerOptions"
          placeholder="选择对话模型"
          class="agent-provider-select"
          @update:value="$emit('update:provider-id', $event)"
        />
      </header>

      <div class="agent-message-list">
        <div v-if="!messages.length" class="agent-empty">
          <strong>开始一段对话</strong>
          <span>可以直接聊天，也可以要求 Agent 使用已安装 Skill 或创建绘图任务。</span>
        </div>
        <article
          v-for="message in messages"
          :key="message.id"
          class="agent-message"
          :data-role="message.role"
        >
          <div class="agent-message-role">{{ message.role === "user" ? "你" : "Agent" }}</div>
          <div class="agent-message-body">{{ message.content }}</div>
        </article>
        <article v-if="busy || streamText" class="agent-message" data-role="assistant">
          <div class="agent-message-role">Agent</div>
          <div class="agent-message-body">{{ streamText || "正在思考..." }}</div>
        </article>
      </div>

      <div class="agent-composer">
        <div v-if="attachments.length" class="agent-attachments">
          <div v-for="attachment in attachments" :key="attachment.id" class="agent-attachment">
            <img :src="attachment.dataUrl" :alt="attachment.fileName" />
            <button type="button" aria-label="移除参考图" @click="$emit('remove-attachment', attachment.id)">×</button>
          </div>
        </div>
        <n-input
          v-model:value="draft"
          type="textarea"
          :autosize="{ minRows: 3, maxRows: 8 }"
          placeholder="输入消息"
          :disabled="busy"
          @keydown.meta.enter.prevent="send"
          @keydown.ctrl.enter.prevent="send"
        />
        <footer>
          <n-button size="small" :disabled="busy" @click="$emit('add-reference')">添加参考图</n-button>
          <n-button v-if="busy" size="small" type="error" secondary @click="$emit('stop')">停止</n-button>
          <n-button v-else size="small" type="primary" :disabled="!draft.trim() || !providerId" @click="send">发送</n-button>
        </footer>
      </div>
    </div>
  </section>
</template>

<script setup>
import { ref } from "vue";

const props = defineProps({
  sessions: { type: Array, default: () => [] },
  currentSession: { type: Object, default: null },
  providerOptions: { type: Array, default: () => [] },
  providerId: { type: String, default: "" },
  busy: Boolean,
  streamText: { type: String, default: "" },
  attachments: { type: Array, default: () => [] },
});
const emit = defineEmits([
  "create", "select", "send", "stop", "add-reference", "remove-attachment", "update:provider-id",
]);
const draft = ref("");

function send() {
  const content = draft.value.trim();
  if (!content || props.busy || !props.providerId) return;
  emit("send", content);
  draft.value = "";
}

function formatTime(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}
</script>
