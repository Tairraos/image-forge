<template>
  <section
    class="agent-workspace"
    :class="{ 'sidebar-open': sidebarOpen }"
    @keydown.esc="sidebarOpen && closeSidebar()"
  >
    <button
      v-if="sidebarOpen"
      type="button"
      class="sidebar-backdrop"
      aria-label="收起侧栏"
      @click="closeSidebar"
    ></button>
    <aside
      id="workspace-sidebar"
      ref="sidebar"
      class="function-bar"
      @keydown.esc.prevent.stop="closeSidebar"
    >
      <div class="function-bar-head">
        <div class="function-bar-titlebar" data-tauri-drag-region="deep"></div>
        <div class="function-bar-brand">
          <span class="brand-symbol"><Aperture :size="23" :stroke-width="1.7" /></span>
          <span>Image Forge</span>
        </div>
        <nav class="function-bar-nav" aria-label="功能栏">
          <button
            type="button"
            class="function-bar-item"
            @click="
              panel = 'chat';
              sidebarOpen = false;
              $emit('create');
            "
          >
            <SquarePen :size="17" />
            <span>新对话</span>
          </button>
          <button
            type="button"
            class="function-bar-item"
            :class="{ active: panel === 'library' }"
            @click="
              panel = panel === 'library' ? 'chat' : 'library';
              sidebarOpen = false;
            "
          >
            <Images :size="17" />
            <span>图片库</span>
          </button>
        </nav>
      </div>

      <div class="function-bar-sessions">
        <div class="sidebar-section-label">最近对话</div>
        <div v-if="sessions.length" class="agent-session-list">
          <div
            v-for="session in sessions"
            :key="session.id"
            class="agent-session-row"
            :class="{ active: session.id === currentSession?.id }"
          >
            <div
              class="agent-session-item"
              role="button"
              tabindex="0"
              :title="session.title || '新对话'"
              @click="selectSession(session.id)"
              @keydown.enter="selectSession(session.id)"
              @keydown.space.prevent="selectSession(session.id)"
            >
              <input
                v-if="renaming?.id === session.id && renaming?.where === 'bar'"
                :ref="(el) => setRenameInput(el)"
                v-model="titleDraft"
                class="agent-session-rename"
                aria-label="对话标题"
                @click.stop
                @keydown.enter.prevent="commitRename"
                @keydown.esc.prevent="cancelRename"
                @blur="commitRename"
              />
              <span
                v-else
                class="agent-session-title"
                @dblclick.stop="startRename(session, 'bar')"
                >{{ session.title || '新对话' }}</span
              >
            </div>
            <button
              type="button"
              class="agent-session-delete"
              title="删除对话"
              aria-label="删除对话"
              @click.stop="$emit('delete-session', session.id)"
            >
              <Trash2 :size="14" />
            </button>
          </div>
        </div>
        <p v-else class="function-bar-empty">还没有对话</p>
      </div>
      <footer class="function-bar-footer">
        <button
          type="button"
          class="function-bar-item"
          @click="
            $emit('open-settings');
            sidebarOpen = false;
          "
        >
          <Settings2 :size="17" />
          <span>设置</span>
        </button>
        <button
          type="button"
          class="icon-button theme-toggle"
          :title="theme === 'dark' ? '切换到浅色模式' : '切换到深色模式'"
          :aria-label="theme === 'dark' ? '切换到浅色模式' : '切换到深色模式'"
          @click="$emit('toggle-theme')"
        >
          <Sun v-if="theme === 'dark'" :size="17" />
          <Moon v-else :size="17" />
        </button>
      </footer>
    </aside>

    <div class="info-area">
      <header class="info-area-titlebar" data-tauri-drag-region="deep">
        <button
          ref="sidebarToggle"
          type="button"
          class="icon-button mobile-sidebar-toggle"
          :aria-label="sidebarOpen ? '收起侧栏' : '展开侧栏'"
          :aria-expanded="sidebarOpen"
          aria-controls="workspace-sidebar"
          @click="toggleSidebar"
        >
          <PanelLeft :size="18" />
        </button>
        <template v-if="panel === 'chat'">
          <input
            v-if="renaming?.id === currentSession?.id && renaming?.where === 'top'"
            :ref="(el) => setRenameInput(el)"
            v-model="titleDraft"
            class="info-area-title-rename"
            aria-label="对话标题"
            data-tauri-drag-region="none"
            @keydown.enter.prevent="commitRename"
            @keydown.esc.prevent="cancelRename"
            @blur="commitRename"
          />
          <strong
            v-else
            class="info-area-title"
            :class="{ 'is-empty': !currentSession }"
            data-tauri-drag-region="none"
            @click="currentSession && startRename(currentSession, 'top')"
            >{{ currentSession ? currentSession.title || '新对话' : '开始新对话' }}</strong
          >
        </template>
        <strong v-else class="info-area-title">图片库</strong>
      </header>
      <AgentLibraryPanel
        v-if="panel === 'library'"
        :version="agentLibraryVersion"
        @preview-images="$emit('preview-images', $event)"
        @delete-task="$emit('delete-task', $event)"
        @download-output="$emit('download-output', $event)"
        @reveal-output="$emit('reveal-output', $event)"
        @reference-to-agent="handleReferenceToAgent"
        @add-to-template="$emit('add-to-template', $event)"
      />
      <template v-else>
        <AgentMessageList
          :messages="messages"
          :busy="busy"
          :stream-text="streamText"
          :tool-status-text="toolStatusText"
          :answers="answers"
          @open-task-group="$emit('open-task-group', $event)"
          @preview-images="$emit('preview-images', $event)"
          @cancel-task-group="$emit('cancel-task-group', $event)"
          @retry-task-group="$emit('retry-task-group', $event)"
          @redraw-task-group="$emit('redraw-task-group', $event)"
          @retry="$emit('retry', $event)"
          @update-answer="$emit('update-answer', $event)"
          @answer-questions="$emit('answer-questions', $event)"
        />
        <AgentComposer
          :provider-id="providerId"
          :image-provider-id="imageProviderId"
          :busy="busy"
          :attachments="attachments"
          :ratio="ratio"
          :resolution="resolution"
          :prefill-prompt="prefillPrompt"
          :templates="templates"
          :template-fill-busy="templateFillBusy"
          @send="$emit('send', $event)"
          @stop="$emit('stop')"
          @add-reference="$emit('add-reference')"
          @paste-reference="$emit('paste-reference', $event)"
          @drop-reference="$emit('drop-reference', $event)"
          @remove-attachment="$emit('remove-attachment', $event)"
          @apply-template="$emit('apply-template', $event)"
          @fill-template="$emit('fill-template', $event)"
          @update:ratio="$emit('update:ratio', $event)"
          @update:resolution="$emit('update:resolution', $event)"
        />
      </template>
    </div>
  </section>
