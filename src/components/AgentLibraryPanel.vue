<template>
  <section class="agent-library">
    <main class="agent-library-content" :aria-busy="loading">
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
                @click="openPreview(card.output.path)"
              >
                <img
                  loading="lazy"
                  :src="fileUrl(card.output.path)"
                  :alt="card.output.fileName || card.output.file_name || card.task.prompt"
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
                    :aria-label="`查看参考图 ${ri + 1}`"
                    @click.stop="openReferencePreview(card, refPath)"
                  >
                    <img :src="fileUrl(refPath)" :alt="`参考图 ${ri + 1}`" loading="lazy" />
                  </button>
                </div>
                <div class="library-image-actions">
                  <n-tooltip trigger="hover" :delay="0">
                    <template #trigger>
                      <button
                        type="button"
                        aria-label="复制提示词"
                        @click.stop="copyPrompt(card.task)"
                      >
                        <Copy :size="14" />
                      </button>
                    </template>
                    复制提示词
                  </n-tooltip>
                  <n-tooltip trigger="hover" :delay="0">
                    <template #trigger>
                      <button
                        type="button"
                        aria-label="引用到 Agent"
                        @click.stop="
                          $emit('reference-to-agent', { task: card.task, output: card.output })
                        "
                      >
                        <Link2 :size="14" />
                      </button>
                    </template>
                    引用到 Agent
                  </n-tooltip>
                  <n-tooltip trigger="hover" :delay="0">
                    <template #trigger>
                      <button
                        type="button"
                        aria-label="添加到模板"
                        @click.stop="
                          $emit('add-to-template', { task: card.task, output: card.output })
                        "
                      >
                        <BookmarkPlus :size="14" />
                      </button>
                    </template>
                    添加到模板
                  </n-tooltip>
                  <n-tooltip trigger="hover" :delay="0">
                    <template #trigger>
                      <button
                        type="button"
                        aria-label="下载图片"
                        @click.stop="$emit('download-output', card.output)"
                      >
                        <Download :size="14" />
                      </button>
                    </template>
                    下载
                  </n-tooltip>
                  <n-tooltip trigger="hover" :delay="0">
                    <template #trigger>
                      <button
                        type="button"
                        aria-label="在 Finder 中显示"
                        @click.stop="$emit('reveal-output', card.output)"
                      >
                        <FolderOpen :size="14" />
                      </button>
                    </template>
                    在 Finder 中显示
                  </n-tooltip>
                  <n-tooltip trigger="hover" :delay="0">
                    <template #trigger>
                      <button
                        type="button"
                        aria-label="删除任务及图片"
                        @click.stop="$emit('delete-task', card.task)"
                      >
                        <Trash2 :size="14" />
                      </button>
                    </template>
                    删除任务及图片
                  </n-tooltip>
                </div>
              </div>
            </div>
          </figure>
        </div>
      </section>

      <div v-if="!dayGroups.length" class="image-library-empty">
        <Images :size="28" />
        <strong>{{ loadError ? '图片库加载失败' : '没有找到图片' }}</strong>
        <span v-if="loadError" class="image-library-error">{{ loadError }}</span>
        <span v-else>{{
          searching ? '换个关键词试试' : months.length ? '这个月还没有图片' : '还没有图片'
        }}</span>
      </div>
    </main>

    <footer class="agent-library-bar">
      <n-input
        v-model:value="query"
        size="small"
        clearable
        placeholder="搜索提示词（全部图片）"
        aria-label="搜索提示词"
      >
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <n-popselect
        v-model:value="sourceFilter"
        :options="sourceOptions"
        placement="top-start"
        trigger="click"
      >
        <button class="agent-library-month" type="button" aria-label="按来源筛选">
          <Filter :size="15" />
          <span>{{ sourceLabel }}</span>
        </button>
      </n-popselect>
      <n-popselect
        v-model:value="month"
        :options="monthOptions"
        placement="top-start"
        trigger="click"
        :disabled="searching"
      >
        <button
          class="agent-library-month"
          type="button"
          :disabled="searching"
          aria-label="选择月份"
        >
          <Calendar :size="15" />
          <span>{{ searching ? '全部月份' : monthLabel }}</span>
        </button>
      </n-popselect>
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
    </footer>
  </section>
</template>

<script setup>
import {
  BookmarkPlus,
  Calendar,
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  Filter,
  FolderOpen,
  Images,
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
]);

const month = ref('');
const query = ref('');
const months = ref([]);
const tasks = ref([]);
const loading = ref(false);
const loadError = ref('');
const sourceFilter = ref('all');
let queryTimer = 0;
let requestId = 0;

const sourceOptions = taskSourceOptions();
const sourceLabel = computed(
  () => sourceOptions.find((option) => option.value === sourceFilter.value)?.label || '全部来源'
);
const filteredTasks = computed(() =>
  sourceFilter.value === 'all'
    ? tasks.value
    : tasks.value.filter((task) => taskSource(task) === sourceFilter.value)
);

const searching = computed(() => query.value.trim() !== '');
const monthLabel = computed(() => (month.value ? formatMonth(month.value) : '全部月份'));
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
  } catch (error) {
    console.error('图片库加载失败:', error);
    if (current === requestId) {
      tasks.value = [];
      months.value = [];
      loadError.value = String(error);
    }
  } finally {
    if (current === requestId) loading.value = false;
  }
}

function goMonth(value) {
  if (value) month.value = value;
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
  } catch {
    // 复制失败静默忽略
  }
}

watch(month, load, { immediate: true });
watch(query, () => {
  window.clearTimeout(queryTimer);
  queryTimer = window.setTimeout(load, 250);
});
watch(() => props.version, load);
onUnmounted(() => window.clearTimeout(queryTimer));
</script>
