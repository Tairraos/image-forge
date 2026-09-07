<template>
  <NativeDialog
    v-model:show="show"
    :title="title"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="false"
    class="confirm-modal"
    @opened="focusConfirmButton"
    @cancel="cancel"
  >
    <div class="confirm-dialog-shell" tabindex="-1">
      <p class="confirm-dialog-message">{{ message }}</p>
      <div class="confirm-dialog-actions">
        <button type="button" class="button button-small" @click="cancel">取消</button>
        <button
          ref="confirmButton"
          type="button"
          class="button button-small button-danger"
          @click="confirm"
        >
          确认
        </button>
      </div>
    </div>
  </NativeDialog>
</template>

<script setup>
import NativeDialog from './NativeDialog.vue';
import { nextTick, ref, watch } from 'vue';

const show = defineModel('show', { type: Boolean, default: false });

defineProps({
  title: { type: String, default: '请确认' },
  message: { type: String, required: true },
});

const emit = defineEmits(['confirm', 'cancel']);
const confirmButton = ref(null);

watch(show, (visible) => {
  if (visible) nextTick(focusConfirmButton);
});

function focusConfirmButton() {
  const button = confirmButton.value;
  button?.focus?.();
}

function confirm() {
  show.value = false;
  emit('confirm');
}

function cancel() {
  show.value = false;
  emit('cancel');
}
</script>
