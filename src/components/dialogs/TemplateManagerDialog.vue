<template>
  <n-modal v-model:show="show" preset="card" title="模板维护" class="template-manager-modal">
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
            <template #icon><Upload :size="15" /></template>
            导入
          </n-button>
          <n-button size="small" secondary @click="$emit('export')">
            <template #icon><FileArchive :size="15" /></template>
            导出
          </n-button>
        </div>
      </div>

      <div class="template-table-wrap">
        <table class="template-table">
          <thead>
            <tr>
              <th>排序</th>
              <th>标题</th>
              <th>参考图</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(template, index) in templates" :key="template.id">
              <td class="template-sort-cell">
                <div class="template-sort-actions">
                  <button
                    type="button"
                    title="上移"
                    aria-label="上移模板"
                    :disabled="index === 0"
                    @click="moveTemplate(index, -1)"
                  >
                    <ArrowUp :size="13" />
                  </button>
                  <button
                    type="button"
                    title="下移"
                    aria-label="下移模板"
                    :disabled="index === templates.length - 1"
                    @click="moveTemplate(index, 1)"
                  >
                    <ArrowDown :size="13" />
                  </button>
                </div>
              </td>
              <td class="template-title-cell">
                <n-popover
                  trigger="hover"
                  placement="right-start"
                  :delay="280"
                  :max-width="440"
                  content-class="template-prompt-popover"
                >
                  <template #trigger>
                    <button
                      type="button"
                      class="template-title-button"
                      @click="emit('view', template)"
                    >
                      {{ template.title }}
                    </button>
                  </template>
                  <div class="template-prompt-preview">{{ template.content }}</div>
                </n-popover>
              </td>
              <td class="template-reference-cell">
                <n-popover
                  v-if="template.referencePaths?.length"
                  trigger="hover"
                  placement="left-start"
                  :delay="280"
                  content-class="template-image-popover"
                >
                  <template #trigger>
                    <button type="button" class="template-reference-button">
                      {{ referenceCount(template) }}
                    </button>
                  </template>
                  <div class="template-reference-preview">
                    <span
                      v-for="(path, referenceIndex) in template.referencePaths"
                      :key="path"
                      class="template-reference-thumbnail"
                    >
                      <img
                        :src="convertFileSrc(path)"
                        :alt="`${template.title} 参考图 ${referenceIndex + 1}`"
                        loading="lazy"
                      />
                    </span>
                  </div>
                </n-popover>
              </td>
              <td>
                <div class="template-table-actions">
                  <n-button size="tiny" secondary @click="emit('edit', template)">编辑</n-button>
                  <n-button size="tiny" quaternary type="error" @click="emit('delete', template.id)">
                    删除
                  </n-button>
                </div>
              </td>
            </tr>
            <tr v-if="!templates.length">
              <td colspan="4" class="template-empty-cell">没有模板</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { ArrowDown, ArrowUp, FileArchive, Plus, Search, Upload } from "@lucide/vue";
import { convertFileSrc } from "../../tauri";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

const props = defineProps({
  templates: { type: Array, default: () => [] },
});

const emit = defineEmits(["create", "import", "export", "view", "edit", "delete", "move"]);

function moveTemplate(index, offset) {
  const template = props.templates[index];
  const target = props.templates[index + offset];
  if (!template || !target) return;
  emit("move", { templateId: template.id, targetTemplateId: target.id });
}

function referenceCount(template) {
  const count = template.referencePaths?.length || 0;
  return count ? `${count} 个参考图` : "";
}
</script>
