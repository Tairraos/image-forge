<template>
  <NativeDialog
    v-model:show="visible"
    class="data-transfer-modal"
    :title="mode === 'export' ? '导出数据' : '导入数据'"
  >
    <template v-if="mode === 'export'">
      <p class="data-transfer-desc">选择要导出的内容，生成一个 ZIP 文件。</p>
      <div class="data-transfer-options">
        <label class="checkbox-field"
          ><input v-model="selected" type="checkbox" value="settings" /><span
            >API 源配置<small>服务地址、模型与密钥</small></span
          ></label
        >
        <label class="checkbox-field"
          ><input v-model="selected" type="checkbox" value="templates" /><span
            >提示词模板<small>模板内容与参考图</small></span
          ></label
        >
        <label class="checkbox-field"
          ><input v-model="selected" type="checkbox" value="sessions" /><span
            >Agent 对话<small>会话记录与创作过程</small></span
          ></label
        >
        <label class="checkbox-field"
          ><input v-model="selected" type="checkbox" value="tasks" /><span
            >图片库<small>生成的图片与任务信息</small></span
          ></label
        >
      </div>
      <p v-if="selected.includes('settings')" class="data-transfer-key-note">
        备份包含明文 API Key，请保存在可信位置。
      </p>
      <div class="data-transfer-actions">
        <button type="button" class="button" @click="emit('update:show', false)">取消</button>
        <button
          type="button"
          class="button button-primary"
          :aria-busy="exporting"
          :disabled="!selected.length || exporting"
          @click="doExport"
        >
          导出 ZIP
        </button>
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
        <button type="button" class="button button-small" @click="pickFile">选择文件</button>
      </div>
      <input
        ref="fileInput"
        type="file"
        accept=".zip"
        style="display: none"
        @change="handleFileChange"
      />
      <div class="data-transfer-actions">
        <button type="button" class="button" @click="emit('update:show', false)">关闭</button>
      </div>
      <p v-if="importResult" class="data-transfer-result">{{ importResult }}</p>
    </template>
  </NativeDialog>
</template>

<script setup>
import NativeDialog from './NativeDialog.vue';
import { ref, watch, computed } from 'vue';
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
