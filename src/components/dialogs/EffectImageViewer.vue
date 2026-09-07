<template>
  <NativeDialog
    v-model:show="show"
    class="effect-image-viewer"
    :title="currentTitle"
    frameless
    :mask-closable="false"
    :close-on-esc="false"
    @keydown="closeOnKey"
  >
    <div class="effect-image-viewer-stage" @click="handleStageClick">
      <!-- 顶部标题条：左半提示词，右侧 info 与操作一上一下 -->
      <header class="viewer-title-bar" :class="{ expanded }" @click.stop>
        <button
          type="button"
          class="viewer-title-prompts"
          :aria-expanded="expanded"
          :disabled="!currentPrompt && !currentRevisedPrompt"
          aria-controls="viewer-prompt-details"
          :aria-label="expanded ? '收起提示词' : '展开提示词'"
          @click="expanded = !expanded"
        >
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
          <ChevronDown
            v-if="currentPrompt || currentRevisedPrompt"
            class="viewer-details-chevron"
            :size="14"
          />
        </button>
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
        <button
          type="button"
          class="icon-button viewer-close"
          aria-label="关闭预览"
          title="关闭预览 · Esc"
          autofocus
          @click.stop="show = false"
        >
          <X :size="18" />
        </button>
      </header>

      <div v-if="expanded" id="viewer-prompt-details" class="viewer-expand-panel" @click.stop>
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
      <div class="viewer-image-canvas" :class="{ zoomed }">
        <img
          v-if="currentPath"
          :key="`${currentPath}-${imageAttempt}`"
          class="viewer-main-image"
          :class="{ 'is-ready': imageState === 'ready' }"
          :src="convertFileSrc(currentPath)"
          :alt="currentTitle"
          @load="updateImageSize"
          @error="imageState = 'error'"
          @click.stop
          @dblclick.stop="zoomed = !zoomed"
        />
      </div>
      <div v-if="imageState !== 'ready'" class="viewer-image-state" role="status" @click.stop>
        <LoaderCircle v-if="imageState === 'loading'" class="spinning" :size="24" />
        <ImageOff v-else :size="28" :stroke-width="1.4" />
        <strong>{{ imageState === 'loading' ? '正在载入图片' : '暂时无法显示这张图片' }}</strong>
        <span v-if="imageState === 'error'">图片可能已移动，或连接暂时不可用。</span>
        <button
          v-if="imageState === 'error'"
          type="button"
          class="button button-small"
          @click="retryImage"
        >
          重新加载
        </button>
      </div>
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

      <div class="viewer-bottom-bar" @click.stop>
        <span v-if="viewerItems.length > 1" class="viewer-position" aria-live="polite"
          >{{ currentIndex + 1 }} / {{ viewerItems.length }}</span
        >
        <div class="viewer-zoom-control" role="group" aria-label="图片缩放">
          <button type="button" :aria-pressed="!zoomed" @click="zoomed = false">适应窗口</button>
          <button
            type="button"
            :aria-pressed="zoomed"
            :disabled="imageState !== 'ready'"
            @click="zoomed = true"
          >
            100%
          </button>
        </div>
        <span class="viewer-keyboard-hint"
          >{{ viewerItems.length > 1 ? '← → 切换 · ' : '' }}Esc 关闭</span
        >
      </div>
      <span v-if="actionMessage" class="viewer-action-feedback" role="status">{{
        actionMessage
      }}</span>

      <!-- 参考图放大叠层：点击任意位置关闭 -->
      <div v-if="overlayPath" class="viewer-ref-overlay" @click.stop="overlayPath = ''">
        <button
          type="button"
          class="icon-button viewer-ref-close"
          aria-label="关闭参考图"
          @click.stop="overlayPath = ''"
        >
          <X :size="20" />
        </button>
        <img
          v-show="overlayState !== 'error'"
          :src="convertFileSrc(overlayPath)"
          :alt="overlayTitle"
          @load="overlayState = 'ready'"
          @error="overlayState = 'error'"
        />
        <div v-if="overlayState === 'error'" class="viewer-image-state">
          <ImageOff :size="26" /><strong>参考图暂时不可用</strong
          ><span>关闭预览后可重新添加参考图。</span>
        </div>
        <span class="viewer-ref-caption">{{ overlayTitle }} · 点击返回</span>
      </div>
    </div>
  </NativeDialog>
