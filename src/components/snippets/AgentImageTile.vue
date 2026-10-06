<template>
  <div class="agent-generated-item">
    <button
      type="button"
      class="agent-generated-thumb"
      :aria-label="`查看生成图片 ${index + 1}`"
      :style="aspectStyle"
      @click="$emit('preview')"
    >
      <img
        loading="lazy"
        :src="fileUrl(image.path)"
        :alt="image.title || image.fileName || '生成图片'"
      />
    </button>
    <div class="agent-generated-tools" role="toolbar" aria-label="图片操作">
      <button
        type="button"
        class="agent-generated-tool"
        aria-label="复制图片"
        title="复制图片"
        @click="$emit('copy')"
      >
        <Copy :size="15" />
      </button>
      <button
        type="button"
        class="agent-generated-tool"
        aria-label="再来一张"
        title="把这张图的提示词和参考图填到输入框"
        @click="$emit('reuse')"
      >
        <RotateCcw :size="15" />
      </button>
      <button
        type="button"
        class="agent-generated-tool"
        aria-label="添加到模板"
        title="添加到模板"
        @click="$emit('add-to-template')"
      >
        <BookmarkPlus :size="15" />
      </button>
      <button
        type="button"
        class="agent-generated-tool"
        aria-label="从对话删除"
        title="从对话删除提示词和图片（图片库保留）"
        @click="$emit('delete')"
      >
        <Trash2 :size="15" />
      </button>
    </div>
  </div>
</template>

<script setup>
import { BookmarkPlus, Copy, RotateCcw, Trash2 } from '@lucide/vue';
import { fileUrl } from '../../lib/formatters';

defineProps({
  image: { type: Object, required: true },
  index: { type: Number, default: 0 },
  // 批量网格里按任务比例锁定宽高；单图路径传空对象走自适应
  aspectStyle: { type: Object, default: () => ({}) },
});

defineEmits(['preview', 'copy', 'reuse', 'add-to-template', 'delete']);
</script>
