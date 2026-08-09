<template>
  <div class="about-panel">
    <section class="about-meta">
      <img :src="logoUrl" alt="Image Forge" class="about-logo" />
      <dl class="about-details">
        <div>
          <dt>版本</dt>
          <dd>{{ info.version || "未知" }}</dd>
        </div>
        <div>
          <dt>编译时间</dt>
          <dd>{{ info.buildTime || "未知" }}</dd>
        </div>
        <div>
          <dt>开发者</dt>
          <dd>Tairraos</dd>
        </div>
      </dl>
    </section>

    <div class="dialog-actions">
      <n-button size="small" secondary @click="emit('cleanup')">清理</n-button>
      <n-button size="small" secondary @click="emit('show-logs')">日志</n-button>
      <n-button size="small" @click="emit('close')">关闭</n-button>
    </div>
  </div>
</template>

<script setup>
import logoUrl from "../../assets/title.png";

defineProps({
  info: {
    type: Object,
    default: () => ({ version: "", buildTime: "" }),
  },
});

const emit = defineEmits(["show-logs", "cleanup", "close"]);
</script>
