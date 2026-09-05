<template>
  <n-modal v-model:show="show" class="effect-image-viewer" :mask-closable="false">
    <div
      class="effect-image-viewer-stage"
      role="dialog"
      aria-modal="true"
      :aria-label="currentTitle"
      @click="handleStageClick"
    >
      <!-- 顶部标题条：左半提示词，右侧 info 与操作一上一下 -->
      <header class="viewer-title-bar" :class="{ expanded }" @click.stop="expanded = !expanded">
        <div class="viewer-title-prompts">
          <span class="viewer-prompt-row" :title="currentPrompt">
            {{ currentPrompt || currentTitle }}
          </span>
          <span
            v-if="currentRevisedPrompt"
            class="viewer-prompt-row revised"
            :title="currentRevisedPrompt"
          >
            {{ currentRevisedPrompt }}
          </span>
        </div>
        <div class="viewer-title-side">
          <span class="viewer-title-info">{{ infoLine }}</span>
          <div class="viewer-title-tools">
            <div v-if="currentReferencePaths.length" class="library-image-ref-thumbs">
              <button
                v-for="(refPath, ri) in currentReferencePaths"
                :key="refPath"
                type="button"
                class="library-image-ref-thumb"
                :aria-label="`放大参考图 ${ri + 1}`"
                @click.stop="overlayPath = refPath"
              >
                <img :src="convertFileSrc(refPath)" :alt="`参考图 ${ri + 1}`" loading="lazy" />
              </button>
            </div>
            <div v-if="hasTaskActions" class="library-image-actions">
              <button
                v-for="action in actions"
                :key="action.key"
                type="button"
                :title="action.label"
                :aria-label="action.label"
                @click.stop="runAction(action.key)"
              >
                <component :is="action.icon" :size="12" />
              </button>
            </div>
          </div>
        </div>
      </header>

      <!-- 展开面板：上方全部提示词（可滚动），底部按钮与 info 常驻；点击任意空白处收起 -->
      <div v-if="expanded" class="viewer-expand-panel" @click.stop="expanded = false">
        <div class="viewer-expand-prompts">
          <section v-if="currentPrompt">
            <h3>提示词</h3>
            <p>{{ currentPrompt }}</p>
          </section>
          <section v-if="currentRevisedPrompt">
            <h3>改写提示词</h3>
            <p>{{ currentRevisedPrompt }}</p>
          </section>
        </div>
        <footer class="viewer-expand-footer">
          <div v-if="currentReferencePaths.length" class="library-image-ref-thumbs">
            <button
              v-for="(refPath, ri) in currentReferencePaths"
              :key="refPath"
              type="button"
              class="library-image-ref-thumb"
              :aria-label="`放大参考图 ${ri + 1}`"
              @click.stop="overlayPath = refPath"
            >
              <img :src="convertFileSrc(refPath)" :alt="`参考图 ${ri + 1}`" loading="lazy" />
            </button>
          </div>
          <span class="viewer-title-info">{{ infoLine }}</span>
          <div v-if="hasTaskActions" class="library-image-actions">
            <button
              v-for="action in actions"
              :key="action.key"
              type="button"
              :title="action.label"
              :aria-label="action.label"
              @click.stop="runAction(action.key)"
            >
              <component :is="action.icon" :size="12" />
            </button>
          </div>
        </footer>
      </div>

      <button
        v-if="viewerItems.length > 1"
        type="button"
        class="effect-image-viewer-nav previous"
        title="上一张"
        aria-label="上一张"
        @click.stop="move(-1)"
      >
        <ChevronLeft :size="28" />
      </button>
      <img
        v-if="currentPath"
        :src="convertFileSrc(currentPath)"
        :alt="currentTitle"
        :style="imageStyle"
        @load="updateImageSize"
        @click.stop
      />
      <button
        v-if="viewerItems.length > 1"
        type="button"
        class="effect-image-viewer-nav next"
        title="下一张"
        aria-label="下一张"
        @click.stop="move(1)"
      >
        <ChevronRight :size="28" />
      </button>

      <!-- 参考图放大叠层：点击任意位置关闭 -->
      <div v-if="overlayPath" class="viewer-ref-overlay" @click.stop="overlayPath = ''">
        <img
          :src="convertFileSrc(overlayPath)"
          :alt="overlayTitle"
          :style="overlayImageStyle"
          @load="updateOverlaySize"
        />
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import {
  BookmarkPlus,
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  FolderOpen,
  Link2,
  Trash2,
} from '@lucide/vue';
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import { convertFileSrc } from '../../tauri';
import { formatTime } from '../../lib/libraryFormat';

const show = defineModel('show', { type: Boolean, default: false });
const props = defineProps({
  imagePath: { type: String, default: '' },
  title: { type: String, default: '' },
  items: { type: Array, default: () => [] },
  initialIndex: { type: Number, default: 0 },
});
const emit = defineEmits([
  'download-output',
  'reveal-output',
  'delete-task',
  'reference-to-agent',
  'add-to-template',
]);

