<template>
  <n-modal v-model:show="show" preset="card" title="关于 Image Forge" class="about-modal">
    <section class="about-meta">
      <div>
        <span>版本</span>
        <strong>{{ info.version || "未知" }}</strong>
      </div>
      <div>
        <span>编译时间</span>
        <strong>{{ info.buildTime || "未知" }}</strong>
      </div>
      <div>
        <span>开发者</span>
        <strong>Tairraos</strong>
      </div>
    </section>

    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" secondary @click="$emit('show-logs')">日志</n-button>
        <n-button size="small" @click="show = false">关闭</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  info: {
    type: Object,
    default: () => ({ version: "", buildTime: "" }),
  },
});

defineEmits(["show-logs"]);
</script>
