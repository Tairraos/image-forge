<template>
  <n-modal v-model:show="show" preset="card" :title="dialogTitle" class="skill-editor-modal">
    <div class="skill-edit-body">
      <div
        v-if="mode === 'new'"
        class="skill-url-row skill-md-drop-zone"
        :class="{ 'skill-md-drop-active': dragActive }"
        data-skill-drop-target="url"
        @dragover.prevent="dragActive = true"
        @dragleave="dragActive = false"
        @drop.prevent="dropMarkdown"
      >
        <n-input
          v-model:value="skill.sourceUrl"
          :readonly="readonly"
          placeholder="https://example.com/SKILL.md"
          @keydown.enter.prevent="readonly ? null : $emit('fetch')"
        >
          <template #prefix><Link :size="15" /></template>
        </n-input>
        <n-button
          v-if="!readonly"
          size="small"
          secondary
          :loading="fetching"
          :disabled="!skill.sourceUrl.trim()"
          @click="$emit('fetch')"
        >
          从 URL 提取
        </n-button>
      </div>
      <n-input
        v-model:value="skill.notes"
        :readonly="readonly"
        maxlength="200"
        placeholder="备注（可选）"
      />
      <div
        class="skill-md-drop-zone"
        :class="{ 'skill-md-drop-active': dragActive }"
        data-skill-drop-target="content"
        @dragover.prevent="dragActive = true"
        @dragleave="dragActive = false"
        @drop.prevent="dropMarkdown"
      >
        <n-input
          v-model:value="skill.content"
          type="textarea"
          class="skill-content-input"
          :readonly="readonly"
          :autosize="{ minRows: 12, maxRows: 12 }"
          :resizable="false"
          placeholder="粘贴 SKILL.md 内容，或拖入 .md 文件"
        />
      </div>
    </div>

    <template #footer>
      <div class="dialog-actions">
        <n-button size="small" @click="show = false">{{ readonly ? "关闭" : "取消" }}</n-button>
        <n-button v-if="!readonly" size="small" type="primary" @click="$emit('save')">保存</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { Link } from "@lucide/vue";
import { computed, ref } from "vue";

const show = defineModel("show", { type: Boolean, default: false });

const props = defineProps({
  skill: { type: Object, required: true },
  mode: { type: String, default: "edit" },
  fetching: { type: Boolean, default: false },
});

const emit = defineEmits(["fetch", "save", "drop-markdown"]);
const dragActive = ref(false);

const readonly = computed(() => props.mode === "view");
const dialogTitle = computed(() => {
  if (props.mode === "new") return "新增 Skill";
  if (props.mode === "view") return props.skill.name || "查看 Skill";
  return props.skill.name ? `编辑 Skill · ${props.skill.name}` : "编辑 Skill";
});

function dropMarkdown(event) {
  dragActive.value = false;
  if (!readonly.value) emit("drop-markdown", event);
}
</script>
