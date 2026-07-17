<template>
  <n-modal v-model:show="show" class="effect-image-viewer" :mask-closable="true">
    <div class="effect-image-viewer-stage" role="dialog" aria-label="效果图预览" @click="show = false">
      <img
        v-if="imagePath"
        :src="convertFileSrc(imagePath)"
        :alt="title || '模板效果图'"
        :style="imageStyle"
        @load="updateImageSize"
        @click.stop
      />
    </div>
  </n-modal>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, watch } from "vue";
import { convertFileSrc } from "../../tauri";

const show = defineModel("show", { type: Boolean, default: false });
const props = defineProps({
  imagePath: { type: String, default: "" },
  title: { type: String, default: "" },
});

const naturalSize = reactive({ width: 0, height: 0 });
const viewport = reactive({ width: window.innerWidth, height: window.innerHeight });

const imageStyle = computed(() => {
  const width = naturalSize.width;
  const height = naturalSize.height;
  if (!width || !height) return {};
  const desiredScale = 640 / Math.min(width, height);
  const fitScale = Math.min((viewport.width - 48) / width, (viewport.height - 48) / height);
  const scale = Math.min(desiredScale, fitScale);
  return {
    width: `${Math.max(1, Math.round(width * scale))}px`,
    height: `${Math.max(1, Math.round(height * scale))}px`,
  };
});

watch(() => props.imagePath, () => {
  naturalSize.width = 0;
  naturalSize.height = 0;
});

onMounted(() => {
  window.addEventListener("keydown", closeOnKey);
  window.addEventListener("resize", updateViewport);
});

onUnmounted(() => {
  window.removeEventListener("keydown", closeOnKey);
  window.removeEventListener("resize", updateViewport);
});

function updateImageSize(event) {
  naturalSize.width = event.target.naturalWidth;
  naturalSize.height = event.target.naturalHeight;
}

function closeOnKey() {
  if (show.value) show.value = false;
}

function updateViewport() {
  viewport.width = window.innerWidth;
  viewport.height = window.innerHeight;
}
</script>
