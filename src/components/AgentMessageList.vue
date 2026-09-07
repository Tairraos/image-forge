<template>
  <div ref="listRef" class="agent-message-list">
    <div v-if="!messages.length" class="agent-empty">
      <span class="agent-empty-mark"><Aperture :size="32" :stroke-width="1.4" /></span>
      <h1>从一个想法开始</h1>
      <p>描述你的画面，与 Agent 一起把灵感变成作品。</p>
    </div>
    <article
      v-for="message in messages"
      :key="message.id"
      class="agent-message"
      :data-role="message.role"
    >
      <div class="agent-message-role" tabindex="0">
        <UserRound v-if="message.role === 'user'" :size="15" /><Wrench
          v-else-if="message.role === 'tool'"
          :size="15"
        /><Aperture v-else :size="17" />
        <span>{{ roleLabel(message.role) }}</span>
        <time v-if="message.createdAt">{{ formatMessageTime(message.createdAt) }}</time>
      </div>
      <div
        v-if="message.content && message.role === 'assistant'"
        class="agent-message-body agent-message-markdown"
        v-html="renderMarkdown(message.content)"
      ></div>
      <div v-else-if="message.content && !message.toolCall" class="agent-message-body">
        {{ message.content }}
      </div>
      <div v-if="message.toolCall" class="agent-tool-card" :data-status="message.toolCall.status">
        <strong>{{ message.toolCall.name }}</strong>
        <span>{{ toolStatus(message.toolCall) }}</span>
        <template v-if="message.toolCall.error">
          <small class="agent-tool-error">{{ toolErrorPreview(message.toolCall.error) }}</small>
          <button
            type="button"
            class="agent-tool-detail-toggle"
            @click="toggleToolDetail(message.id)"
          >
            {{ toolDetailExpanded(message.id) ? '收起详情' : '查看失败详情' }}
          </button>
          <div
            v-if="toolDetailExpanded(message.id)"
            class="agent-tool-error-detail"
            data-testid="tool-error-detail"
          >
            <pre>{{ message.toolCall.error }}</pre>
            <pre v-if="message.toolCall.result">{{
              formatToolResult(message.toolCall.result)
            }}</pre>
          </div>
        </template>
      </div>
      <div v-if="message.questions?.length" class="agent-question-card">
        <div class="agent-question-fields">
          <label v-for="question in message.questions" :key="question.key">
            <span>{{ question.label }}</span>
            <textarea
              class="form-control"
              rows="2"
              :value="answers[question.key] || ''"
              :placeholder="question.placeholder"
              @input="$emit('update-answer', { key: question.key, value: $event.target.value })"
            ></textarea>
          </label>
        </div>
        <div class="agent-question-actions">
          <button
            type="button"
            class="button button-small button-primary"
            @click="$emit('answer-questions', message)"
          >
            提交回答
          </button>
        </div>
      </div>
      <div v-if="message.taskGroup" class="agent-task-group-card">
        <div class="agent-task-group-bar">
          <span class="agent-task-group-status">
            <span class="agent-task-group-dot" :class="'dot--' + message.taskGroup.status"></span>
            <template v-if="message.taskGroup.status === 'completed'">
              已完成，共 {{ message.taskGroup.images?.length || 0 }} 张
            </template>
            <template v-else-if="message.taskGroup.status === 'failed'"> 生成失败 </template>
            <template v-else-if="message.taskGroup.status === 'cancelled'"> 已取消 </template>
            <template v-else> 服务器已经连接，生图中 </template>
          </span>
          <span v-if="!isTerminalStatus(message.taskGroup.status)" class="agent-task-group-timer">{{
            elapsed(message)
          }}</span>
          <div class="agent-task-group-spacer"></div>
          <button
            v-if="!isTerminalStatus(message.taskGroup.status)"
            type="button"
            class="button button-small"
            @click="$emit('cancel-task-group', message.taskGroup)"
          >
            取消
          </button>
          <button
            v-if="canRetryStatus(message.taskGroup.status)"
            type="button"
            class="button button-small"
            @click="$emit('retry-task-group', message.taskGroup)"
          >
            重试
          </button>
          <button
            v-if="message.taskGroup.status === 'completed' && message.taskGroup.taskIds?.length"
            type="button"
            class="button button-small"
            @click="$emit('redraw-task-group', message.taskGroup)"
          >
            再来一张
          </button>
        </div>
        <div v-if="message.taskGroup.images?.length" class="agent-generated-thumbs">
          <button
            v-for="(image, index) in message.taskGroup.images"
            :key="image.path"
            type="button"
            class="agent-generated-thumb"
            :aria-label="`查看生成图片 ${index + 1}`"
            @click="$emit('preview-images', { items: message.taskGroup.images, index })"
          >
            <img
              loading="lazy"
              :src="fileUrl(image.path)"
              :alt="image.title || image.fileName || '生成图片'"
            />
          </button>
        </div>
      </div>
      <button
        v-if="message.error"
        type="button"
        class="button button-small button-danger"
        @click="$emit('retry', message)"
      >
        {{ message.error }} · 重试
      </button>
    </article>
    <article
      v-if="busy || streamText || toolStatusText"
      class="agent-message"
      data-role="assistant"
    >
      <div class="agent-message-role" tabindex="0">
        <Aperture :size="17" />
        <span>Agent</span>
      </div>
      <div
        v-if="streamText"
        class="agent-message-body agent-message-markdown"
        v-html="renderMarkdown(streamText)"
      ></div>
      <div v-else class="agent-thinking" role="status">
        <span class="thinking-dot"></span>{{ toolStatusText || '正在思考…' }}
      </div>
    </article>
  </div>
