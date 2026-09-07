<template>
  <span ref="triggerRef" class="clipboard-image-menu-trigger">
    <slot :open="openMenu" />
    <span
      v-if="show"
      ref="menuRef"
      class="clipboard-menu"
      role="menu"
      :style="position"
      @keydown.esc.prevent.stop="closeMenu"
    >
      <button ref="pasteButton" type="button" role="menuitem" @click="paste">
        <ClipboardPaste :size="16" />粘贴剪贴板图片
      </button>
    </span>
  </span>
</template>

<script setup>
import { ClipboardPaste } from '@lucide/vue';
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue';

const props = defineProps({ disabled: { type: Boolean, default: false } });
const emit = defineEmits(['paste']);
const show = ref(false);
const triggerRef = ref(null);
const menuRef = ref(null);
const pasteButton = ref(null);
const position = ref({});

async function openMenu(event) {
  if (props.disabled) return;
  event.preventDefault();
  event.stopPropagation();
  const bounds = triggerRef.value.getBoundingClientRect();
  position.value = {
    left: `${Math.max(8, Math.min(event.clientX || bounds.left, window.innerWidth - 228))}px`,
    top: `${Math.max(8, Math.min(event.clientY || bounds.top, window.innerHeight - 56))}px`,
  };
  show.value = true;
  await nextTick();
  pasteButton.value?.focus();
}

function closeMenu() {
  show.value = false;
  triggerRef.value?.querySelector('button')?.focus();
}

function paste() {
  closeMenu();
  emit('paste');
}

function closeOutside(event) {
  if (show.value && !menuRef.value?.contains(event.target)) show.value = false;
}

onMounted(() => document.addEventListener('pointerdown', closeOutside, true));
onBeforeUnmount(() => document.removeEventListener('pointerdown', closeOutside, true));
</script>
