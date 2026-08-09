<template>
  <span
    class="app-icon"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      backgroundColor: 'currentColor',
      WebkitMaskImage: `url(${src})`,
      maskImage: `url(${src})`,
    }"
    aria-hidden="true"
  />
</template>

<script setup>
defineProps({
  src: { type: String, required: true },
  size: { type: [Number, String], default: 16 },
});
</script>

<style scoped>
.app-icon {
  display: inline-block;
  flex-shrink: 0;
  background-color: currentColor;
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: contain;
  mask-size: contain;
}
</style>
