<template>
  <n-modal v-model:show="show" preset="card" title="引用 skill" class="skill-reference-modal">
    <div class="skill-reference-table-wrap">
      <table class="skill-reference-table">
        <thead>
          <tr>
            <th>Skill 名字</th>
            <th>备注</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="skill in visibleSkills" :key="skill.id">
            <td>
              <button type="button" @click="$emit('reference', skill)">{{ skill.name }}</button>
            </td>
            <td>
              <button type="button" @click="$emit('reference', skill)">{{ skill.notes || "—" }}</button>
            </td>
          </tr>
          <tr v-if="!visibleSkills.length">
            <td colspan="2" class="skill-empty-cell">没有可引用的 Skill</td>
          </tr>
        </tbody>
      </table>
    </div>
  </n-modal>
</template>

<script setup>
import { computed } from "vue";

const show = defineModel("show", { type: Boolean, default: false });
const props = defineProps({
  skills: { type: Array, default: () => [] },
});

defineEmits(["reference"]);

const visibleSkills = computed(() => props.skills.slice(0, 12));
</script>
