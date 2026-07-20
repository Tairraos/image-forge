<template>
  <div ref="listRef" class="agent-message-list">
    <div v-if="!messages.length" class="agent-empty">
      <strong>开始一段对话</strong>
      <span>直接聊天，或勾选“本轮进行绘画”把提示词送到绘画模型。</span>
    </div>
    <article
      v-for="message in messages"
      :key="message.id"
      class="agent-message"
      :data-role="message.role"
    >
      <div class="agent-message-role" tabindex="0">
        <Icon :icon="roleIcon(message.role)" />
        <span>{{ roleLabel(message.role) }}</span>
        <time v-if="message.createdAt">{{ formatMessageTime(message.createdAt) }}</time>
      </div>
      <div
        v-if="message.content && message.role === 'assistant'"
        class="agent-message-body agent-message-markdown"
        v-html="renderMarkdown(message.content)"
      ></div>
      <div v-else-if="message.content" class="agent-message-body">{{ message.content }}</div>
      <div v-if="message.toolCall" class="agent-tool-card" :data-status="message.toolCall.status">
        <strong>{{ message.toolCall.name }}</strong>
        <span>{{ toolStatus(message.toolCall) }}</span>
        <small v-if="message.toolCall.error">{{ message.toolCall.error }}</small>
      </div>
      <div v-if="message.questions?.length" class="agent-question-card">
        <div class="agent-question-fields">
          <label v-for="question in message.questions" :key="question.key">
            <span>{{ question.label }}</span>
            <n-input
              :value="answers[question.key] || ''"
              :placeholder="question.placeholder"
              @update:value="$emit('update-answer', { key: question.key, value: $event })"
            />
          </label>
        </div>
        <div class="agent-question-actions">
          <n-button size="small" type="primary" @click="$emit('answer-questions', message)">提交回答</n-button>
        </div>
      </div>
      <div v-if="message.taskGroup" class="agent-task-group-card">
        <button type="button" class="agent-task-group-open" @click="$emit('open-task-group', message.taskGroup)">
          <strong>绘图任务组 · {{ message.taskGroup.taskIds?.length || 0 }} 项</strong>
          <span>{{ message.taskGroup.titles?.join('、') || message.taskGroup.id }}</span>
          <small v-if="message.taskGroup.promptSummaries?.length">
            提示词：{{ message.taskGroup.promptSummaries.join('、') }}
          </small>
          <small>{{ taskGroupStatusLabel(message.taskGroup.status) }} · 点击查看绘画</small>
        </button>
        <div class="agent-task-group-actions">
          <n-button
            size="tiny"
            secondary
            :disabled="isTerminalStatus(message.taskGroup.status)"
            @click="$emit('cancel-task-group', message.taskGroup)"
          >
            取消
          </n-button>
          <n-button
            size="tiny"
            secondary
            :disabled="!canRetryStatus(message.taskGroup.status)"
            @click="$emit('retry-task-group', message.taskGroup)"
          >
            重试失败项
          </n-button>
        </div>
      </div>
      <n-button v-if="message.error" size="tiny" type="error" secondary @click="$emit('retry', message)">
        {{ message.error }} · 重试
      </n-button>
    </article>
    <article v-if="busy || streamText || toolStatusText" class="agent-message" data-role="assistant">
      <div class="agent-message-role" tabindex="0">
        <Icon :icon="robotLine" />
        <span>Agent</span>
      </div>
      <div
        class="agent-message-body agent-message-markdown"
        v-html="renderMarkdown(streamText || toolStatusText || '正在思考...')"
      ></div>
    </article>
  </div>
</template>

<script setup>
import MarkdownIt from "markdown-it";
import { nextTick, ref, watch } from "vue";
import { Icon } from "@iconify/vue";
import meIcon from "@iconify-icons/icon-park-solid/me";

