<template>
  <section class="composer-column">
    <div class="section-head">
      <div>
        <h2>生成工作台</h2>
        <p>OpenAI 兼容 Images API</p>
      </div>
      <n-button size="small" type="primary" :loading="submitting" @click="$emit('submit')">
        <template #icon><WandSparkles :size="17" /></template>
        加入队列
      </n-button>
    </div>

    <div class="control-surface">
      <n-form label-placement="top" :show-feedback="false">
        <div class="control-grid">
          <n-form-item label="尺寸">
            <n-select v-model:value="form.size" :options="sizeOptions" size="small" />
          </n-form-item>
          <n-form-item label="质量">
            <n-select v-model:value="form.quality" :options="qualityOptions" size="small" />
          </n-form-item>
          <n-form-item label="格式">
            <n-select v-model:value="form.outputFormat" :options="formatOptions" size="small" />
          </n-form-item>
          <n-form-item label="数量">
            <n-input-number v-model:value="form.count" :min="1" :max="8" size="small" />
          </n-form-item>
          <n-form-item label="背景">
            <n-select v-model:value="form.background" :options="backgroundOptions" size="small" />
          </n-form-item>
          <n-form-item label="压缩">
            <n-input-number
              v-model:value="form.outputCompression"
              size="small"
              clearable
              :min="0"
              :max="100"
              placeholder="可空"
            />
          </n-form-item>
          <n-form-item label="保真">
            <n-select v-model:value="form.inputFidelity" :options="fidelityOptions" size="small" />
          </n-form-item>
        </div>
      </n-form>
    </div>

    <n-input
      v-model:value="form.prompt"
      type="textarea"
      class="prompt-input"
      :resizable="false"
      placeholder="写下你要生成的画面、风格、主体、光线和构图"
    />

    <div class="quick-bar">
      <n-button size="small" secondary @click="$emit('show-template')">
        <template #icon><BookOpen :size="15" /></template>
        插入模板
      </n-button>
      <n-button size="small" secondary @click="$emit('show-snippet')">
        <template #icon><Layers :size="15" /></template>
        插入片段
      </n-button>
      <n-button size="small" secondary @click="$emit('show-gallery')">
        <template #icon><Image :size="15" /></template>
        从图库选图
      </n-button>
      <n-button size="small" secondary @click="$emit('add-reference')">
        <template #icon><Upload :size="15" /></template>
        添加参考图
      </n-button>
    </div>

    <div class="reference-strip">
      <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
        <img :src="item.previewUrl" :alt="item.fileName" />
        <button type="button" title="移除参考图" @click="$emit('remove-reference', index)">
          <XCircle :size="16" />
        </button>
      </div>
      <button class="reference-add" type="button" @click="$emit('add-reference')">
        <Plus :size="18" />
        <span>参考图</span>
      </button>
    </div>
  </section>
</template>

<script setup>
import { BookOpen, Image, Layers, Plus, Upload, WandSparkles, XCircle } from "@lucide/vue";
import {
  backgroundOptions,
  fidelityOptions,
  formatOptions,
  qualityOptions,
  sizeOptions,
} from "../lib/options";

defineProps({
  form: { type: Object, required: true },
  references: { type: Array, default: () => [] },
  submitting: { type: Boolean, default: false },
});

defineEmits([
  "submit",
  "show-template",
  "show-snippet",
  "show-gallery",
  "add-reference",
  "remove-reference",
]);
</script>
