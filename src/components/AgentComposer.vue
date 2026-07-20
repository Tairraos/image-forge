<template>
  <div
    class="agent-composer"
    :class="{ 'reference-drop-active': dragActive }"
    @dragover.prevent="dragActive = true"
    @dragleave="dragActive = false"
    @drop.prevent="dropFiles"
  >
    <div class="agent-composer-body">
      <n-input
        v-model:value="draft"
        type="textarea"
        :autosize="{ minRows: 2, maxRows: 5 }"
        placeholder="输入消息；可粘贴或拖入参考图"
        :disabled="busy"
        @paste="$emit('paste-reference', $event)"
        @keydown="handleKeydown"
      />
    </div>
    <footer>
      <div class="agent-composer-actions">
        <div class="reference-strip agent-reference-strip">
          <div v-for="attachment in attachments" :key="attachment.id" class="reference-tile">
            <img :src="attachment.dataUrl" :alt="attachment.fileName" />
            <button
              type="button"
              title="移除参考图"
              aria-label="移除参考图"
              @click.stop="$emit('remove-attachment', attachment.id)"
            >
              <X :size="14" />
            </button>
          </div>
          <ClipboardImageMenu :disabled="busy" v-slot="{ open }" @paste="$emit('paste-reference', $event)">
            <button
              class="reference-add"
              :class="{ 'reference-drop-active': dragActive }"
              type="button"
              title="点击添加，右键粘贴剪贴板图片"
              :disabled="busy"
              @click="$emit('add-reference')"
              @contextmenu="open"
            >
              <Plus :size="18" />
              <span>参考图</span>
            </button>
          </ClipboardImageMenu>
        </div>
      </div>
      <div class="agent-send-stack">
        <n-button v-if="busy" size="small" type="error" secondary @click="$emit('stop')">停止</n-button>
        <n-button
          v-else
          class="agent-send-button"
          size="small"
          type="primary"
          :disabled="!draft.trim() || (drawThisTurn ? !imageProviderId : !providerId)"
          @click="send"
        >发送</n-button>
        <n-checkbox v-model:checked="drawThisTurn" :disabled="busy">直接绘画</n-checkbox>
      </div>
    </footer>
  </div>
</template>

<script setup>
import { Plus, X } from "@lucide/vue";
import { ref } from "vue";
import ClipboardImageMenu from "./ClipboardImageMenu.vue";
import { extractDroppedFilePaths } from "../lib/referenceFiles";

const props = defineProps({
  providerId: { type: String, default: "" },
  imageProviderId: { type: String, default: "" },
  busy: Boolean,
  attachments: { type: Array, default: () => [] },
});
const emit = defineEmits(["send", "stop", "add-reference", "paste-reference", "drop-reference", "remove-attachment"]);
const draft = ref("");
const dragActive = ref(false);
const drawThisTurn = ref(false);

function send() {
  const content = draft.value.trim();
  const providerId = drawThisTurn.value ? props.imageProviderId : props.providerId;
  if (!content || props.busy || !providerId) return;
  emit("send", { content, drawThisTurn: drawThisTurn.value });
  draft.value = "";
  drawThisTurn.value = false;
}

function handleKeydown(event) {
  if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  send();
}

function dropFiles(event) {
  dragActive.value = false;
  emit("drop-reference", extractDroppedFilePaths(event.dataTransfer));
}
</script>
