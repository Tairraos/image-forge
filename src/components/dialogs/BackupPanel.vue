<template>
  <div class="backup-panel">
    <p class="backup-summary">
      当前 app 共有 {{ templateCount }} 个模板，{{ chatApiCount }} 个对话 API 源，{{ imageApiCount }} 个绘图 API 源
    </p>
    <div class="backup-actions">
      <n-button class="backup-action-btn" size="large" type="primary" secondary>
        备份
      </n-button>
      <n-button class="backup-action-btn" size="large" type="primary" secondary>
        恢复
      </n-button>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { isImageModelType } from "../../lib/models";

const props = defineProps({
  settings: { type: Object, required: true },
  templates: { type: Array, default: () => [] },
});

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
