<template>
  <n-modal
    v-model:show="show"
    preset="card"
    title="Skill 执行中"
    class="skill-run-modal"
    :mask-closable="!busy"
    :close-on-esc="!busy"
  >
    <div class="skill-run-layout">
      <section class="skill-run-summary">
        <div>
          <span class="skill-run-label">当前 Skill</span>
          <strong>{{ skillName || "未指定" }}</strong>
        </div>
        <div>
          <span class="skill-run-label">用户提示词</span>
          <p>{{ originalPrompt || "（空）" }}</p>
        </div>
      </section>

      <section class="skill-run-status" :data-mode="responseMode">
        <strong>{{ statusText || "Skill 正在准备…" }}</strong>
        <small v-if="responseMode === 'stream'">流式响应中 · 已等待 {{ elapsedLabel }}</small>
        <small v-else-if="responseMode === 'non-stream'">
          非流式模型 · 已等待 {{ elapsedLabel }} / {{ timeoutLabel }}
        </small>
        <small v-else>正在检测模型响应方式…</small>
      </section>

      <section v-if="questions.length" class="skill-run-questions">
        <div class="skill-run-question-head">
          <strong>Skill 还需要补充信息</strong>
          <small>补充完整后会继续规划并自动开始生成</small>
        </div>
        <n-form label-placement="top" :show-feedback="false">
          <n-form-item
            v-for="question in questions"
            :key="question.key"
            :label="question.label"
          >
            <n-input
              :value="answers[question.key] || ''"
              type="textarea"
              :autosize="{ minRows: 2, maxRows: 4 }"
              :resizable="false"
              :placeholder="question.placeholder || '请输入补充信息'"
              @update:value="$emit('update-answer', { key: question.key, value: $event })"
            />
          </n-form-item>
        </n-form>
      </section>

      <section v-if="preview" class="skill-run-preview">
        <div class="skill-run-question-head">
          <strong>实时响应</strong>
          <small>仅用于过程反馈，不会写回主提示词框</small>
        </div>
        <n-input
          :value="preview"
          type="textarea"
          :autosize="{ minRows: 10, maxRows: 14 }"
          :resizable="false"
          readonly
        />
      </section>
    </div>

    <template #footer>
      <div class="skill-run-footer">
        <n-button size="small" :disabled="busy" @click="$emit('cancel')">关闭</n-button>
        <n-button
          v-if="questions.length"
          size="small"
          type="primary"
          :disabled="busy || !canContinue"
          @click="$emit('continue')"
        >
          继续
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { computed } from "vue";

const show = defineModel("show", { type: Boolean, default: false });

const props = defineProps({
  skillName: { type: String, default: "" },
  originalPrompt: { type: String, default: "" },
  statusText: { type: String, default: "" },
  responseMode: { type: String, default: "pending" },
  busy: { type: Boolean, default: false },
  elapsedSeconds: { type: Number, default: 0 },
  timeoutSeconds: { type: Number, default: 180 },
  questions: { type: Array, default: () => [] },
  answers: { type: Object, default: () => ({}) },
  preview: { type: String, default: "" },
});

defineEmits(["update-answer", "continue", "cancel"]);

const elapsedLabel = computed(() => formatSeconds(props.elapsedSeconds));
const timeoutLabel = computed(() => formatSeconds(props.timeoutSeconds));
const canContinue = computed(() =>
  props.questions.every((question) => String(props.answers[question.key] || "").trim()),
);

function formatSeconds(value) {
  const total = Math.max(0, Number(value) || 0);
  const minutes = String(Math.floor(total / 60)).padStart(2, "0");
  const seconds = String(total % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}
</script>
