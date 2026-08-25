<template>
  <span
    v-if="svg"
    class="app-icon"
    :style="{ width: `${size}px`, height: `${size}px` }"
    aria-hidden="true"
    v-html="svg"
  />
  <span
    v-else
    class="app-icon"
    :style="{ width: `${size}px`, height: `${size}px` }"
    aria-hidden="true"
  >
    <img v-if="src" :src="src" alt="" />
  </span>
</template>

<script setup>
import { computed } from 'vue';

const props = defineProps({
  src: { type: String, default: '' },
  raw: { type: String, default: '' },
  size: { type: [Number, String], default: 16 },
});

const uid = `appicon-${Math.random().toString(36).slice(2, 9)}`;

const svg = computed(() => {
  const content = String(props.raw || '').trim();
  if (!content) return '';
  return content
    .replace(/fill="#000000"/gi, 'fill="currentColor"')
    .replace(/stroke="#000000"/gi, 'stroke="currentColor"')
    .replace(/\sid="([^"]+)"/g, (_m, id) => ` id="${uid}-${id}"`)
    .replace(/url\(#([^)]+)\)/g, (_m, id) => `url(#${uid}-${id})`)
    .replace(/\swidth="[^"]*"/i, '')
    .replace(/\sheight="[^"]*"/i, '')
    .replace('<svg', '<svg width="100%" height="100%"');
});
</script>

<style scoped>
.app-icon {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  color: inherit;
  line-height: 0;
}

.app-icon :deep(svg),
.app-icon img {
  width: 100%;
  height: 100%;
  display: block;
}

.app-icon img {
  object-fit: contain;
}
</style>
