<template>
  <div class="template-manager">
    <div class="template-toolbar">
      <n-input v-model:value="query" clearable placeholder="搜索标题、模板或 ID">
        <template #prefix><Search :size="15" /></template>
      </n-input>
      <div class="template-toolbar-actions">
        <n-button size="small" type="primary" @click="$emit('create')">
          <template #icon><Plus :size="15" /></template>
          新增
        </n-button>
        <n-button size="small" secondary @click="$emit('import')">
          <template #icon><Download :size="15" /></template>
          导入
        </n-button>
        <n-button size="small" secondary @click="$emit('export')">
          <template #icon><Upload :size="15" /></template>
          导出
        </n-button>
      </div>
    </div>

    <div class="template-workspace">
      <aside class="template-side" aria-label="模板列表" data-persistent-scrollbar>
        <button
          v-for="template in templates"
          :key="template.id"
          type="button"
          class="template-side-item"
          :class="{ active: template.id === selectedId }"
          @click="selectedId = template.id"
        >
          <span class="template-side-title" :title="template.title || '未命名模板'">
            {{ template.title || '未命名模板' }}
          </span>
          <span v-if="template.referencePaths?.length" class="template-side-meta">
            {{ template.referencePaths.length }} 参考图
          </span>
        </button>
        <p v-if="!templates.length" class="template-side-empty">没有模板</p>
      </aside>

      <section v-if="selectedTemplate" class="template-detail">
        <header class="template-detail-head">
          <h3 class="template-detail-title" :title="selectedTemplate.title || '未命名模板'">
            {{ selectedTemplate.title || '未命名模板' }}
          </h3>
          <div class="template-detail-actions">
            <button
              type="button"
              title="上移"
              aria-label="上移模板"
              :disabled="selectedIndex <= 0"
              @click="moveTemplate(-1)"
            >
              <ArrowUp :size="14" />
            </button>
            <button
              type="button"
              title="下移"
              aria-label="下移模板"
              :disabled="selectedIndex < 0 || selectedIndex >= templates.length - 1"
              @click="moveTemplate(1)"
            >
              <ArrowDown :size="14" />
            </button>
            <n-button size="small" secondary @click="emit('edit', selectedTemplate)">
              编辑
            </n-button>
            <n-button
              size="small"
              quaternary
              type="error"
              @click="emit('delete', selectedTemplate.id)"
            >
              删除
            </n-button>
          </div>
        </header>

        <div class="template-detail-body" data-persistent-scrollbar>
          <section class="template-detail-section">
            <h4>提示词</h4>
            <pre class="template-prompt-block">{{
              selectedTemplate.content || '（暂无内容）'
            }}</pre>
          </section>

          <section class="template-detail-section">
            <h4>效果图</h4>
            <button
              v-if="selectedTemplate.effectImagePath"
              type="button"
              class="template-media-thumb effect"
              title="点击查看大图"
              @click="emit('show-effect', selectedTemplate)"
            >
              <img
                :src="convertFileSrc(selectedTemplate.effectImagePath)"
                :alt="`${selectedTemplate.title || '模板'} 效果图`"
                loading="lazy"
              />
            </button>
            <p v-else class="template-detail-empty">未设置效果图</p>
          </section>

          <section class="template-detail-section">
            <h4>
              参考图{{
                selectedTemplate.referencePaths?.length
                  ? `（${selectedTemplate.referencePaths.length}）`
                  : ''
              }}
            </h4>
            <div v-if="selectedTemplate.referencePaths?.length" class="template-reference-grid">
              <button
                v-for="(path, referenceIndex) in selectedTemplate.referencePaths"
                :key="path"
                type="button"
                class="template-media-thumb"
                :title="`点击查看参考图 ${referenceIndex + 1}`"
                @click="showReference(path, referenceIndex)"
              >
                <img
                  :src="convertFileSrc(path)"
                  :alt="`${selectedTemplate.title || '模板'} 参考图 ${referenceIndex + 1}`"
                  loading="lazy"
                />
              </button>
            </div>
            <p v-else class="template-detail-empty">未设置参考图</p>
          </section>
        </div>
      </section>

      <section v-else class="template-detail template-detail-placeholder">
        <p>左侧选择一个模板查看详情</p>
      </section>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue';
import { ArrowDown, ArrowUp, Download, Plus, Search, Upload } from '@lucide/vue';
import { convertFileSrc } from '../../tauri';

const query = defineModel('query', { type: String, default: '' });

const props = defineProps({
  templates: { type: Array, default: () => [] },
});

const emit = defineEmits([
  'create',
  'import',
  'export',
  'edit',
  'delete',
  'move',
  'show-effect',
  'show-image',
]);

const selectedId = ref('');

const selectedTemplate = computed(
  () => props.templates.find((template) => template.id === selectedId.value) || null
);
const selectedIndex = computed(() =>
  props.templates.findIndex((template) => template.id === selectedId.value)
);

// 当前选中项被删除或过滤掉时，回落到列表第一项。
watch(
  () => props.templates,
  (list) => {
    if (!list.some((template) => template.id === selectedId.value)) {
      selectedId.value = list[0]?.id || '';
    }
  },
  { immediate: true }
);

function showReference(path, index) {
  emit('show-image', {
    path,
    title: `${selectedTemplate.value?.title || '模板'} · 参考图 ${index + 1}`,
  });
}

function moveTemplate(offset) {
  const template = selectedTemplate.value;
  const target = props.templates[selectedIndex.value + offset];
  if (!template || !target) return;
  emit('move', { templateId: template.id, targetTemplateId: target.id });
}
</script>
