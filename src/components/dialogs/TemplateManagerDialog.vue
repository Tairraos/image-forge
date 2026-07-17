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
            <template #icon><Download :size="15" /></template>
            导入
          </n-button>
          <n-button size="small" secondary @click="$emit('export')">
            <template #icon><Upload :size="15" /></template>
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
                <button
                  type="button"
                  class="template-title-button"
                  @mouseenter="showPromptPopover(template, $event)"
                  @mouseleave="schedulePromptPopoverHide"
                  @click="emit('view', template)"
                >
                  {{ template.title }}
                </button>
              </td>
              <td class="template-reference-cell">
                <button
                  v-if="template.referencePaths?.length"
                  type="button"
                  class="template-reference-button"
                  @mouseenter="showImagePopover(template, $event)"
                  @mouseleave="scheduleImagePopoverHide"
                >
                  {{ referenceCount(template) }}
                </button>
              </td>
              <td>
                <div class="template-table-actions">
                  <n-button
                    v-if="template.effectImagePath"
                    size="tiny"
                    secondary
                    @click="emit('show-effect', template)"
                  >
                    效果图
                  </n-button>
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

  <n-popover
    trigger="manual"
    :show="promptPopover.show"
    :x="promptPopover.x"
    :y="promptPopover.y"
    :placement="promptPopover.placement"
    :z-index="4000"
    :flip="true"
    to="body"
    style="max-width: 440px"
    content-class="template-prompt-popover"
  >
    <div
      class="template-prompt-preview"
      @mouseenter="cancelPromptPopoverHide"
      @mouseleave="schedulePromptPopoverHide"
    >
      {{ promptPopover.template?.content }}
    </div>
  </n-popover>

  <n-popover
    trigger="manual"
    :show="imagePopover.show"
    :x="imagePopover.x"
    :y="imagePopover.y"
    :placement="imagePopover.placement"
    :z-index="4000"
    :flip="true"
    to="body"
    content-class="template-image-popover"
  >
    <div
      class="template-reference-preview"
      @mouseenter="cancelImagePopoverHide"
      @mouseleave="scheduleImagePopoverHide"
    >
      <span
        v-for="(path, referenceIndex) in imagePopover.template?.referencePaths || []"
        :key="path"
        class="template-reference-thumbnail"
      >
        <img
          :src="convertFileSrc(path)"
          :alt="`${imagePopover.template?.title || '模板'} 参考图 ${referenceIndex + 1}`"
          loading="lazy"
        />
      </span>
    </div>
  </n-popover>
</template>

<script setup>
import { onUnmounted, reactive } from "vue";
import { ArrowDown, ArrowUp, Download, Plus, Search, Upload } from "@lucide/vue";
import { convertFileSrc } from "../../tauri";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

const props = defineProps({
  templates: { type: Array, default: () => [] },
});

const emit = defineEmits([
  "create",
  "import",
  "export",
  "view",
  "edit",
  "delete",
  "move",
  "show-effect",
]);
const promptPopover = reactive(emptyPopover());
const imagePopover = reactive(emptyPopover());
let promptHideTimer = 0;
let imageHideTimer = 0;

onUnmounted(() => {
  window.clearTimeout(promptHideTimer);
  window.clearTimeout(imageHideTimer);
});

function emptyPopover() {
  return { show: false, x: 0, y: 0, placement: "bottom-start", template: null };
}

function popoverPosition(event, estimatedWidth, estimatedHeight) {
  const gap = 10;
  const edge = 12;
  const showAbove = window.innerHeight - event.clientY < estimatedHeight + gap
    && event.clientY > estimatedHeight;
  return {
    x: Math.max(edge, Math.min(event.clientX + gap, window.innerWidth - estimatedWidth - edge)),
    y: event.clientY + (showAbove ? -gap : gap),
    placement: showAbove ? "top-start" : "bottom-start",
  };
}

function showPromptPopover(template, event) {
  cancelPromptPopoverHide();
  Object.assign(promptPopover, popoverPosition(event, 440, 280), { show: true, template });
}

function schedulePromptPopoverHide() {
  window.clearTimeout(promptHideTimer);
  promptHideTimer = window.setTimeout(() => { promptPopover.show = false; }, 180);
}

function cancelPromptPopoverHide() {
  window.clearTimeout(promptHideTimer);
}

function showImagePopover(template, event) {
  cancelImagePopoverHide();
  Object.assign(imagePopover, popoverPosition(event, 304, 240), { show: true, template });
}

function scheduleImagePopoverHide() {
  window.clearTimeout(imageHideTimer);
  imageHideTimer = window.setTimeout(() => { imagePopover.show = false; }, 180);
}

function cancelImagePopoverHide() {
  window.clearTimeout(imageHideTimer);
}

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
