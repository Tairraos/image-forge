<template>
  <div class="agent-generated-item">
    <button
      type="button"
      class="agent-generated-thumb"
      :aria-label="`查看生成图片 ${index + 1}`"
      :style="tileAspect"
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
import { computed } from 'vue';
import { fileUrl } from '../../lib/formatters';

const props = defineProps({
  image: { type: Object, required: true },
  index: { type: Number, default: 0 },
  // 名义比例兜底：输出记录缺像素尺寸（WxH）时按它定容器比例
  ratio: { type: String, default: '' },
});

defineEmits(['preview', 'copy', 'reuse', 'add-to-template', 'delete']);

// 容器比例取图片真实像素比，成图与容器零留白零裁剪；
// 记录缺尺寸时退回名义比例，边缘由圆角裁掉属可接受
const tileAspect = computed(() => {
  const pixels = /^(\d+)\s*x\s*(\d+)$/i.exec(String(props.image?.size || ''));
  if (pixels) return { aspectRatio: `${pixels[1]} / ${pixels[2]}` };
  const nominal = /^(\d+)\s*:\s*(\d+)$/.exec(String(props.ratio || ''));
  return { aspectRatio: nominal ? `${nominal[1]} / ${nominal[2]}` : '1 / 1' };
});
</script>
