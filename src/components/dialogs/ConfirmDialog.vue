<template>
  <n-modal
    v-model:show="show"
    preset="card"
    :title="title"
    :closable="false"
    :mask-closable="false"
    :close-on-esc="false"
    :auto-focus="false"
    class="confirm-modal"
    @after-enter="focusConfirmButton"
  >
    <div
      class="confirm-dialog-shell"
      tabindex="-1"
      @keydown.enter.prevent.stop="confirm"
      @keydown.esc.prevent.stop="cancel"
    >
      <p class="confirm-dialog-message">{{ message }}</p>
      <div class="confirm-dialog-actions">
        <n-button size="small" @click="cancel">取消</n-button>
        <n-button ref="confirmButton" size="small" type="error" @click="confirm">确认</n-button>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { nextTick, ref, watch } from "vue";

const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  title: { type: String, default: "请确认" },
  message: { type: String, required: true },
});

const emit = defineEmits(["confirm", "cancel"]);
const confirmButton = ref(null);

watch(show, (visible) => {
  if (visible) nextTick(focusConfirmButton);
});

function focusConfirmButton() {
  const button = confirmButton.value;
  button?.focus?.();
  button?.$el?.focus?.();
}

function confirm() {
  show.value = false;
  emit("confirm");
}

function cancel() {
  show.value = false;
  emit("cancel");
}
</script>
