<template>
  <n-dropdown
    trigger="manual"
    placement="bottom-start"
    :show="show"
    :disabled="disabled"
    :options="options"
    @update:show="show = $event"
    @select="handleSelect"
    @clickoutside="show = false"
  >
    <span ref="triggerRef" class="clipboard-image-menu-trigger">
      <slot :open="openMenu" />
    </span>
  </n-dropdown>
</template>

<script setup>
import { ClipboardPaste } from "@lucide/vue";
import { h, nextTick, onBeforeUnmount, onMounted, ref } from "vue";

const props = defineProps({
  disabled: { type: Boolean, default: false },
});

const emit = defineEmits(["paste"]);
const show = ref(false);
const triggerRef = ref(null);
const options = [
  {
    label: "粘贴剪贴板图片",
    key: "paste",
    icon: () => h(ClipboardPaste, { size: 16 }),
  },
];

function openMenu(event) {
  if (props.disabled) return;
  event.preventDefault();
  event.stopPropagation();
  show.value = false;
  nextTick(() => {
    show.value = true;
  });
}

function handleSelect(key) {
  show.value = false;
  if (key === "paste") emit("paste");
}

function handleDocumentPointerDown(event) {
  if (!show.value) return;
  const target = event.target;
  if (!(target instanceof Element)) {
    show.value = false;
    return;
  }
  if (target.closest(".n-dropdown")) return;
  if (triggerRef.value?.contains(target) && event.button !== 0) return;
  show.value = false;
}

function handleDocumentKeydown(event) {
  if (event.key === "Escape") show.value = false;
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown, true);
  document.addEventListener("keydown", handleDocumentKeydown, true);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown, true);
  document.removeEventListener("keydown", handleDocumentKeydown, true);
});
</script>
