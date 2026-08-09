<template>
  <header class="topbar" data-tauri-drag-region="deep">
    <div class="brand">
      <img :src="logoUrl" alt="Image Forge" />
    </div>

    <n-button-group class="mode-switch" size="small" aria-label="工作区模式">
      <n-button
        :type="mode === 'drawing' ? 'primary' : 'default'"
        :aria-pressed="mode === 'drawing'"
        title="绘画模式（Ctrl/Cmd+1）"
        @click="$emit('update:mode', 'drawing')"
      >绘画</n-button>
      <n-button
        :type="mode === 'agent' ? 'primary' : 'default'"
        :aria-pressed="mode === 'agent'"
        title="Agent 模式（Ctrl/Cmd+2）"
        @click="$emit('update:mode', 'agent')"
      >Agent</n-button>
      <n-button
        :type="mode === 'library' ? 'primary' : 'default'"
        :aria-pressed="mode === 'library'"
        title="图片库（Ctrl/Cmd+3）"
        @click="$emit('update:mode', 'library')"
      >图片库</n-button>
    </n-button-group>

    <div class="topbar-actions">
      <n-button quaternary size="small" @click="$emit('show-design')">
        <template #icon><Palette :size="16" /></template>
        设计
      </n-button>
    </div>
  </header>
</template>

<script setup>
import { Palette } from "@lucide/vue";
import logoUrl from "../assets/title.png";

defineProps({ mode: { type: String, default: "drawing" } });
defineEmits(["update:mode", "show-design"]);
</script>
