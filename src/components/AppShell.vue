<template>
  <main class="app">
    <AppTopbar
      :mode="mode"
      @update:mode="$emit('update:mode', $event)"
      @show-api="$emit('show-api')"
      @show-template-manager="$emit('show-template-manager')"
      @show-skill-manager="$emit('show-skill-manager')"
      @show-about="$emit('show-about')"
    />

    <div class="app-shell-workspace">
      <slot />
    </div>

    <slot name="footer" />
    <slot name="dialogs" />
  </main>
</template>

<script setup>
import AppTopbar from "./AppTopbar.vue";

defineProps({
  mode: { type: String, default: "drawing" },
});

defineEmits([
  "update:mode",
  "show-api",
  "show-template-manager",
  "show-skill-manager",
  "show-about",
]);
</script>
