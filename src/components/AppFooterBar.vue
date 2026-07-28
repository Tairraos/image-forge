<template>
  <footer class="status-bar">
    <span class="status-pill" :data-tone="statusTone">{{ statusText }}</span>
    <div class="status-summary">
      <span class="status-meta">
        当前 API：
        <n-popselect
          :value="imageProviderId"
          :options="imageProviderOptions"
          placement="top-start"
          trigger="click"
          @update:value="$emit('select-image-provider', $event)"
        >
          <button class="status-api-name" type="button">{{ imageProviderName || "未配置" }}</button>
        </n-popselect>
        <span class="status-api-separator">/</span>
        <n-popselect
          :value="chatProviderId"
          :options="chatProviderOptions"
          placement="top-start"
          trigger="click"
          @update:value="$emit('select-chat-provider', $event)"
        >
          <button class="status-api-name" type="button">{{ chatProviderName || "未配置" }}</button>
        </n-popselect>
      </span>
      <span class="status-count">{{ runningCount }} 运行</span>
      <span class="status-count">{{ waitingCount }} 排队</span>
      <span v-if="imageProviderMissingKey" class="warn-text">API Key 未设置</span>
    </div>
  </footer>
</template>

<script setup>
defineProps({
  statusText: { type: String, default: "" },
  statusTone: { type: String, default: "" },
  imageProviderId: { type: String, default: "" },
  imageProviderName: { type: String, default: "" },
  imageProviderOptions: { type: Array, default: () => [] },
  chatProviderId: { type: String, default: "" },
  chatProviderName: { type: String, default: "" },
  chatProviderOptions: { type: Array, default: () => [] },
  runningCount: { type: Number, default: 0 },
  waitingCount: { type: Number, default: 0 },
  imageProviderMissingKey: Boolean,
});

defineEmits(["select-image-provider", "select-chat-provider"]);
</script>
