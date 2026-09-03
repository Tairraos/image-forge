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
            <n-input
              :value="answers[question.key] || ''"
              :placeholder="question.placeholder"
              @update:value="$emit('update-answer', { key: question.key, value: $event })"
            />
          </label>
        </div>
        <div class="agent-question-actions">
          <n-button size="small" type="primary" @click="$emit('answer-questions', message)"
            >提交回答</n-button
          >
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
          <n-button
            v-if="!isTerminalStatus(message.taskGroup.status)"
            size="tiny"
            secondary
            @click="$emit('cancel-task-group', message.taskGroup)"
          >
            取消
          </n-button>
          <n-button
            v-if="canRetryStatus(message.taskGroup.status)"
            size="tiny"
            type="warning"
            secondary
            @click="$emit('retry-task-group', message.taskGroup)"
          >
            重试
          </n-button>
          <n-button
            v-if="message.taskGroup.status === 'completed' && message.taskGroup.taskIds?.length"
            size="tiny"
            secondary
            @click="$emit('redraw-task-group', message.taskGroup)"
          >
            再来一张
          </n-button>
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
      <n-button
        v-if="message.error"
        size="tiny"
        type="error"
        secondary
        @click="$emit('retry', message)"
      >
        {{ message.error }} · 重试
      </n-button>
    </article>
    <article
      v-if="busy || streamText || toolStatusText"
      class="agent-message"
      data-role="assistant"
    >
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
import MarkdownIt from 'markdown-it';
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import meIcon from '@iconify-icons/icon-park-solid/me';
import { fileUrl } from '../lib/formatters';

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

function roleIcon(role) {
  if (role === 'tool') return paintTool;
  return role === 'user' ? meIcon : robotLine;
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
