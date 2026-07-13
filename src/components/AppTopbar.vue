<template>
  <header class="topbar">
    <div class="brand">
      <img :src="logoUrl" alt="Image Forge" />
      <div>
        <h1>Image Forge</h1>
        <div class="brand-path-row">
          <p>{{ outputDir || "加载输出目录" }}</p>
          <n-button
            size="tiny"
            secondary
            :disabled="!outputDir"
            @click="$emit('reveal-output-dir')"
          >
            定位
          </n-button>
        </div>
      </div>
    </div>

    <div class="topbar-center">
      <label class="model-select-field">
        <span>生图模型</span>
        <n-select
          v-model:value="form.providerId"
          :options="imageProviderOptions"
          size="small"
          class="provider-select"
          placeholder="选择生图模型"
        />
      </label>
      <label class="model-select-field">
        <span>对话模型</span>
        <n-select
          v-model:value="form.chatProviderId"
          :options="chatProviderOptions"
          size="small"
          class="provider-select"
          placeholder="选择对话模型"
          :disabled="!chatProviderOptions.length"
        />
      </label>
      <n-tag size="small" :type="queue.running.length ? 'warning' : 'success'">
        {{ queue.running.length }} 运行 · {{ queue.waiting.length }} 排队
      </n-tag>
    </div>

    <div class="topbar-actions">
      <n-button quaternary size="small" @click="$emit('show-api')">
        <template #icon><Settings :size="16" /></template>
        API 源
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-gallery')">
        <template #icon><Image :size="16" /></template>
        图库
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-template')">
        <template #icon><BookOpen :size="16" /></template>
        模板
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-snippet')">
        <template #icon><Layers :size="16" /></template>
        片段
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-settings')">
        <template #icon><SlidersHorizontal :size="16" /></template>
        设置
      </n-button>
    </div>
  </header>
</template>

<script setup>
import { BookOpen, Image, Layers, Settings, SlidersHorizontal } from "@lucide/vue";
import logoUrl from "../assets/app-icon.png";

defineProps({
  outputDir: { type: String, default: "" },
  form: { type: Object, required: true },
  imageProviderOptions: { type: Array, default: () => [] },
  chatProviderOptions: { type: Array, default: () => [] },
  queue: { type: Object, required: true },
});

defineEmits([
  "reveal-output-dir",
  "show-api",
  "show-gallery",
  "show-template",
  "show-snippet",
  "show-settings",
]);
</script>
