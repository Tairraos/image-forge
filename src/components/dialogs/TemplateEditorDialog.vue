<template>
  <n-modal v-model:show="show" preset="card" title="编辑模板" class="editor-modal">
    <n-form label-placement="top" :show-feedback="false">
      <div class="two-col">
        <n-form-item label="标题">
          <n-input v-model:value="template.title" />
        </n-form-item>
        <n-form-item label="分类">
          <n-input v-model:value="template.category" />
        </n-form-item>
      </div>
      <n-form-item label="内容">
        <n-input v-model:value="template.content" type="textarea" :autosize="{ minRows: 8 }" />
      </n-form-item>
      <n-form-item label="备注">
        <n-input v-model:value="template.notes" />
      </n-form-item>
      <n-checkbox v-model:checked="template.favorite">收藏</n-checkbox>
    </n-form>
    <template #footer>
      <n-button size="small" @click="show = false">取消</n-button>
      <n-button size="small" type="primary" @click="$emit('save')">保存模板</n-button>
    </template>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  template: { type: Object, required: true },
});

defineEmits(["save"]);
</script>
