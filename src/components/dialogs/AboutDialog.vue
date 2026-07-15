<template>
  <n-modal v-model:show="show" preset="card" title="关于 Image Forge" class="about-modal">
    <div class="about-body">
      <section class="about-meta">
        <div>
          <span>版本</span>
          <strong>{{ info.version || "未知" }}</strong>
        </div>
        <div>
          <span>编译时间</span>
          <strong>{{ info.buildTime || "未知" }}</strong>
        </div>
      </section>

      <p class="about-copy">
        Image Forge 是一个本地 AI 生图工作台。它只在本机保存配置、队列、历史和模板；
        API 请求会发送到配置的供应商。可以通过语言模型来润色自己的提示词模板。
      </p>

      <n-input
        :value="info.logs || '本次运行还没有日志。'"
        type="textarea"
        readonly
        class="about-log"
        :autosize="{ minRows: 14, maxRows: 18 }"
      />
    </div>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  info: {
    type: Object,
    default: () => ({ version: "", buildTime: "", logs: "" }),
  },
});
</script>
