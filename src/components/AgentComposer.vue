<template>
  <div
    class="agent-composer"
    :class="{ 'reference-drop-active': dragActive }"
    @dragover.prevent="dragActive = true"
    @dragleave="dragActive = false"
    @drop.prevent="dropFiles"
  >
    <div v-if="attachments.length" class="agent-attachments">
      <div v-for="attachment in attachments" :key="attachment.id" class="agent-attachment">
        <img :src="attachment.dataUrl" :alt="attachment.fileName" />
        <button type="button" aria-label="移除参考图" @click="$emit('remove-attachment', attachment.id)">×</button>
      </div>
    </div>
    <n-input
      v-model:value="draft"
      type="textarea"
      :autosize="{ minRows: 3, maxRows: 8 }"
      placeholder="输入消息；可粘贴或拖入参考图"
      :disabled="busy"
      @paste="$emit('paste-reference', $event)"
      @keydown.meta.enter.prevent="send"
      @keydown.ctrl.enter.prevent="send"
    />
    <footer>
      <n-button size="small" :disabled="busy" @click="$emit('add-reference')">添加参考图</n-button>
      <n-button v-if="busy" size="small" type="error" secondary @click="$emit('stop')">停止</n-button>
      <n-button v-else size="small" type="primary" :disabled="!draft.trim() || !providerId" @click="send">发送</n-button>
    </footer>
  </div>
</template>

<script setup>
import { ref } from "vue";
import { extractDroppedFilePaths } from "../lib/referenceFiles";

const props = defineProps({
  providerId: { type: String, default: "" },
  busy: Boolean,
  attachments: { type: Array, default: () => [] },
});
const emit = defineEmits(["send", "stop", "add-reference", "paste-reference", "drop-reference", "remove-attachment"]);
const draft = ref("");
const dragActive = ref(false);

function send() {
  const content = draft.value.trim();
  if (!content || props.busy || !props.providerId) return;
  emit("send", content);
  draft.value = "";
}

function dropFiles(event) {
  dragActive.value = false;
  emit("drop-reference", extractDroppedFilePaths(event.dataTransfer));
}
</script>