</template>

<script setup>
import { nextTick, ref } from 'vue';
import { Aperture, Images, Moon, PanelLeft, Settings2, SquarePen, Sun, Trash2 } from '@lucide/vue';
import AgentLibraryPanel from './AgentLibraryPanel.vue';
import AgentComposer from './AgentComposer.vue';
import AgentMessageList from './AgentMessageList.vue';

defineProps({
  theme: { type: String, default: 'light' },
  sessions: { type: Array, default: () => [] },
  currentSession: { type: Object, default: null },
  messages: { type: Array, default: () => [] },
  providerId: { type: String, default: '' },
  imageProviderId: { type: String, default: '' },
  busy: Boolean,
  streamText: { type: String, default: '' },
  attachments: { type: Array, default: () => [] },
  toolStatusText: { type: String, default: '' },
  answers: { type: Object, default: () => ({}) },
  agentLibraryVersion: { type: Number, default: 0 },
  ratio: { type: String, default: '1:1' },
  resolution: { type: String, default: 'standard' },
  prefillPrompt: { type: String, default: '' },
  templates: { type: Array, default: () => [] },
  templateFillBusy: Boolean,
});
const emit = defineEmits([
  'create',
  'select',
  'send',
  'stop',
  'add-reference',
  'remove-attachment',
  'open-task-group',
  'preview-images',
  'delete-task',
  'download-output',
  'reveal-output',
  'cancel-task-group',
  'retry-task-group',
  'retry',
  'paste-reference',
  'drop-reference',
  'update-answer',
  'answer-questions',
  'delete-session',
  'open-settings',
  'toggle-theme',
  'rename-session',
  'apply-template',
  'fill-template',
  'update:ratio',
  'update:resolution',
  'reference-to-agent',
  'add-to-template',
  'redraw-task-group',
]);

const panel = ref('chat');
const sidebarOpen = ref(false);
const sidebar = ref(null);
const sidebarToggle = ref(null);
const renaming = ref(null);
const titleDraft = ref('');
let renameInputEl = null;

async function toggleSidebar() {
  if (sidebarOpen.value) return closeSidebar();
  sidebarOpen.value = true;
  await nextTick();
  sidebar.value?.querySelector('button')?.focus();
}

function closeSidebar() {
  sidebarOpen.value = false;
  sidebarToggle.value?.focus();
}

function handleReferenceToAgent(payload) {
  panel.value = 'chat';
  emit('reference-to-agent', payload);
}

function selectSession(id) {
  sidebarOpen.value = false;
  panel.value = 'chat';
  emit('select', id);
}

function setRenameInput(el) {
  renameInputEl = el;
}

function startRename(session, where) {
  renaming.value = { id: session.id, where };
  titleDraft.value = session.title || '新对话';
  nextTick(() => renameInputEl?.select?.());
}

function commitRename() {
  const target = renaming.value;
  if (!target) return;
  renaming.value = null;
  const title = titleDraft.value.trim();
  if (title) {
    emit('rename-session', { sessionId: target.id, title });
  }
}

function cancelRename() {
  renaming.value = null;
}
</script>
