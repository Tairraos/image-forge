<template>
  <n-modal v-model:show="show" preset="card" title="使用 Skill" class="skill-reference-modal">
    <div class="skill-reference-layout">
      <div class="skill-reference-toolbar">
        <n-input v-model:value="query" clearable placeholder="搜索 Skill 名称或内容">
          <template #prefix><Search :size="15" /></template>
        </n-input>
        <n-select
          :value="selectedSkillId"
          :options="skillOptions"
          placeholder="选择 Skill"
          :disabled="!skillOptions.length"
          @update:value="selectSkill"
        />
      </div>

      <div class="skill-reference-editors">
        <n-input
          :value="skillContent"
          type="textarea"
          class="skill-reference-content"
          :autosize="{ minRows: 16, maxRows: 16 }"
          :resizable="false"
          readonly
          placeholder="选择 Skill 后显示 Markdown 内容"
        />
        <n-input
          v-model:value="promptContent"
          type="textarea"
          class="skill-reference-prompt"
          :autosize="{ minRows: 16, maxRows: 16 }"
          :resizable="false"
          placeholder="输入要交给 Skill 处理的画面需求"
        />
      </div>
    </div>

    <template #footer>
      <div class="skill-reference-footer">
        <n-select
          :value="chatProviderId"
          :options="chatProviderOptions"
          size="small"
          placement="top-start"
          class="reference-chat-select"
          placeholder="选择对话模型"
          :disabled="!chatProviderOptions.length"
          @update:value="$emit('update:chat-provider-id', $event)"
        />
        <n-button size="small" secondary :loading="filling" @click="$emit('ai-fill')">
          AI 生成
        </n-button>
        <n-button size="small" type="primary" @click="$emit('insert')">引用</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { Search } from "@lucide/vue";
import { computed } from "vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });
const promptContent = defineModel("promptContent", { type: String, default: "" });

const props = defineProps({
  skills: { type: Array, default: () => [] },
  selectedSkillId: { type: String, default: "" },
  skillContent: { type: String, default: "" },
  chatProviderId: { type: String, default: "" },
  chatProviderOptions: { type: Array, default: () => [] },
  filling: { type: Boolean, default: false },
});

const emit = defineEmits([
  "select-skill",
  "update:chat-provider-id",
  "ai-fill",
  "insert",
]);

const skillOptions = computed(() =>
  props.skills.map((skill) => ({
    label: skill.name,
    value: skill.id,
  })),
);

function selectSkill(skillId) {
  const skill = props.skills.find((item) => item.id === skillId);
  if (skill) emit("select-skill", skill);
}
</script>
