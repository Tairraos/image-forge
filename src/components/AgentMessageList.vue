<template>
  <section class="agent-conversation" aria-label="对话内容">
    <div ref="listRef" class="agent-message-list" @scroll.passive="updateScrollPosition">
      <div ref="contentRef" class="agent-message-content">
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
          <div v-else-if="message.content && !toolCalls(message).length" class="agent-message-body">
            {{ message.content }}
          </div>
          <div v-if="message.attachments?.length" class="agent-message-attachments">
            <button
              v-for="(attachment, index) in message.attachments"
              :key="attachment.id || attachment.path"
              type="button"
              :aria-label="`查看参考图 ${index + 1}`"
              @click="previewAttachments(message, index)"
            >
              <img
                :src="fileUrl(attachment.path || attachment.dataUrl)"
                :alt="attachment.fileName || `参考图 ${index + 1}`"
                loading="lazy"
              />
            </button>
          </div>
          <div
            v-for="(call, index) in toolCalls(message)"
            :key="toolKey(message, call, index)"
            class="agent-tool-card"
            :data-status="call.status"
          >
            <strong :title="call.name">{{ toolName(call.name) }}</strong>
            <span>{{ toolStatus(call) }}</span>
            <template v-if="call.error">
              <small class="agent-tool-error">{{ toolErrorPreview(call.error) }}</small>
              <button
                type="button"
                class="agent-tool-detail-toggle"
                :aria-expanded="toolDetailExpanded(toolKey(message, call, index))"
                @click="toggleToolDetail(toolKey(message, call, index))"
              >
                {{
                  toolDetailExpanded(toolKey(message, call, index)) ? '收起详情' : '查看失败详情'
                }}
              </button>
              <div
                v-if="toolDetailExpanded(toolKey(message, call, index))"
                class="agent-tool-error-detail"
                data-testid="tool-error-detail"
              >
                <pre>{{ call.error }}</pre>
                <pre v-if="call.result">{{ formatToolResult(call.result) }}</pre>
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
                  :disabled="busy"
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
                :disabled="busy"
                @click="$emit('answer-questions', message)"
              >
                提交回答
              </button>
            </div>
          </div>
          <div
            v-if="message.taskGroup"
            class="agent-task-group-card"
            :data-status="message.taskGroup.status"
          >
            <div class="agent-task-group-bar">
              <span class="agent-task-group-status" role="status">
                <span
                  class="agent-task-group-dot"
                  :class="'dot--' + message.taskGroup.status"
                ></span>
                {{ groupStatusLabel(message.taskGroup) }}
              </span>
              <span
                v-if="!isTerminalStatus(message.taskGroup.status)"
                class="agent-task-group-timer"
                >{{ elapsed(message) }}</span
              >
              <div class="agent-task-group-spacer"></div>
              <button
                v-if="!isTerminalStatus(message.taskGroup.status)"
                type="button"
                class="button button-small"
                :disabled="message.taskGroup.status === 'cancelling'"
                @click="$emit('cancel-task-group', message.taskGroup)"
              >
                {{ message.taskGroup.status === 'cancelling' ? '取消中…' : '取消' }}
              </button>
              <button
                v-if="canRetryStatus(message.taskGroup.status)"
                type="button"
                class="button button-small"
                @click="$emit('retry-task-group', message.taskGroup)"
              >
                重试失败项
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
            <div v-if="message.taskGroup.progress?.total > 1" class="agent-task-progress">
              <progress
                :value="message.taskGroup.progress.completed"
                :max="message.taskGroup.progress.total"
                aria-label="图片生成进度"
              ></progress>
              <span
                >{{ message.taskGroup.progress.completed }} /
                {{ message.taskGroup.progress.total }} 已完成</span
              >
            </div>
            <div
              v-if="
                !isTerminalStatus(message.taskGroup.status) && !message.taskGroup.images?.length
              "
              class="agent-generation-placeholder"
            >
              <Image :size="24" :stroke-width="1.4" />
              <div>
                <strong>{{ groupStatusLabel(message.taskGroup) }}</strong
                ><span>{{
                  message.taskGroup.status === 'queued'
                    ? '轮到此任务后会自动开始'
                    : message.taskGroup.status === 'cancelling'
                      ? '正在停止当前请求'
                      : '图片完成后会显示在这里'
                }}</span>
              </div>
            </div>
            <details v-if="message.taskGroup.errors?.length" class="agent-task-error">
              <summary>
                <TriangleAlert :size="14" />{{
                  message.taskGroup.errors.length > 1
                    ? `${message.taskGroup.errors.length} 项任务未完成`
                    : '查看失败原因'
                }}<ChevronDown :size="13" />
              </summary>
              <p v-for="error in message.taskGroup.errors" :key="error">{{ error }}</p>
            </details>
            <p
              v-if="message.taskGroup.loadError && !isTerminalStatus(message.taskGroup.status)"
              class="agent-task-connection"
              role="status"
            >
              暂时无法更新进度，正在重试连接…
            </p>
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
          <div v-if="message.error" class="agent-message-error" role="alert">
            <TriangleAlert :size="16" />
            <p>{{ message.error }}</p>
            <button
              type="button"
              class="button button-small"
              :disabled="busy"
              @click="$emit('retry', message)"
            >
              <RotateCcw :size="13" />重新发送
            </button>
          </div>
          <span v-if="message.status === 'cancelled'" class="agent-turn-stopped"
            ><Square :size="10" />已停止生成</span
          >
          <div
            v-if="message.content && message.role !== 'tool' && !toolCalls(message).length"
            class="agent-message-actions"
          >
            <button
              type="button"
              class="icon-button"
              :aria-label="copiedMessage === message.id ? '已复制' : '复制消息'"
              :title="copiedMessage === message.id ? '已复制' : '复制消息'"
              @click="copyMessage(message)"
            >
              <Check v-if="copiedMessage === message.id" :size="14" /><Copy v-else :size="14" />
            </button>
            <span v-if="copyError === message.id" role="status">复制失败，请选中文字复制</span>
          </div>
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
    </div>
    <button
      v-if="showScrollLatest"
      type="button"
      class="agent-scroll-latest"
      aria-label="回到最新消息"
      @click="scrollToLatest"
    >
      <ArrowDown :size="15" /><span>回到最新</span>
    </button>
  </section>
