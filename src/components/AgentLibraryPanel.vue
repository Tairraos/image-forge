<template>
  <section class="agent-library">
    <main class="agent-library-content" :aria-busy="loading">
      <div v-if="loadError && dayGroups.length" class="library-load-notice" role="status">
        <span>暂时无法更新，仍显示上次加载的图片。</span
        ><button type="button" class="button button-small" :disabled="loading" @click="load">
          重新加载
        </button>
      </div>
      <div
        v-if="loading && !tasks.length"
        class="image-day-grid library-loading-grid"
        role="status"
        aria-label="正在加载图片库"
      >
        <span
          v-for="index in 8"
          :key="index"
          class="library-image-skeleton"
          aria-hidden="true"
        ></span>
      </div>
      <section v-for="group in dayGroups" :key="group.date" class="image-day-group">
        <header class="image-day-heading">
          <div>
            <h2>{{ formatDayHeading(group.date) }}</h2>
            <span>{{ formatFullDate(group.date) }}</span>
          </div>
          <strong>{{ group.imageCount }} 张</strong>
        </header>

        <div class="image-day-grid">
          <figure v-for="card in group.cards" :key="card.output.path" class="library-image-card">
            <div class="library-image-frame">
              <button
                type="button"
                class="library-image-preview"
                :aria-label="`预览图片：${card.task.prompt || card.output.fileName || card.output.file_name || '生成图片'}`"
                @click="openPreview(card.output.path)"
              >
                <span v-if="failedImages.has(card.output.path)" class="library-image-unavailable"
                  ><ImageOff :size="24" :stroke-width="1.4" /><span>图片暂时不可用</span></span
                >
                <img
                  v-else
                  loading="lazy"
                  :src="fileUrl(card.output.path)"
                  :alt="card.output.fileName || card.output.file_name || card.task.prompt"
                  @error="failedImages = new Set([...failedImages, card.output.path])"
                />
              </button>
              <div class="library-image-topbar">
                <time>{{ formatTime(card.time) }}</time>
                <span>{{
                  card.task.model || card.task.providerName || card.task.provider_name || ''
                }}</span>
                <span>{{ card.output.size || card.task.params?.size || '' }}</span>
              </div>
              <div class="library-image-footbar">
                <div v-if="card.referencePaths.length" class="library-image-ref-thumbs">
                  <button
                    v-for="(refPath, ri) in card.referencePaths"
                    :key="refPath"
                    type="button"
                    class="library-image-ref-thumb"
                    :title="`查看参考图 ${ri + 1}`"
                    :aria-label="`查看参考图 ${ri + 1}`"
                    @click.stop="openReferencePreview(card, refPath)"
                  >
                    <img :src="fileUrl(refPath)" :alt="`参考图 ${ri + 1}`" loading="lazy" />
                  </button>
                </div>
                <div class="library-image-actions">
                  <button
                    type="button"
                    :title="copiedTask === card.task.id ? '已复制' : '复制提示词'"
                    :aria-label="copiedTask === card.task.id ? '已复制' : '复制提示词'"
                    @click.stop="copyPrompt(card.task)"
                  >
                    <Check v-if="copiedTask === card.task.id" :size="14" /><Copy
                      v-else
                      :size="14"
                    />
                  </button>

                  <button
                    type="button"
                    title="引用到 Agent"
                    aria-label="引用到 Agent"
                    @click.stop="
                      $emit('reference-to-agent', { task: card.task, output: card.output })
                    "
                  >
                    <Link2 :size="14" />
                  </button>

                  <button
                    type="button"
                    title="添加到模板"
                    aria-label="添加到模板"
                    @click.stop="$emit('add-to-template', { task: card.task, output: card.output })"
                  >
                    <BookmarkPlus :size="14" />
                  </button>

                  <button
                    type="button"
                    title="下载图片"
                    aria-label="下载图片"
                    @click.stop="$emit('download-output', card.output)"
                  >
                    <Download :size="14" />
                  </button>

                  <button
                    type="button"
                    title="在 Finder 中显示"
                    aria-label="在 Finder 中显示"
                    @click.stop="$emit('reveal-output', card.output)"
                  >
                    <FolderOpen :size="14" />
                  </button>

                  <button
                    type="button"
                    title="删除任务及图片"
                    aria-label="删除任务及图片"
                    @click.stop="$emit('delete-task', card.task)"
                  >
                    <Trash2 :size="14" />
                  </button>
                </div>
              </div>
            </div>
            <figcaption class="library-image-caption" :title="card.task.prompt">
              {{ card.task.prompt || '未命名作品' }}
            </figcaption>
          </figure>
        </div>
      </section>

      <div v-if="!loading && !dayGroups.length" class="image-library-empty">
        <Images :size="28" />
        <strong>{{
          loadError ? '图片库加载失败' : hasFilters ? '没有匹配的图片' : '还没有作品'
        }}</strong>
        <span v-if="loadError" class="image-library-error">{{ loadError }}</span>
        <span v-else>{{
          hasFilters
            ? '试试其它关键词，或清除筛选条件。'
            : '在对话中描述一个画面，第一张作品会出现在这里。'
        }}</span>
        <button v-if="loadError" type="button" class="button button-small" @click="load">
          重新加载
        </button>
        <button
          v-else-if="hasFilters"
          type="button"
          class="button button-small"
          @click="clearFilters"
        >
          清除筛选
        </button>
        <button
          v-else
          type="button"
          class="button button-primary button-small"
          @click="$emit('start-creation')"
        >
          开始创作
        </button>
      </div>
    </main>

    <footer class="agent-library-bar">
      <label class="search-field"
        ><Search :size="16" /><input
          v-model="query"
          type="search"
          placeholder="搜索全部图片的提示词"
          aria-label="搜索提示词"
      /></label>
      <label class="library-filter"
        ><Filter :size="15" /><select v-model="sourceFilter" aria-label="按来源筛选">
          <option v-for="option in sourceOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select></label
      >
      <label class="library-filter"
        ><Calendar :size="15" /><select v-model="month" :disabled="searching" aria-label="选择月份">
          <option v-for="option in monthOptions" :key="option.value" :value="option.value">
            {{ searching ? '全部月份' : option.label }}
          </option>
        </select></label
      >
      <div class="agent-library-month-nav" role="group" aria-label="切换月份">
        <button
          type="button"
          title="上一月"
          aria-label="上一月"
          :disabled="searching || !prevMonth"
          @click="goMonth(prevMonth)"
        >
          <ChevronLeft :size="17" />
        </button>
        <button
          type="button"
          title="下一月"
          aria-label="下一月"
          :disabled="searching || !nextMonth"
          @click="goMonth(nextMonth)"
        >
          <ChevronRight :size="17" />
        </button>
      </div>
      <span class="library-result-count" role="status">{{
        copyFeedback || (loading ? '加载中…' : `${visibleImages.length} 张图片`)
      }}</span>
    </footer>
  </section>
