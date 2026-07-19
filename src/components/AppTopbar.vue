<template>
  <header class="topbar">
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
    </n-button-group>

    <div class="topbar-actions">
      <n-button quaternary size="small" @click="$emit('show-api')">
        <template #icon><Settings :size="16" /></template>
        API 源
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-template-manager')">
        <template #icon><BookOpen :size="16" /></template>
        模板
      </n-button>
      <n-button v-if="mode === 'agent'" quaternary size="small" @click="$emit('show-skill-manager')">
        <template #icon><FileText :size="16" /></template>
        Skill
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-about')">
        <template #icon><Info :size="16" /></template>
        关于
      </n-button>
    </div>
  </header>
</template>

<script setup>
import { BookOpen, FileText, Info, Settings } from "@lucide/vue";
import logoUrl from "../assets/title.png";

defineProps({ mode: { type: String, default: "drawing" } });
defineEmits(["update:mode", "show-api", "show-template-manager", "show-skill-manager", "show-about"]);
</script>
