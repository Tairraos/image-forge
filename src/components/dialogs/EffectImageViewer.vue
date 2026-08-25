<template>
  <n-modal v-model:show="show" class="effect-image-viewer" :mask-closable="true">
    <div
      class="effect-image-viewer-stage"
      role="dialog"
      aria-modal="true"
      :aria-label="currentTitle"
      @click="show = false"
    >
      <header class="effect-image-viewer-head" @click.stop>
        <div>
          <strong>{{ currentTitle }}</strong>
          <span v-if="currentItem?.meta">{{ currentItem.meta }}</span>
        </div>
        <span v-if="viewerItems.length > 1">{{ currentIndex + 1 }} / {{ viewerItems.length }}</span>
        <button type="button" title="关闭" aria-label="关闭大图" @click="show = false">
          <X :size="20" />
        </button>
      </header>
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
    </div>
  </n-modal>
</template>

<script setup>
import { ChevronLeft, ChevronRight, X } from '@lucide/vue';
import { computed, onMounted, onUnmounted, reactive, watch } from 'vue';
import { convertFileSrc } from '../../tauri';

const show = defineModel('show', { type: Boolean, default: false });
const props = defineProps({
  imagePath: { type: String, default: '' },
  title: { type: String, default: '' },
  items: { type: Array, default: () => [] },
  initialIndex: { type: Number, default: 0 },
});

const naturalSize = reactive({ width: 0, height: 0 });
const viewport = reactive({ width: window.innerWidth, height: window.innerHeight });
const currentIndex = defineModel('index', { type: Number, default: 0 });
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

const imageStyle = computed(() => {
  const width = naturalSize.width;
  const height = naturalSize.height;
  if (!width || !height) return {};
  const fitScale = Math.min((viewport.width - 144) / width, (viewport.height - 116) / height);
  const scale = Math.min(1, fitScale);
  return {
    width: `${Math.max(1, Math.round(width * scale))}px`,
    height: `${Math.max(1, Math.round(height * scale))}px`,
  };
});

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
  },
  { deep: false }
);

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

function closeOnKey(event) {
  if (!show.value) return;
  if (event.key === 'Escape') show.value = false;
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
</script>