</template>

<script setup>
import NativeDialog from './NativeDialog.vue';
import {
  BookmarkPlus,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  FolderOpen,
  ImageOff,
  Link2,
  LoaderCircle,
  Trash2,
  X,
} from '@lucide/vue';
import { computed, onUnmounted, reactive, ref, watch } from 'vue';
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
const currentIndex = defineModel('index', { type: Number, default: 0 });
const expanded = ref(false);
const overlayPath = ref('');
const imageState = ref('loading');
const overlayState = ref('loading');
const imageAttempt = ref(0);
const zoomed = ref(false);
const actionMessage = ref('');
let actionTimer = 0;

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
  const size =
    naturalSize.width && naturalSize.height
      ? `${naturalSize.width} × ${naturalSize.height}`
      : item.size;
  return [formatTime(item.time || ''), item.model, size].filter(Boolean).join(' · ');
});

const actions = [
  { key: 'copy', label: '复制提示词', icon: Copy },
  { key: 'reference', label: '引用到 Agent', icon: Link2 },
  { key: 'template', label: '添加到模板', icon: BookmarkPlus },
  { key: 'download', label: '下载图片', icon: Download },
  { key: 'reveal', label: '在 Finder 中显示', icon: FolderOpen },
  { key: 'delete', label: '删除任务及图片', icon: Trash2 },
];

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
    imageState.value = 'loading';
    zoomed.value = false;
    actionMessage.value = '';
    expanded.value = false;
    overlayPath.value = '';
  },
  { immediate: true }
);

watch(currentIndex, () => {
  naturalSize.width = 0;
  naturalSize.height = 0;
  imageState.value = 'loading';
  zoomed.value = false;
  overlayPath.value = '';
});

watch(overlayPath, () => {
  overlayState.value = 'loading';
});

onUnmounted(() => window.clearTimeout(actionTimer));

function updateImageSize(event) {
  naturalSize.width = event.target.naturalWidth;
  naturalSize.height = event.target.naturalHeight;
  imageState.value = 'ready';
}

function retryImage() {
  imageState.value = 'loading';
  imageAttempt.value += 1;
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
    event.preventDefault();
    if (overlayPath.value) overlayPath.value = '';
    else if (expanded.value) expanded.value = false;
    else show.value = false;
  }
  if (
    event.ctrlKey ||
    event.metaKey ||
    event.altKey ||
    event.shiftKey ||
    expanded.value ||
    overlayPath.value ||
    zoomed.value
  )
    return;
  if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
    event.preventDefault();
    move(event.key === 'ArrowLeft' ? -1 : 1);
  }
}

function move(offset) {
  const count = viewerItems.value.length;
  if (count < 2) return;
  currentIndex.value = (currentIndex.value + offset + count) % count;
  naturalSize.width = 0;
  naturalSize.height = 0;
}

async function runAction(key) {
  const item = currentItem.value;
  if (!item?.task) return;
  if (key === 'copy') {
    try {
      await navigator.clipboard.writeText(item.prompt || item.task.prompt || '');
      actionMessage.value = '已复制提示词';
    } catch {
      actionMessage.value = '复制失败，请展开提示词后手动复制';
    }
    window.clearTimeout(actionTimer);
    actionTimer = window.setTimeout(() => {
      actionMessage.value = '';
    }, 2400);
    return;
  }
  if (key === 'download') emit('download-output', item);
  if (key === 'reveal') emit('reveal-output', item);
  if (key === 'delete') emit('delete-task', item.task);
  if (key === 'reference') emit('reference-to-agent', { task: item.task, output: item });
  if (key === 'template') emit('add-to-template', { task: item.task, output: item });
}
</script>
