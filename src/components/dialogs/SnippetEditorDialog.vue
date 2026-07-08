<template>
  <n-modal v-model:show="show" preset="card" title="编辑片段" class="editor-modal">
    <n-form label-placement="top" :show-feedback="false">
      <div class="two-col">
        <n-form-item label="短标签">
          <n-input v-model:value="snippet.tag" placeholder="例如 portrait" />
        </n-form-item>
        <n-form-item label="分类">
          <n-input v-model:value="snippet.category" />
        </n-form-item>
      </div>
      <n-form-item label="标题">
        <n-input v-model:value="snippet.title" />
      </n-form-item>
      <n-form-item label="内容">
        <n-input v-model:value="snippet.content" type="textarea" :autosize="{ minRows: 5 }" />
      </n-form-item>
    </n-form>
    <template #footer>
      <n-button size="small" @click="show = false">取消</n-button>
      <n-button size="small" type="primary" @click="$emit('save')">保存片段</n-button>
    </template>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  snippet: { type: Object, required: true },
});

defineEmits(["save"]);
</script>
