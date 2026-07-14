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
              size="small"
              placeholder="选择生图模型"
            />
          </n-form-item>
        </div>
      </n-form>
    </div>

    <div class="reference-strip">
      <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
        <img :src="item.previewUrl" :alt="item.fileName" />
        <button type="button" title="移除参考图" @click="$emit('remove-reference', index)">
          <X :size="14" />
        </button>
      </div>
      <button class="reference-add" type="button" @click="$emit('add-reference')">
        <Plus :size="18" />
        <span>参考图</span>
      </button>
    </div>

    <div class="prompt-live-panel">
      <div class="prompt-live-head">
        <span>提示词</span>
        <small>{{ form.prompt.length }} 字</small>
      </div>
      <n-input
        v-model:value="form.prompt"
        type="textarea"
        class="prompt-input"
        :resizable="false"
        placeholder="写下你要生成的画面、风格、主体、光线和构图"
        @focus="$emit('prompt-focus', $event)"
        @click="$emit('prompt-cursor', $event)"
        @keyup="$emit('prompt-cursor', $event)"
        @select="$emit('prompt-cursor', $event)"
        @paste="$emit('prompt-paste', $event)"
      />
      <div class="prompt-submit-row">
        <n-button size="small" secondary @click="$emit('clear-prompt')">清空</n-button>
        <n-button size="small" secondary @click="$emit('save-template')">存为模板</n-button>
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
import { Plus, WandSparkles, X } from "@lucide/vue";
import {
  promptModeOptions,
  qualityOptions,
  ratioOptions,
  resolutionOptions,
} from "../lib/options";

defineProps({
  form: { type: Object, required: true },
  imageProviderOptions: { type: Array, default: () => [] },
  references: { type: Array, default: () => [] },
  submitting: { type: Boolean, default: false },
});

defineEmits([
  "submit",
  "show-template",
  "save-template",
  "clear-prompt",
  "prompt-focus",
  "prompt-cursor",
  "prompt-paste",
  "add-reference",
  "remove-reference",
]);
</script>
