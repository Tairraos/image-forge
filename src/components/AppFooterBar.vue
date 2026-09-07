<template>
  <footer class="status-bar">
    <span class="status-pill" :data-tone="statusTone" role="status"
      ><span class="status-dot"></span>{{ statusText }}</span
    >
    <div class="status-summary">
      <label class="status-model"
        ><Images :size="13" /><select
          class="status-api-name"
          aria-label="绘图模型"
          :value="imageProviderId"
          :title="imageProviderName"
          :disabled="!imageProviderOptions.length"
          @change="$emit('select-image-provider', $event.target.value)"
        >
          <option v-if="!imageProviderOptions.length" value="">未配置绘图 API</option>
          <option v-for="option in imageProviderOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select></label
      >
      <span class="status-api-separator">/</span>
      <label class="status-model"
        ><MessageSquare :size="13" /><select
          class="status-api-name"
          aria-label="对话模型"
          :value="chatProviderId"
          :title="chatProviderName"
          :disabled="!chatProviderOptions.length"
          @change="$emit('select-chat-provider', $event.target.value)"
        >
          <option v-if="!chatProviderOptions.length" value="">未配置对话 API</option>
          <option v-for="option in chatProviderOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select></label
      >
      <span class="status-count">{{ runningCount }} 运行<span>·</span>{{ waitingCount }} 排队</span>
      <span v-if="imageProviderMissingKey" class="warn-text">API Key 未设置</span>
    </div>
  </footer>
</template>

<script setup>
import { Images, MessageSquare } from '@lucide/vue';

defineProps({
  statusText: { type: String, default: '' },
  statusTone: { type: String, default: '' },
  imageProviderId: { type: String, default: '' },
  imageProviderName: { type: String, default: '' },
  imageProviderOptions: { type: Array, default: () => [] },
  chatProviderId: { type: String, default: '' },
  chatProviderName: { type: String, default: '' },
  chatProviderOptions: { type: Array, default: () => [] },
  runningCount: { type: Number, default: 0 },
  waitingCount: { type: Number, default: 0 },
  imageProviderMissingKey: Boolean,
});

defineEmits(['select-image-provider', 'select-chat-provider']);
</script>
