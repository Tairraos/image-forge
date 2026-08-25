<template>
  <n-modal
    v-model:show="visible"
    preset="card"
    class="data-transfer-modal"
    :title="mode === 'export' ? '导出数据' : '导入数据'"
    :bordered="false"
    style="width: min(560px, calc(100vw - 48px))"
  >
    <template v-if="mode === 'export'">
      <p class="data-transfer-desc">选择要导出的内容，生成一个 ZIP 文件。</p>
      <n-checkbox-group v-model:value="selected">
        <n-space vertical>
          <n-checkbox value="settings" label="API 源配置" />
          <n-checkbox value="templates" label="提示词模板" />
          <n-checkbox value="sessions" label="Agent 对话" />
          <n-checkbox value="tasks" label="图片库" />
        </n-space>
      </n-checkbox-group>
      <div class="data-transfer-actions">
        <n-button @click="emit('update:show', false)">取消</n-button>
        <n-button
          type="primary"
          :loading="exporting"
          :disabled="!selected.length"
          @click="doExport"
        >
          导出 ZIP
        </n-button>
      </div>
      <p v-if="exportResult" class="data-transfer-result">{{ exportResult }}</p>
    </template>

    <template v-else>
      <p class="data-transfer-desc">从 ZIP 文件导入数据，将覆盖或合并现有数据。</p>
      <div
        class="data-transfer-drop"
        :class="{ active: dragActive }"
        @dragover.prevent="dragActive = true"
        @dragleave="dragActive = false"
        @drop.prevent="handleDrop"
      >
        <p>拖入 ZIP 文件，或点击选择</p>
        <n-button size="small" @click="pickFile">选择文件</n-button>
      </div>
      <input
        ref="fileInput"
        type="file"
        accept=".zip"
        style="display: none"
        @change="handleFileChange"
      />
      <div class="data-transfer-actions">
        <n-button @click="emit('update:show', false)">关闭</n-button>
      </div>
      <p v-if="importResult" class="data-transfer-result">{{ importResult }}</p>
    </template>
  </n-modal>
</template>

<script setup>
import { inject, ref, watch, computed } from 'vue';
import * as api from '../../api/index.js';

const props = defineProps({
  show: Boolean,
  mode: { type: String, default: 'export' },
});
const emit = defineEmits(['update:show']);

const visible = computed({
  get: () => props.show,
  set: (val) => emit('update:show', val),
});

const selected = ref(['settings', 'templates', 'sessions', 'tasks']);
const exporting = ref(false);
const exportResult = ref('');
const importResult = ref('');
const dragActive = ref(false);
const fileInput = ref(null);

watch(
  () => props.show,
  (visible) => {
    if (visible) {
      exportResult.value = '';
      importResult.value = '';
      selected.value = ['settings', 'templates', 'sessions', 'tasks'];
    }
  }
);

function pickFile() {
  fileInput.value?.click();
}

async function handleFileChange(event) {
  const file = event.target.files?.[0];
  if (file) await doImport(file);
  event.target.value = '';
}

async function handleDrop(event) {
  dragActive.value = false;
  const file = event.dataTransfer?.files?.[0];
  if (file) await doImport(file);
}

async function doExport() {
  exporting.value = true;
  exportResult.value = '';
  try {
    const result = await api.exportDataBundle(selected.value);
    exportResult.value = `已导出到：${result}`;
  } catch (error) {
    exportResult.value = `导出失败：${String(error)}`;
  } finally {
    exporting.value = false;
  }
}

async function doImport(file) {
  importResult.value = '导入中...';
  try {
    const result = await api.importDataBundle(file);
    importResult.value = `导入完成：设置 ${result.settings} 个、模板 ${result.templates} 个、会话 ${result.sessions} 个、图片 ${result.tasks} 条`;
  } catch (error) {
    importResult.value = `导入失败：${String(error)}`;
  }
}
</script>

<style scoped>
.data-transfer-desc {
  margin: 0 0 12px;
  color: #666;
  font-size: 13px;
}
.data-transfer-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}
.data-transfer-result {
  margin: 12px 0 0;
  color: #237257;
  font-size: 12px;
  word-break: break-all;
}
.data-transfer-drop {
  display: grid;
  place-items: center;
  gap: 8px;
  padding: 24px;
  border: 1px dashed #c8bdf0;
  border-radius: 10px;
  background: #faf8ff;
  text-align: center;
}
.data-transfer-drop.active {
  border-color: #7c5ce8;
  background: #f4efff;
}
.data-transfer-drop p {
  margin: 0;
  color: #666;
  font-size: 13px;
}
</style>