</template>

<script setup>
import MarkdownIt from 'markdown-it';
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import {
  Aperture,
  ArrowDown,
  Check,
  ChevronDown,
  Copy,
  Image,
  RotateCcw,
  Square,
  TriangleAlert,
  UserRound,
  Wrench,
} from '@lucide/vue';
import { fileUrl } from '../lib/formatters';

const props = defineProps({
  messages: { type: Array, default: () => [] },
  sessionId: { type: String, default: '' },
  busy: Boolean,
  streamText: { type: String, default: '' },
  toolStatusText: { type: String, default: '' },
  answers: { type: Object, default: () => ({}) },
});
const emit = defineEmits([
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
const contentRef = ref(null);
const followingLatest = ref(true);
const showScrollLatest = ref(false);
const copiedMessage = ref('');
const copyError = ref('');
const markdown = new MarkdownIt({ html: false, breaks: true, linkify: true });
const now = ref(Date.now());
const expandedToolCalls = ref(new Set());
let timer = 0;
let copyTimer = 0;
let resizeObserver;

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
  scrollToLatest();
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => {
      if (followingLatest.value) scrollToLatest();
      else updateScrollPosition();
    });
    resizeObserver.observe(contentRef.value);
  }
});

onBeforeUnmount(() => {
  window.clearInterval(timer);
  window.clearTimeout(copyTimer);
  resizeObserver?.disconnect();
});

watch(
  () => [props.messages.length, props.busy, props.streamText, props.toolStatusText],
  async () => {
    const follow = distanceToBottom() <= 120;
    await nextTick();
    if (follow) scrollToLatest();
    else updateScrollPosition();
  }
);

watch(
  () => props.sessionId,
  async () => {
    expandedToolCalls.value = new Set();
    await nextTick();
    scrollToLatest();
  }
);

function distanceToBottom() {
  const element = listRef.value;
  return element ? element.scrollHeight - element.scrollTop - element.clientHeight : 0;
}

function updateScrollPosition() {
  followingLatest.value = distanceToBottom() <= 120;
  showScrollLatest.value = !followingLatest.value;
}

function scrollToLatest() {
  if (listRef.value) listRef.value.scrollTop = listRef.value.scrollHeight;
  followingLatest.value = true;
  showScrollLatest.value = false;
}

function toolCalls(message) {
  return message.toolCalls?.length ? message.toolCalls : message.toolCall ? [message.toolCall] : [];
}

function toolKey(message, call, index) {
  return `${message.id}-${call.id || index}`;
}

function toolName(name) {
  return (
    {
      create_image_tasks: '创建绘画任务',
      get_task_status: '查看生成进度',
      list_templates: '查找提示词模板',
    }[name] || name
  );
}

function groupStatusLabel(group) {
  if (group.status === 'completed')
    return group.images?.length ? `已完成 · ${group.images.length} 张图片` : '已完成';
  return (
    {
      queued: '已加入队列',
      running: '正在生成图片',
      cancelling: '正在取消…',
      failed: '生成未完成',
      cancelled: '已取消',
      missing: '任务记录不可用',
    }[group.status] || '正在读取任务状态'
  );
}

function previewAttachments(message, index) {
  emit('preview-images', {
    items: message.attachments.map((attachment) => ({
      path: attachment.path || attachment.dataUrl,
      title: attachment.fileName || '参考图',
    })),
    index,
  });
}

async function copyMessage(message) {
  copyError.value = '';
  try {
    await navigator.clipboard.writeText(message.content);
    copiedMessage.value = message.id;
    window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => {
      copiedMessage.value = '';
    }, 2000);
  } catch {
    copyError.value = message.id;
  }
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