</template>

<script setup>
import {
  BookmarkPlus,
  Calendar,
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  Filter,
  FolderOpen,
  Images,
  ImageOff,
  Link2,
  Search,
  Trash2,
} from '@lucide/vue';
import { computed, onUnmounted, ref, watch } from 'vue';
import { fileUrl } from '../lib/formatters';
import {
  dateKey,
  formatDayHeading,
  formatFullDate,
  formatMonth,
  formatTime,
  previewItem,
  referencePreviewItem,
  taskReferencePaths,
  taskSource,
  taskSourceOptions,
  taskTime,
} from '../lib/libraryFormat';
import * as api from '../api/index.js';

const props = defineProps({
  version: { type: Number, default: 0 },
});
const emit = defineEmits([
  'preview-images',
  'delete-task',
  'download-output',
  'reveal-output',
  'reference-to-agent',
  'add-to-template',
  'start-creation',
]);

const month = ref('');
const query = ref('');
const months = ref([]);
const tasks = ref([]);
const loading = ref(false);
const loadError = ref('');
const sourceFilter = ref('all');
const failedImages = ref(new Set());
const copiedTask = ref('');
const copyFeedback = ref('');
let queryTimer = 0;
let copyTimer = 0;
let requestId = 0;

const sourceOptions = taskSourceOptions();
const filteredTasks = computed(() =>
  sourceFilter.value === 'all'
    ? tasks.value
    : tasks.value.filter((task) => taskSource(task) === sourceFilter.value)
);

