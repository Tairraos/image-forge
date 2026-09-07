<template>
  <div class="about-panel">
    <section class="about-meta">
      <div class="about-brand">
        <span class="about-brand-icon"><Aperture :size="32" :stroke-width="1.5" /></span>
        <div>
          <h3>Image Forge</h3>
          <p>本地优先的 AI 图像工作台</p>
        </div>
        <span class="version-badge">v{{ info.version || '未知' }}</span>
      </div>
      <dl class="about-details">
        <div>
          <dt>开发者</dt>
          <dd>Tairraos</dd>
        </div>
        <div>
          <dt>编译时间</dt>
          <dd>{{ info.buildTime || '未知' }}</dd>
        </div>
      </dl>
    </section>
    <section class="about-stats" aria-label="数据统计">
      <div class="about-stat">
        <Images :size="17" /><strong>{{ stats.images }}</strong
        ><span>张图片</span>
      </div>
      <div class="about-stat">
        <MessageSquare :size="17" /><strong>{{ stats.sessions }}</strong
        ><span>段对话</span>
      </div>
      <div class="about-stat">
        <Plug :size="17" /><strong>{{ stats.providers }}</strong
        ><span>个 API 源</span>
      </div>
    </section>
    <section class="about-data">
      <h3>数据管理</h3>
      <div class="about-actions">
        <button type="button" class="about-action" @click="emit('export-data')">
          <ArrowDownToLine :size="19" /><span
            ><strong>备份数据</strong><small>选择需要的内容，导出为 ZIP 文件</small></span
          ><ChevronRight :size="16" />
        </button>
        <button type="button" class="about-action" @click="emit('import-data')">
          <ArrowUpFromLine :size="19" /><span
            ><strong>恢复数据</strong><small>从备份导入，自动合并已有内容</small></span
          ><ChevronRight :size="16" />
        </button>
        <button type="button" class="about-action" @click="emit('cleanup')">
          <HardDrive :size="19" /><span
            ><strong>清理存储空间</strong><small>查找未使用的文件，确认后移入回收站</small></span
          ><ChevronRight :size="16" />
        </button>
      </div>
    </section>
  </div>
</template>

<script setup>
import {
  Aperture,
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronRight,
  HardDrive,
  MessageSquare,
  Images,
  Plug,
} from '@lucide/vue';

defineProps({
  info: {
    type: Object,
    default: () => ({ version: '', buildTime: '' }),
  },
  stats: {
    type: Object,
    default: () => ({ images: 0, sessions: 0, providers: 0 }),
  },
});

const emit = defineEmits(['export-data', 'import-data', 'cleanup']);
</script>