const robotLine = {
  body: '<g fill="none"><path d="m12.594 23.258l-.012.002l-.071.035l-.02.004l-.014-.004l-.071-.036q-.016-.004-.024.006l-.004.01l-.017.428l.005.02l.01.013l.104.074l.015.004l.012-.004l.104-.074l.012-.016l.004-.017l-.017-.427q-.004-.016-.016-.018m.264-.113l-.014.002l-.184.093l-.01.01l-.003.011l.018.43l.005.012l.008.008l.201.092q.019.005.029-.008l.004-.014l-.034-.614q-.005-.019-.02-.022m-.715.002a.02.02 0 0 0-.027.006l-.006.014l-.034.614q.001.018.017.024l.015-.002l.201-.093l.01-.008l.003-.011l.018-.43l-.003-.012l-.01-.01z"/><path fill="currentColor" d="M18 10a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2zM8 14v-2a1 1 0 1 1 2 0v2a1 1 0 1 1-2 0m6 0v-2a1 1 0 1 1 2 0v2a1 1 0 1 1-2 0m0-10c0 .74-.403 1.383-1 1.73V6h3a4 4 0 0 1 4 4v.05a2.501 2.501 0 0 1 0 4.9V16a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4v-1.05a2.5 2.5 0 0 1 0-4.9V10a4 4 0 0 1 4-4h3v-.27A2 2 0 1 1 14 4"/></g>',
  width: 24,
  height: 24,
};
const paintTool = {
  body: '<path fill="currentColor" d="M18 1h-8a3 3 0 0 0-3 3H6a3 3 0 0 0-3 3v3a3 3 0 0 0 3 3h6a1 1 0 0 1 1 1v1a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2v-1a3 3 0 0 0-3-3H6a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h1a3 3 0 0 0 3 3h8a3 3 0 0 0 3-3V4a3 3 0 0 0-3-3m-3 16v4h-2v-4Zm4-11a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1Z"/>',
  width: 24,
  height: 24,
};

const props = defineProps({
  messages: { type: Array, default: () => [] },
  busy: Boolean,
  streamText: { type: String, default: "" },
  toolStatusText: { type: String, default: "" },
  answers: { type: Object, default: () => ({}) },
});
defineEmits(["open-task-group", "cancel-task-group", "retry-task-group", "retry", "update-answer", "answer-questions"]);

const listRef = ref(null);
const markdown = new MarkdownIt({ html: false, breaks: true, linkify: true });

watch(
  () => [props.messages.length, props.busy, props.streamText, props.toolStatusText],
  async () => {
    await nextTick();
    scrollToBottomIfNearBottom();
  },
  { immediate: true },
);

function scrollToBottomIfNearBottom() {
  const element = listRef.value;
  if (!element) return;
  const distanceToBottom = element.scrollHeight - element.scrollTop - element.clientHeight;
  if (distanceToBottom > 120) return;
  element.scrollTop = element.scrollHeight;
}

function roleLabel(role) {
  if (role === "user") return "你";
  if (role === "tool") return "工具";
  return "Agent";
}

function roleIcon(role) {
  if (role === "tool") return paintTool;
  return role === "user" ? meIcon : robotLine;
}

function formatMessageTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function renderMarkdown(content) {
  return markdown.render(content || "");
}

function toolStatus(call) {
  return {
    pending: "准备调用",
    running: "执行中",
    completed: "执行完成",
    failed: "执行失败",
  }[call.status] || call.status || "已记录";
}

function taskGroupStatusLabel(status) {
  return {
    queued: "等待中",
    running: "进行中",
    cancelling: "取消中",
    completed: "已完成",
    failed: "已失败",
    cancelled: "已取消",
    missing: "已丢失",
  }[status] || status || "未知状态";
}

function isTerminalStatus(status) {
  return ["completed", "failed", "cancelled", "missing"].includes(status);
}

function canRetryStatus(status) {
  return ["failed", "cancelled"].includes(status);
}
</script>
