<template>
  <n-modal
    v-model:show="show"
    preset="card"
    title="设置"
    class="design-modal"
    :style="{ width: 'min(1080px, calc(100vw - 48px))' }"
  >
    <div class="design-layout">
      <aside class="design-sidebar" aria-label="设计菜单">
        <button
          v-for="item in menuItems"
          :key="item.id"
          type="button"
          class="design-menu-item"
          :class="{ active: tab === item.id }"
          :aria-pressed="tab === item.id"
          @click="tab = item.id"
        >
          <component :is="item.icon" :size="17" />
          <span>{{ item.label }}</span>
        </button>
      </aside>

      <div class="design-content">
        <ApiSourcePanel
          v-if="tab === 'api'"
          :show="show && tab === 'api'"
          :settings="settings"
          @save="emit('save-api', $event)"
          @close="show = false"
        />
        <div v-else-if="tab === 'templates'" class="design-templates">
          <TemplateManagerPanel
            v-model:query="templateQuery"
            :templates="filteredTemplates"
            @view="emit('view-template', $event)"
            @edit="emit('edit-template', $event)"
            @delete="emit('delete-template', $event)"
            @create="emit('create-template')"
            @import="emit('import-template')"
            @export="emit('export-template')"
            @move="emit('move-template', $event)"
            @show-effect="emit('show-template-effect', $event)"
          />
        </div>
        <AboutPanel
          v-else
          :info="info"
          @show-logs="emit('show-logs')"
          @cleanup="emit('cleanup')"
          @close="show = false"
        />
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { BookOpen, Info, Settings } from "@lucide/vue";
import { computed, ref } from "vue";
import AboutPanel from "./AboutPanel.vue";
import ApiSourcePanel from "./ApiSourcePanel.vue";
import TemplateManagerPanel from "./TemplateManagerPanel.vue";

const show = defineModel("show", { type: Boolean, default: false });

const props = defineProps({
  settings: { type: Object, required: true },
  templates: { type: Array, default: () => [] },
  info: { type: Object, default: () => ({}) },
});

const emit = defineEmits([
  "save-api",
  "view-template",
  "edit-template",
  "delete-template",
  "create-template",
  "import-template",
  "export-template",
  "move-template",
  "show-template-effect",
  "show-logs",
  "cleanup",
]);

const menuItems = [
  { id: "templates", label: "模板库", icon: BookOpen },
  { id: "api", label: "API 源", icon: Settings },
  { id: "about", label: "关于", icon: Info },
];

const tab = ref("templates");
const templateQuery = ref("");

const filteredTemplates = computed(() => {
  const query = templateQuery.value.trim().toLowerCase();
  if (!query) return props.templates;
  return props.templates.filter((item) =>
    [item.id, item.title, item.content].filter(Boolean).join(" ").toLowerCase().includes(query),
  );
});
</script>
