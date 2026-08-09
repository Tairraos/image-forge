<template>
  <section class="image-library">
    <header class="image-library-toolbar">
      <div class="image-library-title">
        <strong>图片库</strong>
        <span>{{ totalImages }} 张图片</span>
      </div>
      <n-input
        v-model:value="query"
        size="small"
        clearable
        placeholder="搜索提示词、模型或任务 ID"
        aria-label="搜索图片库"
      >
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <div class="image-source-filter" role="group" aria-label="图片来源">
        <button
          v-for="option in sourceOptions"
          :key="option.value"
          type="button"
          :class="{ active: source === option.value }"
          :aria-pressed="source === option.value"
          @click="source = option.value"
        >
          {{ option.label }}
        </button>
      </div>
    </header>

    <div class="image-library-layout">
      <aside class="image-calendar-panel">
        <div class="image-calendar-head">
          <button type="button" title="上个月" aria-label="上个月" @click="moveMonth(-1)">
            <ChevronLeft :size="17" />
          </button>
          <input v-model="calendarMonth" type="month" aria-label="选择月份" />
          <button type="button" title="下个月" aria-label="下个月" @click="moveMonth(1)">
            <ChevronRight :size="17" />
          </button>
        </div>
        <div class="image-calendar-weekdays" aria-hidden="true">
          <span v-for="day in weekdays" :key="day">{{ day }}</span>
        </div>
        <div class="image-calendar-grid">
          <button
            v-for="day in calendarDays"
            :key="day.key"
            type="button"
            :class="{
              outside: !day.currentMonth,
              selected: selectedDate === day.key,
              populated: day.count > 0,
            }"
            :aria-label="`${day.key}，${day.count} 张图片`"
            :aria-pressed="selectedDate === day.key"
            @click="selectDate(day.key)"
          >
            <span>{{ day.day }}</span>
            <small v-if="day.count">{{ day.count }}</small>
          </button>
        </div>
        <button
          v-if="selectedDate"
          type="button"
          class="image-calendar-clear"
          @click="selectedDate = ''"
        >
          查看全部日期
        </button>
        <p class="image-calendar-summary">
          {{ selectedDate ? formatFullDate(selectedDate) : `${formatMonth(calendarMonth)} · 点击日期筛选` }}
        </p>
      </aside>

      <main class="image-library-content" :aria-busy="loading">
        <section v-for="group in dayGroups" :key="group.date" class="image-day-group">
          <header class="image-day-heading">
            <div>
              <h2>{{ formatDayHeading(group.date) }}</h2>
              <span>{{ formatFullDate(group.date) }}</span>
            </div>
            <strong>{{ group.imageCount }} 张</strong>
          </header>

          <article v-for="task in group.tasks" :key="task.id" class="image-batch">
            <header class="image-batch-head">
              <div>
                <div class="image-batch-source">
                  <span :data-source="taskSource(task)">{{ taskSourceLabel(task) }}</span>
                  <time>{{ formatTime(taskTime(task)) }}</time>
                </div>
                <strong :title="task.prompt">{{ task.prompt || "空提示词" }}</strong>
                <small>{{ task.model || task.providerName || task.id }}</small>
              </div>
              <div class="image-batch-actions">
                <button type="button" title="在绘画视图中打开" aria-label="在绘画视图中打开" @click="$emit('open-task', task)">
                  <Paintbrush :size="16" />
                </button>
                <button type="button" title="删除任务及图片" aria-label="删除任务及图片" @click="$emit('delete-task', task)">
                  <Trash2 :size="16" />
                </button>
              </div>
            </header>

            <div class="image-batch-grid">
              <figure v-for="output in task.outputs" :key="output.path" class="library-image-card">
                <button type="button" class="library-image-preview" @click="openPreview(output.path)">
                  <img loading="lazy" :src="fileUrl(output.path)" :alt="output.fileName || task.prompt" />
                </button>
                <figcaption>
                  <span>{{ output.size || task.params?.size }}</span>
                  <div>
                    <button type="button" title="下载图片" aria-label="下载图片" @click="$emit('download-output', output)">
                      <Download :size="15" />
                    </button>
                    <button type="button" title="在 Finder 中显示" aria-label="在 Finder 中显示" @click="$emit('reveal-output', output)">
                      <FolderOpen :size="15" />
                    </button>
                  </div>
                </figcaption>
              </figure>
            </div>
          </article>
        </section>

        <div v-if="!dayGroups.length" class="image-library-empty">
          <Images :size="28" />
          <strong>没有找到图片</strong>
          <span>调整日期、来源或搜索条件</span>
        </div>
        <footer v-if="totalPages > 1" class="image-library-pagination">
          <n-pagination
            :page="page"
            :page-count="totalPages"
            :page-slot="7"
            @update:page="requestPage"
          />
        </footer>
      </main>
    </div>
  </section>
