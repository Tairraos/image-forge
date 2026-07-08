<template>
  <n-modal v-model:show="show" preset="card" title="编辑图库条目" class="editor-modal">
    <n-form label-placement="top" :show-feedback="false">
      <n-form-item label="名称">
        <n-input v-model:value="item.name" />
      </n-form-item>
      <n-form-item label="分类">
        <n-input v-model:value="item.category" />
      </n-form-item>
      <n-form-item label="备注">
        <n-input v-model:value="item.note" type="textarea" :autosize="{ minRows: 3 }" />
      </n-form-item>
    </n-form>
    <template #footer>
      <n-button size="small" @click="show = false">取消</n-button>
      <n-button size="small" type="primary" @click="$emit('save')">保存图库</n-button>
    </template>
  </n-modal>
</template>

<script setup>
const show = defineModel("show", { type: Boolean, default: false });

defineProps({
  item: { type: Object, required: true },
});

defineEmits(["save"]);
</script>
