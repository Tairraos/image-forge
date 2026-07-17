<template>
  <section class="composer-column">
    <div class="control-surface">
      <n-form label-placement="top" :show-feedback="false">
        <div class="control-grid">
          <n-form-item label="提示词模式">
            <n-select v-model:value="form.promptMode" :options="promptModeOptions" size="small" />
          </n-form-item>
          <n-form-item label="分辨率">
            <n-select v-model:value="form.resolution" :options="resolutionOptions" size="small" />
          </n-form-item>
          <n-form-item label="比例">
            <n-select v-model:value="form.ratio" :options="ratioOptions" size="small" />
          </n-form-item>
          <n-form-item label="质量">
            <n-select v-model:value="form.quality" :options="qualityOptions" size="small" />
          </n-form-item>
          <n-form-item label="生图模型" class="model-form-item">
            <n-select
              v-model:value="form.providerId"
              :options="imageProviderOptions"
              :consistent-menu-width="false"
              :menu-props="modelSelectMenuProps"
              size="small"
              placeholder="选择生图模型"
            />
          </n-form-item>
          <n-form-item class="model-form-item">
            <template #label>
              <span class="model-form-label">
                对话模型
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <CircleAlert :size="14" class="model-help-icon" />
                  </template>
                  对话模型用于执行 skill
                </n-tooltip>
              </span>
            </template>
            <n-select
              v-model:value="form.chatProviderId"
              :options="chatProviderOptions"
              :consistent-menu-width="false"
              :menu-props="modelSelectMenuProps"
              size="small"
              placeholder="选择对话模型"
            />
          </n-form-item>
        </div>
      </n-form>
    </div>

    <div class="reference-strip">
      <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
        <img :src="item.previewUrl" :alt="item.fileName" />
        <button
          type="button"
          title="移除参考图"
          @click.stop="$emit('remove-reference', index)"
        >
          <X :size="14" />
        </button>
      </div>
      <ClipboardImageMenu v-slot="{ open }" @paste="$emit('paste-reference')">
        <button
          class="reference-add"
          :class="{ 'reference-drop-active': referenceDragActive }"
          data-reference-drop-target="workbench"
          type="button"
          title="点击添加，右键粘贴剪贴板图片"
          @dragover.prevent="$emit('reference-drag-over')"
          @dragleave="$emit('reference-drag-leave')"
          @drop.prevent="$emit('drop-reference', $event)"
          @click="$emit('add-reference')"
          @contextmenu="open"
        >
          <Plus :size="18" />
          <span>参考图</span>
        </button>
      </ClipboardImageMenu>
    </div>

    <div
      class="prompt-live-panel"
      :class="{ 'reference-drop-active': referenceDragActive }"
      data-reference-drop-target="workbench"
      @dragover.prevent="$emit('reference-drag-over')"
      @dragleave="$emit('reference-drag-leave')"
      @drop.prevent="$emit('drop-reference', $event)"
    >
      <div class="prompt-live-head">
        <div class="prompt-live-title">
          <span>提示词</span>
        </div>
        <small>{{ form.prompt.length }} 字</small>
      </div>
      <n-input
        ref="promptInput"
        v-model:value="form.prompt"
        type="textarea"
        class="prompt-input"
        :resizable="false"
        placeholder="写下你要生成的画面、风格、主体、光线和构图"
        @focus="handlePromptFocus"
        @blur="handlePromptBlur"
        @click="handlePromptCursor"
        @keyup="handlePromptCursor"
        @keydown="handlePromptKeydown"
        @select="handlePromptCursor"
        @update:value="scheduleMentionSync"
        @paste="$emit('prompt-paste', $event)"
      />
      <div v-if="mentionOpen" class="skill-mention-popup" role="listbox">
        <button
          v-for="(skill, index) in mentionSkills"
          :key="skill.id"
          type="button"
          role="option"
          :aria-selected="index === mentionIndex"
          :class="{ 'is-active': index === mentionIndex }"
          @mousedown.prevent="completeMention(skill)"
        >
          <span>{{ skill.name }}</span>
          <small v-if="skill.notes">{{ skill.notes }}</small>
        </button>
        <div v-if="!mentionSkills.length" class="skill-mention-empty">没有匹配的 Skill</div>
      </div>
      <div class="prompt-submit-row">
        <n-button size="small" secondary @click="$emit('clear-prompt')">清空</n-button>
        <n-button size="small" secondary @click="$emit('show-template')">
          引用模板
        </n-button>
        <n-button size="small" secondary @click="$emit('show-skill')">引用 skill</n-button>
        <n-button size="small" type="primary" :loading="submitting" @click="$emit('submit')">
          <template #icon><WandSparkles :size="17" /></template>
          开始生成
        </n-button>
      </div>
    </div>
  </section>