</template>

<script setup>
import {
  ChevronLeft,
  ChevronRight,
  Download,
  FolderOpen,
  Images,
  Paintbrush,
  Search,
  Trash2,
} from "@lucide/vue";
import { computed, onUnmounted, ref, watch } from "vue";
import { fileUrl } from "../lib/formatters";
import {
  dateKey,
  formatDayHeading,
  formatFullDate,
  formatMonth,
  formatTime,
  monthKey,
  previewItem,
  taskSource,
  taskSourceLabel,
  taskTime,
} from "../lib/libraryFormat";

const props = defineProps({
  tasks: { type: Array, default: () => [] },
  dayCounts: { type: Array, default: () => [] },
  totalTasks: { type: Number, default: 0 },
  totalImages: { type: Number, default: 0 },
  page: { type: Number, default: 1 },
  pageSize: { type: Number, default: 40 },
  loading: { type: Boolean, default: false },
});

const emit = defineEmits(["request-page", "preview-images", "open-task", "delete-task", "download-output", "reveal-output"]);
const query = ref("");
const source = ref("all");
const selectedDate = ref("");
const calendarMonth = ref(monthKey(new Date()));
const weekdays = ["一", "二", "三", "四", "五", "六", "日"];
const sourceOptions = [
  { label: "全部", value: "all" },
  { label: "绘画", value: "drawing" },
  { label: "Agent", value: "agent" },
];
let queryTimer = 0;

const libraryTasks = computed(() => props.tasks
  .filter((task) => task.status === "completed" && task.outputs?.length)
  .sort((left, right) => taskTime(right).localeCompare(taskTime(left))));
const visibleTasks = libraryTasks;

const visibleImages = computed(() => visibleTasks.value.flatMap((task) =>
  task.outputs.map((output) => previewItem(task, output))));

const dayGroups = computed(() => {
  const groups = new Map();
  for (const task of visibleTasks.value) {
    const date = dateKey(taskTime(task));
    if (!groups.has(date)) groups.set(date, []);
    groups.get(date).push(task);
  }
  return Array.from(groups, ([date, tasks]) => ({
    date,
    tasks,
    imageCount: tasks.reduce((count, task) => count + task.outputs.length, 0),
  }));
});

const imageCountByDate = computed(() => {
  return new Map(props.dayCounts.map((item) => [item.date, item.imageCount]));
});
const totalPages = computed(() => Math.max(1, Math.ceil(props.totalTasks / props.pageSize)));

const calendarDays = computed(() => {
  const [year, month] = normalizedMonth().split("-").map(Number);
  const first = new Date(year, month - 1, 1);
  const mondayOffset = (first.getDay() + 6) % 7;
  const start = new Date(year, month - 1, 1 - mondayOffset);
  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start);
    date.setDate(start.getDate() + index);
    const key = dateKey(date);
    return {
      key,
      day: date.getDate(),
      currentMonth: date.getMonth() === month - 1,
      count: imageCountByDate.value.get(key) || 0,
    };
  });
});

function openPreview(path) {
  const index = visibleImages.value.findIndex((item) => item.path === path);
  emit("preview-images", { items: visibleImages.value, index: Math.max(0, index) });
}

function requestPage(nextPage = 1) {
  emit("request-page", {
    month: normalizedMonth(),
    date: selectedDate.value,
    query: query.value.trim(),
    origin: source.value,
    page: nextPage,
    pageSize: props.pageSize,
  });
}

function selectDate(value) {
  selectedDate.value = selectedDate.value === value ? "" : value;
  calendarMonth.value = value.slice(0, 7);
}

function moveMonth(offset) {
  const [year, month] = normalizedMonth().split("-").map(Number);
  calendarMonth.value = monthKey(new Date(year, month - 1 + offset, 1));
}

watch(calendarMonth, () => {
  if (selectedDate.value && !selectedDate.value.startsWith(normalizedMonth())) {
    selectedDate.value = "";
    return;
  }
  requestPage(1);
}, { immediate: true });
watch([selectedDate, source], () => requestPage(1));
watch(query, () => {
  window.clearTimeout(queryTimer);
  queryTimer = window.setTimeout(() => requestPage(1), 250);
});
onUnmounted(() => window.clearTimeout(queryTimer));

function normalizedMonth() {
  return /^\d{4}-\d{2}$/.test(calendarMonth.value) ? calendarMonth.value : monthKey(new Date());
}
</script>
