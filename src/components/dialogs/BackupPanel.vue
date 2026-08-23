<template>
  <div class="backup-panel">
    <p class="backup-summary">
      当前 app 共有 {{ templateCount }} 个模板，{{ chatApiCount }} 个对话 API 源，{{ imageApiCount }} 个绘图 API 源
    </p>
    <div class="backup-actions">
      <n-button
        class="backup-action-btn"
        size="large"
        type="primary"
        secondary
        @click="$emit('export-data')"
      >
        导出数据
      </n-button>
      <n-button
        class="backup-action-btn"
        size="large"
        type="primary"
        secondary
        @click="$emit('import-data')"
      >
        导入数据
      </n-button>
    </div>
    <p class="backup-note">导出为 ZIP 文件，可选择 API 配置、模板、对话和图片库。导入时自动合并。</p>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { isImageModelType } from "../../lib/models";

const props = defineProps({
  settings: { type: Object, required: true },
  templates: { type: Array, default: () => [] },
});

defineEmits(["export-data", "import-data"]);

const providers = computed(() =>
  Array.isArray(props.settings?.providers) ? props.settings.providers : [],
);

const templateCount = computed(() => props.templates.length);
const chatApiCount = computed(
  () => providers.value.filter((provider) => provider.modelType === "chat").length,
);
const imageApiCount = computed(
  () => providers.value.filter((provider) => isImageModelType(provider.modelType)).length,
);
</script>

<style scoped>
.backup-note {
  margin-top: 16px;
  color: #888;
  font-size: 12px;
  text-align: center;
}
</style>