const naturalSize = reactive({ width: 0, height: 0 });
const overlaySize = reactive({ width: 0, height: 0 });
const viewport = reactive({ width: window.innerWidth, height: window.innerHeight });
const currentIndex = defineModel('index', { type: Number, default: 0 });
const expanded = ref(false);
const overlayPath = ref('');

const viewerItems = computed(() =>
  props.items.length
    ? props.items
    : props.imagePath
      ? [{ path: props.imagePath, title: props.title }]
      : []
);
const currentItem = computed(
  () => viewerItems.value[currentIndex.value] || viewerItems.value[0] || null
);
const currentPath = computed(() => currentItem.value?.path || '');
const currentTitle = computed(() => currentItem.value?.title || props.title || '图片预览');
const currentPrompt = computed(() => currentItem.value?.prompt || '');
const currentRevisedPrompt = computed(() => currentItem.value?.revisedPrompt || '');
const currentReferencePaths = computed(() => currentItem.value?.referencePaths || []);
const overlayTitle = computed(() => {
  const index = currentReferencePaths.value.indexOf(overlayPath.value);
  return index >= 0 ? `参考图 ${index + 1}` : '参考图';
});
const hasTaskActions = computed(() => Boolean(currentItem.value?.task));
const infoLine = computed(() => {
  const item = currentItem.value || {};
  return [formatTime(item.time || ''), item.model, item.size].filter(Boolean).join(' · ');
});

const actions = [
  { key: 'copy', label: '复制提示词', icon: Copy },
  { key: 'reference', label: '引用到 Agent', icon: Link2 },
  { key: 'template', label: '添加到模板', icon: BookmarkPlus },
  { key: 'download', label: '下载图片', icon: Download },
  { key: 'reveal', label: '在 Finder 中显示', icon: FolderOpen },
  { key: 'delete', label: '删除任务及图片', icon: Trash2 },
];

function fitStyle(size, marginX, marginY) {
  const width = size.width;
  const height = size.height;
  if (!width || !height) return {};
  const fitScale = Math.min(
    (viewport.width - marginX) / width,
    (viewport.height - marginY) / height
  );
  const scale = Math.min(1, fitScale);
  return {
    width: `${Math.max(1, Math.round(width * scale))}px`,
    height: `${Math.max(1, Math.round(height * scale))}px`,
  };
}

// 尽可能 1:1 显示，尺寸不够时保持比例缩小
const imageStyle = computed(() => fitStyle(naturalSize, 144, 176));
const overlayImageStyle = computed(() => fitStyle(overlaySize, 144, 144));

watch(
  () => [props.imagePath, props.items, props.initialIndex, show.value],
  () => {
    if (show.value) {
      currentIndex.value = Math.min(
        Math.max(0, props.initialIndex),
        Math.max(0, viewerItems.value.length - 1)
      );
    }
    naturalSize.width = 0;
    naturalSize.height = 0;
    expanded.value = false;
    overlayPath.value = '';
  },
  { deep: false }
);

watch(currentIndex, () => {
  naturalSize.width = 0;
  naturalSize.height = 0;
  overlayPath.value = '';
});

onMounted(() => {
  window.addEventListener('keydown', closeOnKey);
  window.addEventListener('resize', updateViewport);
});

onUnmounted(() => {
  window.removeEventListener('keydown', closeOnKey);
  window.removeEventListener('resize', updateViewport);
});

function updateImageSize(event) {
  naturalSize.width = event.target.naturalWidth;
  naturalSize.height = event.target.naturalHeight;
}

function updateOverlaySize(event) {
  overlaySize.width = event.target.naturalWidth;
  overlaySize.height = event.target.naturalHeight;
}

function handleStageClick() {
  if (expanded.value) {
    expanded.value = false;
    return;
  }
  show.value = false;
}

function closeOnKey(event) {
  if (!show.value) return;
  if (event.key === 'Escape') {
    if (overlayPath.value) overlayPath.value = '';
    else if (expanded.value) expanded.value = false;
    else show.value = false;
  }
  if (event.key === 'ArrowLeft') move(-1);
  if (event.key === 'ArrowRight') move(1);
}

function updateViewport() {
  viewport.width = window.innerWidth;
  viewport.height = window.innerHeight;
}

function move(offset) {
  const count = viewerItems.value.length;
  if (count < 2) return;
  currentIndex.value = (currentIndex.value + offset + count) % count;
  naturalSize.width = 0;
  naturalSize.height = 0;
}

function runAction(key) {
  const item = currentItem.value;
  if (!item?.task) return;
  if (key === 'copy') {
    navigator.clipboard?.writeText(item.prompt || item.task.prompt || '');
    return;
  }
  if (key === 'download') emit('download-output', item);
  if (key === 'reveal') emit('reveal-output', item);
  if (key === 'delete') emit('delete-task', item.task);
  if (key === 'reference') emit('reference-to-agent', { task: item.task, output: item });
  if (key === 'template') emit('add-to-template', { task: item.task, output: item });
}
</script>
