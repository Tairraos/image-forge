<template>
  <div class="template-manager">
    <div class="template-toolbar">
      <label class="search-field"
        ><Search :size="16" /><input
          v-model="query"
          type="search"
          aria-label="搜索模板"
          placeholder="搜索模板标题或内容"
      /></label>
      <div class="template-toolbar-actions">
        <button type="button" class="button button-small button-primary" @click="$emit('create')">
          <Plus :size="15" />
          新建模板
        </button>
        <button type="button" class="button button-small" @click="$emit('import')">
          <Download :size="15" />
          导入
        </button>
        <button type="button" class="button button-small" @click="$emit('export')">
          <Upload :size="15" />
          导出
        </button>
      </div>
    </div>

    <div class="template-workspace">
      <aside class="template-side" aria-label="模板列表" data-persistent-scrollbar>
        <div class="template-list-label">
          {{ query ? '搜索结果' : '全部模板' }}<span>{{ templates.length }}</span>
        </div>
        <button
          v-for="template in templates"
          :key="template.id"
          type="button"
          class="template-side-item"
          :class="{ active: template.id === selectedId }"
          :aria-pressed="template.id === selectedId"
          @click="selectedId = template.id"
        >
          <span class="template-side-thumbnail"
            ><img
              v-if="template.effectImagePath"
              :src="convertFileSrc(template.effectImagePath)"
              alt=""
              loading="lazy" /><LayoutTemplate v-else :size="19"
          /></span>
          <span class="template-side-copy"
            ><span class="template-side-title" :title="template.title || '未命名模板'">{{
              template.title || '未命名模板'
            }}</span
            ><span class="template-side-meta">{{
              template.referencePaths?.length
                ? `${template.referencePaths.length} 张参考图`
                : '提示词模板'
            }}</span></span
          >
        </button>
        <p v-if="!templates.length" class="template-side-empty">
          {{ query ? '没有匹配的模板' : '还没有模板' }}
        </p>
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
            <button
              type="button"
              class="button button-small"
              @click="emit('edit', selectedTemplate)"
            >
              编辑
            </button>
            <button
              type="button"
              class="button button-small button-danger button-ghost"
              @click="emit('delete', selectedTemplate.id)"
            >
              删除
            </button>
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
        <LayoutTemplate :size="30" /><strong>{{
          query ? '没有找到相关模板' : '把好用的提示词留下来'
        }}</strong>
        <p>{{ query ? '试试其他关键词' : '创建模板，复用提示词与参考图。' }}</p>
        <button v-if="!query" type="button" class="button button-primary" @click="emit('create')">
          <Plus :size="15" />新建模板
        </button>
      </section>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue';
import { ArrowDown, ArrowUp, Download, LayoutTemplate, Plus, Search, Upload } from '@lucide/vue';
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
