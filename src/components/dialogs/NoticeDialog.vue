<template>
  <n-modal
    v-model:show="show"
    preset="card"
    :title="title"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="false"
    :auto-focus="false"
    class="notice-modal"
    @after-enter="focusActionButton"
  >
    <div
      class="notice-dialog-shell"
      tabindex="-1"
      @keydown.enter.prevent.stop="close"
      @keydown.esc.prevent.stop="close"
    >
      <p class="notice-dialog-message">{{ message }}</p>
      <div class="notice-dialog-actions">
        <n-button ref="actionButton" size="small" type="primary" @click="close">
          {{ buttonText }}
        </n-button>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { nextTick, ref, watch } from "vue";

const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  title: { type: String, default: "提示" },
  message: { type: String, required: true },
  buttonText: { type: String, default: "确认" },
});

const emit = defineEmits(["close"]);
const actionButton = ref(null);

watch(show, (visible) => {
  if (visible) nextTick(focusActionButton);
});

function focusActionButton() {
  const button = actionButton.value;
  button?.focus?.();
  button?.$el?.focus?.();
}

function close() {
  show.value = false;
  emit("close");
}
</script>