</template>

<script setup>
import { CircleAlert, Plus, WandSparkles, X } from "@lucide/vue";
import { computed, nextTick, ref } from "vue";
import ClipboardImageMenu from "./ClipboardImageMenu.vue";
import {
  promptModeOptions,
  qualityOptions,
  ratioOptions,
  resolutionOptions,
} from "../lib/options";

const props = defineProps({
  form: { type: Object, required: true },
  imageProviderOptions: { type: Array, default: () => [] },
  chatProviderOptions: { type: Array, default: () => [] },
  references: { type: Array, default: () => [] },
  submitting: { type: Boolean, default: false },
  referenceDragActive: { type: Boolean, default: false },
  skills: { type: Array, default: () => [] },
});

const emit = defineEmits([
  "submit",
  "show-template",
  "show-skill",
  "clear-prompt",
  "prompt-focus",
  "prompt-cursor",
  "prompt-paste",
  "paste-reference",
  "add-reference",
  "remove-reference",
  "reference-drag-over",
  "reference-drag-leave",
  "drop-reference",
]);

const promptInput = ref(null);
const modelSelectMenuProps = { class: "model-select-menu" };
const mentionRange = ref(null);
const mentionIndex = ref(0);
const promptFocused = ref(false);
let blurTimer = 0;

const mentionSkills = computed(() => {
  const query = mentionRange.value?.query?.toLowerCase() || "";
  return props.skills
    .filter((skill) => !query || String(skill.name || "").toLowerCase().includes(query))
    .slice(0, 12);
});
const mentionOpen = computed(() => Boolean(mentionRange.value));

function nativeTextarea() {
  return promptInput.value?.$el?.querySelector("textarea") || null;
}

function handlePromptFocus(event) {
  window.clearTimeout(blurTimer);
  promptFocused.value = true;
  emit("prompt-focus", event);
  syncMention(event?.target);
}

function handlePromptBlur() {
  blurTimer = window.setTimeout(() => {
    promptFocused.value = false;
    mentionRange.value = null;
  }, 100);
}

function handlePromptCursor(event) {
  emit("prompt-cursor", event);
  syncMention(event?.target);
}

function scheduleMentionSync() {
  nextTick(() => syncMention(nativeTextarea()));
}

function syncMention(target) {
  if (typeof target?.selectionStart !== "number") return;
  const cursor = target.selectionStart;
  const value = props.form.prompt;
  const before = value.slice(0, cursor);
  const match = /(^|\s)@([^\s@]*)$/u.exec(before);
  if (!match) {
    mentionRange.value = null;
    return;
  }
  let end = cursor;
  while (end < value.length && !/\s/u.test(value[end])) end += 1;
  mentionRange.value = {
    start: cursor - match[2].length - 1,
    end,
    query: match[2],
  };
  mentionIndex.value = Math.min(mentionIndex.value, Math.max(0, mentionSkills.value.length - 1));
}

function handlePromptKeydown(event) {
  if (!mentionOpen.value) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    mentionIndex.value = (mentionIndex.value + 1) % Math.max(1, mentionSkills.value.length);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    mentionIndex.value = (mentionIndex.value - 1 + Math.max(1, mentionSkills.value.length)) % Math.max(1, mentionSkills.value.length);
  } else if (event.key === "Enter" && mentionSkills.value.length) {
    event.preventDefault();
    completeMention(mentionSkills.value[mentionIndex.value]);
  } else if (event.key === "Escape") {
    event.preventDefault();
    mentionRange.value = null;
  }
}

function completeMention(skill) {
  const range = mentionRange.value;
  if (!range) return;
  const text = `@${skill.name} `;
  props.form.prompt = `${props.form.prompt.slice(0, range.start)}${text}${props.form.prompt.slice(range.end)}`;
  const cursor = range.start + text.length;
  mentionRange.value = null;
  nextTick(() => {
    const textarea = nativeTextarea();
    textarea?.focus();
    textarea?.setSelectionRange(cursor, cursor);
    emit("prompt-cursor", { target: textarea });
  });
}
</script>
