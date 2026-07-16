<template>
  <header class="topbar">
    <div class="brand">
      <img :src="logoUrl" alt="Image Forge" />
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
      <n-button quaternary size="small" @click="$emit('show-skill-manager')">
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

defineEmits(["show-api", "show-template-manager", "show-skill-manager", "show-about"]);
</script>
