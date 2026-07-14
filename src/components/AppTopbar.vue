<template>
  <header class="topbar">
    <div class="brand">
      <img :src="logoUrl" alt="Image Forge" />
      <div>
        <h1>Image Forge</h1>
      </div>
    </div>

    <div class="topbar-actions">
      <n-button quaternary size="small" @click="$emit('show-api')">
        <template #icon><Settings :size="16" /></template>
        API 源
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-template-manager')">
        <template #icon><BookOpen :size="16" /></template>
        模板
      </n-button>
      <n-button quaternary size="small" @click="$emit('show-settings')">
        <template #icon><SlidersHorizontal :size="16" /></template>
        设置
      </n-button>
    </div>
  </header>
</template>

<script setup>
import { BookOpen, Settings, SlidersHorizontal } from "@lucide/vue";
import logoUrl from "../assets/app-icon.png";

defineEmits([
  "show-api",
  "show-template-manager",
  "show-settings",
]);
</script>
