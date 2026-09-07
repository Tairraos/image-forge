<template>
  <NativeDialog v-model:show="visible" title="API 源管理" class="api-modal">
    <ApiSourcePanel
      :show="visible"
      :settings="settings"
      @save="emit('save', $event)"
      @close="visible = false"
    />
  </NativeDialog>
</template>

<script setup>
import NativeDialog from './NativeDialog.vue';
import { computed } from 'vue';
import ApiSourcePanel from './ApiSourcePanel.vue';

const props = defineProps({
  show: { type: Boolean, default: false },
  settings: { type: Object, required: true },
});
const emit = defineEmits(['update:show', 'save']);
const visible = computed({
  get: () => props.show,
  set: (value) => emit('update:show', value),
});
</script>
