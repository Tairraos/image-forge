<template>
  <n-modal v-model:show="show" preset="card" title="系统设置" class="settings-modal">
    <n-form label-placement="top" :show-feedback="false">
      <n-form-item label="输出目录">
        <n-input v-model:value="settings.outputDir" placeholder="默认写入应用数据目录">
          <template #suffix>
            <n-button size="small" text @click="$emit('choose-output-dir')">
              <FolderOpen :size="16" />
            </n-button>
          </template>
        </n-input>
      </n-form-item>
      <div class="switch-list">
        <label>
          <span>自动启动队列</span>
          <n-switch v-model:value="settings.autoStartQueue" />
        </label>
        <label>
          <span>失败自动重试一次</span>
          <n-switch v-model:value="settings.autoRetry" />
        </label>
        <label>
          <span>应用内完成提示</span>
          <n-switch v-model:value="settings.notificationsEnabled" />
        </label>
      </div>
    </n-form>
    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" @click="show = false">取消</n-button>
        <n-button size="small" type="primary" @click="$emit('save')">
          <template #icon><Save :size="16" /></template>
          保存设置
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { FolderOpen, Save } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  settings: { type: Object, required: true },
});

defineEmits(["choose-output-dir", "save"]);
</script>
