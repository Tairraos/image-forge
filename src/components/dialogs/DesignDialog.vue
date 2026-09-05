<template>
  <n-modal
    v-model:show="show"
    :mask-closable="true"
    :close-on-esc="true"
    :auto-focus="false"
    :closable="false"
    transform-origin="center"
    class="design-modal"
    :style="{ width: '900px' }"
  >
    <div class="design-shell">
      <aside class="design-side" aria-label="设置菜单">
        <div class="design-side-title">设置</div>
        <nav class="design-side-nav">
          <button
            v-for="item in menuItems"
            :key="item.id"
            type="button"
            class="design-menu-item"
            :class="{ active: tab === item.id }"
            :aria-pressed="tab === item.id"
            @click="tab = item.id"
          >
            <AppIcon :raw="item.icon" :size="17" />
            <span>{{ item.label }}</span>
          </button>
        </nav>
      </aside>

      <section class="design-main">
        <header class="design-main-titlebar">
          <button
            type="button"
            class="design-close"
            title="关闭"
            aria-label="关闭"
            @click="show = false"
          >
            ×
          </button>
        </header>

        <div class="design-content">
          <ApiSourcePanel
            v-if="tab === 'chat-api' || tab === 'image-api'"
            :show="show && (tab === 'chat-api' || tab === 'image-api')"
            :settings="settings"
            :kind="tab === 'chat-api' ? 'chat' : 'image'"
            @save="emit('save-api', $event)"
          />
          <div v-else-if="tab === 'templates'" class="design-templates">
            <TemplateManagerPanel
              v-model:query="templateQuery"
              :templates="filteredTemplates"
              @edit="emit('edit-template', $event)"
              @delete="emit('delete-template', $event)"
              @create="emit('create-template')"
              @import="emit('import-template')"
              @export="emit('export-template')"
              @move="emit('move-template', $event)"
              @show-effect="emit('show-template-effect', $event)"
              @show-image="emit('show-template-image', $event)"
            />
          </div>
          <AboutPanel
            v-else
            :info="info"
            :stats="stats"
            @export-data="emit('export-data')"
            @import-data="emit('import-data')"
            @cleanup="emit('cleanup')"
          />
        </div>
      </section>
    </div>
  </n-modal>
</template>

<script setup>
import { computed, ref } from 'vue';
import aboutIcon from '../../assets/关于.svg?raw';
import chatApiIcon from '../../assets/对话API.svg?raw';
import imageApiIcon from '../../assets/绘图API.svg?raw';
import templatesIcon from '../../assets/模板库.svg?raw';
import AppIcon from '../snippets/AppIcon.vue';
import AboutPanel from './AboutPanel.vue';
import ApiSourcePanel from './ApiSourcePanel.vue';
import TemplateManagerPanel from './TemplateManagerPanel.vue';

const show = defineModel('show', { type: Boolean, default: false });

const props = defineProps({
  settings: { type: Object, required: true },
  templates: { type: Array, default: () => [] },
  info: { type: Object, default: () => ({}) },
  stats: { type: Object, default: () => ({ images: 0, sessions: 0, providers: 0 }) },
});

const emit = defineEmits([
  'save-api',
  'edit-template',
  'delete-template',
  'create-template',
  'import-template',
  'export-template',
  'export-data',
  'import-data',
  'move-template',
  'show-template-effect',
  'show-template-image',
  'cleanup',
]);

const menuItems = [
  { id: 'templates', label: '模板库', icon: templatesIcon },
  { id: 'chat-api', label: '对话API', icon: chatApiIcon },
  { id: 'image-api', label: '绘图API', icon: imageApiIcon },
  { id: 'about', label: '关于', icon: aboutIcon },
];

const tab = ref('templates');
const templateQuery = ref('');

const filteredTemplates = computed(() => {
  const query = templateQuery.value.trim().toLowerCase();
  if (!query) return props.templates;
  return props.templates.filter((item) =>
    [item.id, item.title, item.content].filter(Boolean).join(' ').toLowerCase().includes(query)
  );
});
</script>
