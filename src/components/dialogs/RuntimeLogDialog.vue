<template>
  <n-modal v-model:show="show" preset="card" title="运行日志" class="runtime-log-modal">
    <n-input
      :value="logs || '本次运行还没有日志。'"
      type="textarea"
      readonly
      class="runtime-log-input"
      :autosize="{ minRows: 12, maxRows: 12 }"
    />

    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" @click="show = false">关闭</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  logs: { type: String, default: "" },
});
</script>