</template>

<script setup>
import MarkdownIt from 'markdown-it';
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { Aperture, UserRound, Wrench } from '@lucide/vue';
import { fileUrl } from '../lib/formatters';

const props = defineProps({
  messages: { type: Array, default: () => [] },
  busy: Boolean,
  streamText: { type: String, default: '' },
  toolStatusText: { type: String, default: '' },
  answers: { type: Object, default: () => ({}) },
});
defineEmits([
  'open-task-group',
  'preview-images',
  'cancel-task-group',
  'retry-task-group',
  'redraw-task-group',
  'retry',
  'update-answer',
  'answer-questions',
]);

const listRef = ref(null);
const markdown = new MarkdownIt({ html: false, breaks: true, linkify: true });
const now = ref(Date.now());
const expandedToolCalls = ref(new Set());
let timer = 0;

const TOOL_ERROR_PREVIEW_LIMIT = 120;

function toggleToolDetail(id) {
  const next = new Set(expandedToolCalls.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  expandedToolCalls.value = next;
}

function toolDetailExpanded(id) {
  return expandedToolCalls.value.has(id);
}

function toolErrorPreview(error) {
  const text = String(error || '').trim();
  if (text.length <= TOOL_ERROR_PREVIEW_LIMIT) return text;
  return `${text.slice(0, TOOL_ERROR_PREVIEW_LIMIT)}…`;
}

function formatToolResult(result) {
  try {
    return JSON.stringify(result, null, 2);
  } catch {
    return String(result);
  }
}

onMounted(() => {
  timer = window.setInterval(() => {
    now.value = Date.now();
  }, 1000);
});

onBeforeUnmount(() => {
  window.clearInterval(timer);
});

watch(
  () => [props.messages.length, props.busy, props.streamText, props.toolStatusText],
  async () => {
    await nextTick();
    scrollToBottomIfNearBottom();
  },
  { immediate: true }
);

function scrollToBottomIfNearBottom() {
  const element = listRef.value;
  if (!element) return;
  const distanceToBottom = element.scrollHeight - element.scrollTop - element.clientHeight;
  if (distanceToBottom > 120) return;
  element.scrollTop = element.scrollHeight;
}

function roleLabel(role) {
  if (role === 'user') return '你';
  if (role === 'tool') return '工具';
  return 'Agent';
}

function formatMessageTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function elapsed(message) {
  const start = new Date(message.createdAt).getTime();
  if (Number.isNaN(start)) return '';
  const diff = Math.max(0, now.value - start);
  const sec = Math.floor(diff / 1000) % 60;
  const min = Math.floor(diff / 60000) % 60;
  const hour = Math.floor(diff / 3600000);
  const pad = (n) => String(n).padStart(2, '0');
  return hour > 0 ? `${pad(hour)}:${pad(min)}:${pad(sec)}` : `${pad(min)}:${pad(sec)}`;
}

function renderMarkdown(content) {
  return markdown.render(content || '');
}

function toolStatus(call) {
  return (
    {
      pending: '准备调用',
      running: '执行中',
      completed: '执行完成',
      failed: '执行失败',
    }[call.status] ||
    call.status ||
    '已记录'
  );
}

function isTerminalStatus(status) {
  return ['completed', 'failed', 'cancelled', 'missing'].includes(status);
}

function canRetryStatus(status) {
  return ['failed', 'cancelled'].includes(status);
}
</script>
