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
        <span>{{ searching ? "换个关键词试试" : (months.length ? "这个月还没有图片" : "还没有图片") }}</span>
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
        v-model:value="month"
        :options="monthOptions"
        placement="top-start"
        trigger="click"
        :disabled="searching"
      >
        <button class="agent-library-month" type="button" :disabled="searching" aria-label="选择月份">
          <Calendar :size="15" />
          <span>{{ searching ? "全部月份" : monthLabel }}</span>
        </button>
      </n-popselect>
      <div class="agent-library-month-nav" role="group" aria-label="切换月份">
        <button type="button" title="上一月" aria-label="上一月" :disabled="searching || !prevMonth" @click="goMonth(prevMonth)">
          <ChevronLeft :size="17" />
        </button>
        <button type="button" title="下一月" aria-label="下一月" :disabled="searching || !nextMonth" @click="goMonth(nextMonth)">
          <ChevronRight :size="17" />
        </button>
      </div>
    </footer>
  </section>
</template>

<script setup>
import {
  Calendar,
  ChevronLeft,
  ChevronRight,
  Download,
  FolderOpen,
  Images,
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
import { invoke } from "../tauri";

const props = defineProps({
  version: { type: Number, default: 0 },
});
const emit = defineEmits(["preview-images", "delete-task", "download-output", "reveal-output"]);

const month = ref(monthKey(new Date()));
const query = ref("");
const months = ref([]);
const tasks = ref([]);
const loading = ref(false);
let queryTimer = 0;
let requestId = 0;

const searching = computed(() => query.value.trim() !== "");
const monthLabel = computed(() => formatMonth(month.value));
const monthOptions = computed(() =>
  months.value.map((item) => ({
    label: `${formatMonth(item.date)}（${item.imageCount} 张）`,
    value: item.date,
  })),
);
const prevMonth = computed(() => {
  const current = month.value;
  return months.value.filter((item) => item.date < current).sort((a, b) => b.date.localeCompare(a.date))[0]?.date || "";
});
const nextMonth = computed(() => {
  const current = month.value;
  return months.value.filter((item) => item.date > current).sort((a, b) => a.date.localeCompare(b.date))[0]?.date || "";
});

const visibleImages = computed(() =>
  tasks.value.flatMap((task) => task.outputs.map((output) => previewItem(task, output))),
);
const dayGroups = computed(() => {
  const groups = new Map();
  for (const task of tasks.value) {
    const date = dateKey(taskTime(task));
    if (!date) continue;
    if (!groups.has(date)) groups.set(date, []);
    groups.get(date).push(task);
  }
  return Array.from(groups, ([date, groupTasks]) => ({
    date,
    tasks: groupTasks,
    imageCount: groupTasks.reduce((count, task) => count + task.outputs.length, 0),
  }));
});

async function load() {
  const current = ++requestId;
  loading.value = true;
  try {
    const result = await invoke("agent_library", { month: month.value, query: query.value });
    if (current !== requestId) return;
    tasks.value = result.tasks || [];
    months.value = result.months || [];
  } catch (error) {
    if (current === requestId) tasks.value = [];
  } finally {
    if (current === requestId) loading.value = false;
  }
}

function goMonth(value) {
  if (value) month.value = value;
}

function openPreview(path) {
  const index = visibleImages.value.findIndex((item) => item.path === path);
  emit("preview-images", { items: visibleImages.value, index: Math.max(0, index) });
}

watch(month, load, { immediate: true });
watch(query, () => {
  window.clearTimeout(queryTimer);
  queryTimer = window.setTimeout(load, 250);
});
watch(() => props.version, load);
onUnmounted(() => window.clearTimeout(queryTimer));
</script>
