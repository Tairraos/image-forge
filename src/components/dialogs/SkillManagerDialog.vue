<template>
  <n-modal v-model:show="show" preset="card" title="Skill 维护" class="skill-manager-modal">
    <div class="skill-manager">
      <div class="skill-toolbar">
        <n-input v-model:value="query" clearable placeholder="搜索 Skill 名称或内容">
          <template #prefix><Search :size="15" /></template>
        </n-input>
        <n-button size="small" type="primary" @click="$emit('create')">
          <template #icon><Plus :size="15" /></template>
          新增
        </n-button>
      </div>

      <div class="skill-table-wrap">
        <table class="skill-table">
          <thead>
            <tr>
              <th>Skill 名称</th>
              <th>来源</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="skill in skills" :key="skill.id">
              <td>
                <button type="button" class="skill-name-button" @click="$emit('view', skill)">
                  {{ skill.name }}
                </button>
              </td>
              <td class="skill-source-cell" :title="skill.sourceUrl || '本地录入'">
                {{ skill.sourceUrl || "本地录入" }}
              </td>
              <td>
                <div class="skill-table-actions">
                  <n-button size="tiny" secondary @click="$emit('edit', skill)">编辑</n-button>
                  <n-button size="tiny" quaternary type="error" @click="$emit('delete', skill.id)">
                    删除
                  </n-button>
                </div>
              </td>
            </tr>
            <tr v-if="!skills.length">
              <td colspan="3" class="skill-empty-cell">没有 Skill</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { Plus, Search } from "@lucide/vue";

const show = defineModel("show", { type: Boolean, default: false });
const query = defineModel("query", { type: String, default: "" });

defineProps({
  skills: { type: Array, default: () => [] },
});

defineEmits(["create", "view", "edit", "delete"]);
</script>
