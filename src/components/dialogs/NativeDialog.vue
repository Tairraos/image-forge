<template>
  <dialog
    ref="dialog"
    class="app-dialog"
    :class="{ 'dialog-frameless': frameless }"
    :aria-labelledby="!frameless && title ? titleId : undefined"
    :aria-label="frameless ? title : undefined"
    @cancel.prevent="cancel"
    @close="onClose"
    @pointerdown="backdropPressed = $event.target === dialog"
    @click="closeFromBackdrop"
  >
    <template v-if="show">
      <header v-if="!frameless" class="dialog-header">
        <h2 :id="titleId">{{ title }}</h2>
        <button
          v-if="closable"
          type="button"
          class="icon-button dialog-close"
          aria-label="关闭"
          title="关闭"
          @click="show = false"
        >
          <X :size="18" />
        </button>
      </header>
      <div class="dialog-body"><slot /></div>
      <footer v-if="$slots.footer" class="dialog-footer"><slot name="footer" /></footer>
    </template>
  </dialog>
</template>

<script setup>
import { X } from '@lucide/vue';
import { onBeforeUnmount, onMounted, ref, useId, watch } from 'vue';

const show = defineModel('show', { type: Boolean, default: false });
const props = defineProps({
  title: { type: String, default: '' },
  frameless: Boolean,
  closable: { type: Boolean, default: true },
  maskClosable: { type: Boolean, default: true },
  closeOnEsc: { type: Boolean, default: true },
});
const emit = defineEmits(['opened', 'cancel']);
const dialog = ref(null);
const titleId = useId();
let backdropPressed = false;

function syncDialog() {
  if (show.value && !dialog.value.open) {
    dialog.value.showModal();
    emit('opened');
  } else if (!show.value && dialog.value.open) {
    dialog.value.close();
  }
}

function cancel() {
  emit('cancel');
  if (props.closeOnEsc) show.value = false;
}

function onClose() {
  if (!dialog.value.open) show.value = false;
}

function closeFromBackdrop(event) {
  if (!backdropPressed || !props.maskClosable || event.target !== dialog.value) return;
  const bounds = dialog.value.getBoundingClientRect();
  if (
    event.clientX < bounds.left ||
    event.clientX > bounds.right ||
    event.clientY < bounds.top ||
    event.clientY > bounds.bottom
  ) {
    show.value = false;
  }
}

onMounted(syncDialog);
watch(show, syncDialog, { flush: 'post' });
onBeforeUnmount(() => dialog.value?.close());
</script>
