<template>
  <n-modal v-model:show="show" preset="card" title="清理孤岛文件" class="cleanup-modal">
    <div class="cleanup-content">
      <p class="cleanup-summary">以下文件没有被当前历史、模板、队列或请求引用。确认后会移入系统回收站。</p>
      <div v-if="loading" class="cleanup-state">正在扫描数据文件…</div>
      <div v-else-if="error" class="cleanup-state cleanup-error">{{ error }}</div>
      <div v-else-if="!candidates.length" class="cleanup-state">没有发现可以清理的孤岛文件。</div>
      <div v-else class="cleanup-table-wrap">
        <table class="cleanup-table">
          <thead><tr><th>文件</th><th>大小</th></tr></thead>
          <tbody>
            <tr v-for="candidate in candidates" :key="candidate.path">
              <td :title="candidate.path">{{ candidate.relativePath }}</td>
              <td>{{ formatBytes(candidate.size) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" @click="show = false">取消</n-button>
        <n-button size="small" type="primary" :loading="confirming" :disabled="loading || !!error || !candidates.length" @click="$emit('confirm')">确认清理</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });
defineProps({
  candidates: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  confirming: { type: Boolean, default: false },
  error: { type: String, default: "" },
});
defineEmits(["confirm"]);

function formatBytes(value) {
  if (!value) return "0 B";
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`;
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
</script>
