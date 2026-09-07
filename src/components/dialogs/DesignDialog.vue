<template>
  <NativeDialog v-model:show="show" title="设置" frameless class="design-modal">
    <div class="design-shell">
      <aside class="design-side" aria-label="设置菜单">
        <div class="design-side-title">设置<span>工作空间偏好</span></div>
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
            <component :is="item.icon" :size="17" /><span>{{ item.label }}</span>
          </button>
        </nav>
        <div class="design-side-footer">
          <Aperture :size="17" /><span
            >Image Forge<small v-if="info.version">v{{ info.version }}</small></span
          >
        </div>
      </aside>
      <section class="design-main" :aria-label="currentTab.label">
        <header class="design-main-titlebar">
          <div>
            <h2>{{ currentTab.label }}</h2>
            <p>{{ currentTab.description }}</p>
          </div>
          <button
            type="button"
            class="icon-button design-close"
            title="关闭设置"
            aria-label="关闭设置"
            @click="show = false"
          >
            <X :size="19" />
          </button>
        </header>
        <div class="design-content">
          <ApiSourcePanel
            v-if="tab === 'chat-api' || tab === 'image-api'"
            :show="show"
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
          <section v-else-if="tab === 'appearance'" class="appearance-panel">
            <h3>界面主题</h3>
            <p class="section-description">浅色清爽，深色专注。也可以随系统自动切换。</p>
            <div class="theme-options" role="group" aria-label="界面主题">
              <button
                v-for="option in themeOptions"
                :key="option.value"
                type="button"
                class="theme-option"
                :class="{ selected: theme === option.value }"
                :aria-pressed="theme === option.value"
                @click="theme = option.value"
              >
                <span class="theme-preview" :data-appearance="option.value"
                  ><span class="theme-preview-sidebar"><i></i><i></i><i></i></span
                  ><span class="theme-preview-main"><i></i><i></i><span></span></span
                ></span>
                <span class="theme-option-label"
                  ><component :is="option.icon" :size="16" />{{ option.label
                  }}<Check v-if="theme === option.value" :size="16"
                /></span>
              </button>
            </div>
            <div class="appearance-note">
              <Monitor :size="18" />
              <p>主题应用于整个工作空间，并自动记住你的选择。</p>
            </div>
          </section>
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
  </NativeDialog>
</template>

<script setup>
import NativeDialog from './NativeDialog.vue';
import { computed, ref } from 'vue';
import {
  Aperture,
  Check,
  Images,
  LayoutTemplate,
  MessageSquare,
  Monitor,
  Moon,
  Sun,
  X,
} from '@lucide/vue';
import AboutPanel from './AboutPanel.vue';
import ApiSourcePanel from './ApiSourcePanel.vue';
import TemplateManagerPanel from './TemplateManagerPanel.vue';

const show = defineModel('show', { type: Boolean, default: false });
const theme = defineModel('theme', { type: String, default: 'system' });

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
  {
    id: 'templates',
    label: '模板库',
    icon: LayoutTemplate,
    description: '收藏常用提示词，让灵感随时可用。',
  },
  {
    id: 'chat-api',
    label: '对话 API',
    icon: MessageSquare,
    description: '连接对话模型，一起构思、规划和创作。',
  },
  {
    id: 'image-api',
    label: '绘图 API',
    icon: Images,
    description: '管理绘图服务与模型，把想法变成画面。',
  },
  { id: 'appearance', label: '外观', icon: Sun, description: '选择适合你的工作空间。' },
  {
    id: 'about',
    label: '关于与数据',
    icon: Aperture,
    description: '应用信息、数据备份与存储管理。',
  },
];
const themeOptions = [
  { value: 'light', label: '浅色', icon: Sun },
  { value: 'dark', label: '深色', icon: Moon },
  { value: 'system', label: '跟随系统', icon: Monitor },
];

const tab = ref('templates');
const currentTab = computed(() => menuItems.find((item) => item.id === tab.value));
const templateQuery = ref('');

const filteredTemplates = computed(() => {
  const query = templateQuery.value.trim().toLowerCase();
  if (!query) return props.templates;
  return props.templates.filter((item) =>
    [item.id, item.title, item.content].filter(Boolean).join(' ').toLowerCase().includes(query)
  );
});
</script>
