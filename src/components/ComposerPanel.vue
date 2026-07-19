<template>
  <section class="composer-column">
    <div class="control-surface">
      <n-form label-placement="top" :show-feedback="false">
        <div class="control-grid">
          <n-form-item label="提示词模式">
            <n-select v-model:value="form.promptMode" :options="promptModeOptions" size="small" />
          </n-form-item>
          <n-form-item label="分辨率">
            <n-select v-model:value="form.resolution" :options="resolutionOptions" size="small" />
          </n-form-item>
          <n-form-item label="比例">
            <n-select v-model:value="form.ratio" :options="ratioOptions" size="small" />
          </n-form-item>
          <n-form-item label="质量">
            <n-select v-model:value="form.quality" :options="qualityOptions" size="small" />
          </n-form-item>
          <n-form-item label="生图模型" class="model-form-item">
            <n-select
              v-model:value="form.providerId"
              :options="imageProviderOptions"
              :consistent-menu-width="false"
              :menu-props="modelSelectMenuProps"
              size="small"
              placeholder="选择生图模型"
            />
          </n-form-item>
          <n-form-item class="model-form-item">
            <template #label>
              <span class="model-form-label">
                对话模型
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <CircleAlert :size="14" class="model-help-icon" />
                  </template>
                  对话模型用于执行 skill
                </n-tooltip>
              </span>
            </template>
            <n-select
              v-model:value="form.chatProviderId"
              :options="chatProviderOptions"
              :consistent-menu-width="false"
              :menu-props="modelSelectMenuProps"
              size="small"
              placeholder="选择对话模型"
            />
          </n-form-item>
        </div>
      </n-form>
    </div>

    <div class="reference-strip">
      <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
        <img :src="item.previewUrl" :alt="item.fileName" />
        <button
          type="button"
          title="移除参考图"
          @click.stop="$emit('remove-reference', index)"
        >
          <X :size="14" />
        </button>
      </div>
      <ClipboardImageMenu v-slot="{ open }" @paste="$emit('paste-reference')">
        <button
          class="reference-add"
          :class="{ 'reference-drop-active': referenceDragActive }"
          data-reference-drop-target="workbench"
          type="button"
          title="点击添加，右键粘贴剪贴板图片"
          @dragover.prevent="$emit('reference-drag-over')"
          @dragleave="$emit('reference-drag-leave')"
          @drop.prevent="$emit('drop-reference', $event)"
          @click="$emit('add-reference')"
          @contextmenu="open"
        >
          <Plus :size="18" />
          <span>参考图</span>
        </button>
      </ClipboardImageMenu>
    </div>

    <div
      class="prompt-live-panel"
      :class="{ 'reference-drop-active': referenceDragActive }"
      data-reference-drop-target="workbench"
      @dragover.prevent="$emit('reference-drag-over')"
      @dragleave="$emit('reference-drag-leave')"
      @drop.prevent="$emit('drop-reference', $event)"
    >
      <div class="prompt-live-head">
        <div class="prompt-live-title">
          <span>提示词</span>
        </div>
        <small>{{ form.prompt.length }} 字</small>
      </div>
      <n-input
        ref="promptInput"
        v-model:value="form.prompt"
        type="textarea"
        class="prompt-input"
        :resizable="false"
        placeholder="写下你要生成的画面、风格、主体、光线和构图"
        @focus="handlePromptFocus"
        @click="handlePromptCursor"
        @keyup="handlePromptCursor"
        @select="handlePromptCursor"
        @paste="$emit('prompt-paste', $event)"
      />
      <div class="prompt-submit-row">
        <n-button size="small" secondary @click="$emit('clear-prompt')">清空</n-button>
        <n-button size="small" secondary @click="$emit('show-template')">
          引用模板
        </n-button>
        <n-button size="small" type="primary" :loading="submitting" @click="$emit('submit')">
          <template #icon><WandSparkles :size="17" /></template>
          开始生成
        </n-button>
      </div>
    </div>
  </section>
</template>

<script setup>
import { CircleAlert, Plus, WandSparkles, X } from "@lucide/vue";
import { ref } from "vue";
import ClipboardImageMenu from "./ClipboardImageMenu.vue";
import {
  promptModeOptions,
  qualityOptions,
  ratioOptions,
  resolutionOptions,
} from "../lib/options";

const props = defineProps({
  form: { type: Object, required: true },
  imageProviderOptions: { type: Array, default: () => [] },
  chatProviderOptions: { type: Array, default: () => [] },
  references: { type: Array, default: () => [] },
  submitting: { type: Boolean, default: false },
  referenceDragActive: { type: Boolean, default: false },
});

const emit = defineEmits([
  "submit",
  "show-template",
  "clear-prompt",
  "prompt-focus",
  "prompt-cursor",
  "prompt-paste",
  "paste-reference",
  "add-reference",
  "remove-reference",
  "reference-drag-over",
  "reference-drag-leave",
  "drop-reference",
]);

const promptInput = ref(null);
const modelSelectMenuProps = { class: "model-select-menu" };

function handlePromptFocus(event) {
  emit("prompt-focus", event);
}

function handlePromptCursor(event) {
  emit("prompt-cursor", event);
}
</script>
