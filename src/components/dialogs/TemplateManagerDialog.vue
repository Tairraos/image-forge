<template>
  <n-modal v-model:show="show" preset="card" title="模板维护" class="template-manager-modal">
    <TemplateManagerPanel
      v-model:query="query"
      :templates="templates"
      @view="emit('view', $event)"
      @edit="emit('edit', $event)"
      @delete="emit('delete', $event)"
      @create="emit('create')"
      @import="emit('import')"
      @export="emit('export')"
      @move="emit('move', $event)"
      @show-effect="emit('show-effect', $event)"
    />
  </n-modal>
</template>

<script setup>
import TemplateManagerPanel from './TemplateManagerPanel.vue';

const show = defineModel('show', { type: Boolean, default: false });
const query = defineModel('query', { type: String, default: '' });

defineProps({
  templates: { type: Array, default: () => [] },
});

const emit = defineEmits([
  'create',
  'import',
  'export',
  'view',
  'edit',
  'delete',
  'move',
  'show-effect',
]);
</script>
