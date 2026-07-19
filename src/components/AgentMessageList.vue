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
      <div class="agent-message-role">{{ roleLabel(message.role) }}</div>
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
      <div class="agent-message-role">Agent</div>
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
