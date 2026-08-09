<template>
  <n-modal
    v-model:show="show"
    :mask-closable="true"
    :close-on-esc="true"
    :auto-focus="false"
    class="design-modal"
    :style="{ width: '900px' }"
  >
    <div class="design-layout">
      <aside class="design-sidebar" aria-label="设置菜单">
        <button
          v-for="item in menuItems"
          :key="item.id"
          type="button"
          class="design-menu-item"
          :class="{ active: tab === item.id }"
          :aria-pressed="tab === item.id"
          @click="tab = item.id"
        >
          <AppIcon :src="item.icon" :size="17" />
          <span>{{ item.label }}</span>
        </button>
      </aside>

      <div class="design-content">
        <ApiSourcePanel
          v-if="tab === 'chat-api' || tab === 'image-api'"
          :show="show && (tab === 'chat-api' || tab === 'image-api')"
          :settings="settings"
          :kind="tab === 'chat-api' ? 'chat' : 'image'"
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
import { computed, ref } from "vue";
import aboutIcon from "../../assets/关于.svg";
import chatApiIcon from "../../assets/对话API.svg";
import imageApiIcon from "../../assets/绘图API.svg";
import templatesIcon from "../../assets/模板库.svg";
import AppIcon from "../snippets/AppIcon.vue";
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
  { id: "templates", label: "模板库", icon: templatesIcon },
  { id: "chat-api", label: "对话API", icon: chatApiIcon },
  { id: "image-api", label: "绘图API", icon: imageApiIcon },
  { id: "about", label: "关于", icon: aboutIcon },
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