const searching = computed(() => query.value.trim() !== '');
const hasFilters = computed(() => searching.value || month.value || sourceFilter.value !== 'all');
const monthOptions = computed(() => [
  { label: '全部月份', value: '' },
  ...months.value.map((item) => ({
    label: `${formatMonth(item.date)}（${item.imageCount} 张）`,
    value: item.date,
  })),
]);
const prevMonth = computed(() => {
  if (!month.value) return '';
  const current = month.value;
  return (
    months.value
      .filter((item) => item.date < current)
      .sort((a, b) => b.date.localeCompare(a.date))[0]?.date || ''
  );
});
const nextMonth = computed(() => {
  if (!month.value) return '';
  const current = month.value;
  return (
    months.value
      .filter((item) => item.date > current)
      .sort((a, b) => a.date.localeCompare(b.date))[0]?.date || ''
  );
});

const visibleImages = computed(() =>
  filteredTasks.value.flatMap((task) => task.outputs.map((output) => previewItem(task, output)))
);
const dayGroups = computed(() => {
  const groups = new Map();
  for (const task of filteredTasks.value) {
    const date = dateKey(taskTime(task));
    if (!date) continue;
    if (!groups.has(date)) groups.set(date, []);
    groups.get(date).push(task);
  }
  return Array.from(groups, ([date, groupTasks]) => ({
    date,
    cards: groupTasks.flatMap((task) =>
      task.outputs.map((output) => ({
        task,
        output,
        time: taskTime(task),
        referencePaths: taskReferencePaths(task),
      }))
    ),
    imageCount: groupTasks.reduce((count, task) => count + task.outputs.length, 0),
  }));
});

async function load() {
  const current = ++requestId;
  loading.value = true;
  try {
    const result = await api.agentLibrary(month.value, query.value);
    if (current !== requestId) return;
    tasks.value = result.tasks || [];
    months.value = result.months || [];
    loadError.value = '';
    failedImages.value = new Set();
  } catch (error) {
    console.error('图片库加载失败:', error);
    if (current === requestId) {
      loadError.value = String(error);
    }
  } finally {
    if (current === requestId) loading.value = false;
  }
}

function goMonth(value) {
  if (value) month.value = value;
}

function clearFilters() {
  query.value = '';
  month.value = '';
  sourceFilter.value = 'all';
}

function openPreview(path) {
  const index = visibleImages.value.findIndex((item) => item.path === path);
  emit('preview-images', { items: visibleImages.value, index: Math.max(0, index) });
}

// 点击参考图缩略图与点击主图一致：打开大图查看器，尽可能 1:1 显示参考图。
function openReferencePreview(card, refPath) {
  const items = card.referencePaths.map((path, index) =>
    referencePreviewItem(card.task, path, index, card.referencePaths.length)
  );
  emit('preview-images', {
    items,
    index: Math.max(
      0,
      items.findIndex((item) => item.path === refPath)
    ),
  });
}

async function copyPrompt(task) {
  try {
    await navigator.clipboard.writeText(task.prompt || '');
    copiedTask.value = task.id;
    copyFeedback.value = '已复制提示词';
  } catch {
    copyFeedback.value = '复制失败，请在预览中手动复制';
  }
  window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copiedTask.value = '';
    copyFeedback.value = '';
  }, 2400);
}

watch(
  [month, query, () => props.version],
  (current, previous = []) => {
    window.clearTimeout(queryTimer);
    requestId += 1;
    loading.value = true;
    if (previous.length && current[1] !== previous[1]) queryTimer = window.setTimeout(load, 250);
    else void load();
  },
  { immediate: true }
);
onUnmounted(() => {
  requestId += 1;
  window.clearTimeout(queryTimer);
  window.clearTimeout(copyTimer);
});
</script>
