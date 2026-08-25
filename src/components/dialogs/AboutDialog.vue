<template>
  <n-modal v-model:show="show" preset="card" title="关于 Image Forge" class="about-modal">
    <AboutPanel
      :info="info"
      @show-logs="emit('show-logs')"
      @cleanup="emit('cleanup')"
      @close="show = false"
    />
  </n-modal>
</template>

<script setup>
import AboutPanel from './AboutPanel.vue';

const show = defineModel('show', { type: Boolean, default: false });

defineProps({
  info: {
    type: Object,
    default: () => ({ version: '', buildTime: '' }),
  },
});

const emit = defineEmits(['show-logs', 'cleanup']);
</script>
