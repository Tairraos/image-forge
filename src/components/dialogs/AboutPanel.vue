<template>
  <div class="about-panel">
    <section class="about-meta">
      <img :src="logoUrl" alt="Image Forge" class="about-logo" />
      <dl class="about-details">
        <div>
          <dt>版本</dt>
          <dd>{{ info.version || '未知' }}</dd>
        </div>
        <div>
          <dt>编译时间</dt>
          <dd>{{ info.buildTime || '未知' }}</dd>
        </div>
        <div>
          <dt>开发者</dt>
          <dd>Tairraos</dd>
        </div>
      </dl>
    </section>

    <section class="about-stats" aria-label="数据统计">
      <div class="about-stat">
        <strong>{{ stats.images }}</strong>
        <span>图片</span>
      </div>
      <div class="about-stat">
        <strong>{{ stats.sessions }}</strong>
        <span>对话</span>
      </div>
      <div class="about-stat">
        <strong>{{ stats.providers }}</strong>
        <span>API 项</span>
      </div>
    </section>

    <p class="about-note">
      备份会把对话、图片、图片库信息和 API 配置导出为 ZIP 文件，可勾选包含的内容；恢复时自动合并。
    </p>

    <div class="about-actions">
      <n-button
        class="about-action-btn"
        size="large"
        type="primary"
        secondary
        @click="emit('export-data')"
      >
        备份
      </n-button>
      <n-button
        class="about-action-btn"
        size="large"
        type="primary"
        secondary
        @click="emit('import-data')"
      >
        恢复
      </n-button>
      <n-button class="about-action-btn" size="large" secondary @click="emit('cleanup')">
        清理
      </n-button>
    </div>
  </div>
</template>

<script setup>
import logoUrl from '../../assets/title.png';

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
