<template>
  <NativeDialog
    v-model:show="show"
    :title="title"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="false"
    class="notice-modal"
    @opened="focusActionButton"
    @cancel="close"
  >
    <div class="notice-dialog-shell" tabindex="-1">
      <p class="notice-dialog-message">{{ message }}</p>
      <div class="notice-dialog-actions">
        <button
          ref="actionButton"
          type="button"
          class="button button-small button-primary"
          @click="close"
        >
          {{ buttonText }}
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
  title: { type: String, default: '提示' },
  message: { type: String, required: true },
  buttonText: { type: String, default: '确认' },
});

const emit = defineEmits(['close']);
const actionButton = ref(null);

watch(show, (visible) => {
  if (visible) nextTick(focusActionButton);
});

function focusActionButton() {
  const button = actionButton.value;
  button?.focus?.();
}

function close() {
  show.value = false;
  emit('close');
}
</script>
