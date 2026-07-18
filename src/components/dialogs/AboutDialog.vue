<template>
  <n-modal v-model:show="show" preset="card" title="关于 Image Forge" class="about-modal">
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

    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" secondary @click="$emit('show-logs')">日志</n-button>
        <n-button size="small" @click="show = false">关闭</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import logoUrl from "../../assets/title.png";

const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  info: {
    type: Object,
    default: () => ({ version: "", buildTime: "" }),
  },
});

defineEmits(["show-logs"]);
</script>
