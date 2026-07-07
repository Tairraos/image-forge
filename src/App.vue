<template>
  <n-config-provider :theme-overrides="themeOverrides" component-size="small">
    <n-global-style />
    <main class="app">
      <header class="topbar">
        <div class="brand">
          <img :src="logoUrl" alt="Image Forge" />
          <div>
            <h1>Image Forge</h1>
            <p>{{ dataDir || "加载应用数据目录" }}</p>
          </div>
        </div>

        <div class="topbar-center">
          <n-select
            v-model:value="form.providerId"
            :options="providerOptions"
            size="small"
            class="provider-select"
          />
          <n-tag size="small" :type="queue.running.length ? 'warning' : 'success'">
            {{ queue.running.length }} 运行 · {{ queue.waiting.length }} 排队
          </n-tag>
        </div>

        <div class="topbar-actions">
          <n-button quaternary size="small" @click="showApiDialog = true">
            <template #icon><Settings :size="16" /></template>
            API 源
          </n-button>
          <n-button quaternary size="small" @click="showGalleryDrawer = true">
            <template #icon><Image :size="16" /></template>
            图库
          </n-button>
          <n-button quaternary size="small" @click="showTemplateDrawer = true">
            <template #icon><BookOpen :size="16" /></template>
            模板
          </n-button>
          <n-button quaternary size="small" @click="showSnippetModal = true">
            <template #icon><Layers :size="16" /></template>
            片段
          </n-button>
          <n-button quaternary size="small" @click="showSettingsDialog = true">
            <template #icon><SlidersHorizontal :size="16" /></template>
            设置
          </n-button>
        </div>
      </header>

      <section class="status-row">
        <span class="status-pill" :data-tone="statusTone">{{ statusText }}</span>
        <span>当前 API：{{ activeProvider?.name || "未配置" }} · Images API</span>
        <span v-if="activeProvider && !activeProvider.apiKey" class="warn-text">API Key 未设置</span>
      </section>

      <section class="workspace" :style="workspaceStyle">
        <aside class="queue-column">
          <div class="section-head">
            <div>
              <h2>生图队列</h2>
              <p>{{ queue.workerActive ? "后台执行中" : "等待任务" }}</p>
            </div>
            <n-button circle quaternary size="small" @click="refreshAll">
              <template #icon><RotateCcw :size="16" /></template>
            </n-button>
          </div>

          <n-tabs type="segment" animated>
            <n-tab-pane name="queue" tab="队列">
              <div class="task-stack">
                <TaskCard
                  v-for="task in runningAndWaiting"
                  :key="task.id"
                  :task="task"
                  :selected="selectedTaskId === task.id"
                  @select="selectedTaskId = task.id"
                  @cancel="cancelTask"
                  @retry="retryTask"
                  @promote="promoteTask"
                />
                <div v-if="!runningAndWaiting.length" class="empty-panel compact">队列为空</div>
              </div>
            </n-tab-pane>
            <n-tab-pane name="history" tab="历史">
              <n-input
                v-model:value="historyQuery"
                size="small"
                clearable
                placeholder="搜索提示词或任务 ID"
              >
                <template #prefix><Search :size="15" /></template>
              </n-input>
              <div class="task-stack history-stack">
                <TaskCard
                  v-for="task in filteredHistory"
                  :key="task.id"
                  :task="task"
                  :selected="selectedTaskId === task.id"
                  @select="selectedTaskId = task.id"
                  @cancel="cancelTask"
                  @retry="retryTask"
                  @promote="promoteTask"
                />
                <div v-if="!filteredHistory.length" class="empty-panel compact">没有匹配历史</div>
              </div>
            </n-tab-pane>
          </n-tabs>
        </aside>

        <div
          class="panel-resizer"
          role="separator"
          aria-label="调整队列和结果预览宽度"
          @pointerdown="startPanelResize('queue', $event)"
        ></div>

        <aside class="result-column">
          <div class="section-head">
            <div>
              <h2>结果预览</h2>
              <p>{{ selectedTask ? shortId(selectedTask.id) : "未选择任务" }}</p>
            </div>
            <n-button
              circle
              quaternary
              size="small"
              :disabled="!selectedTask"
              @click="showTaskDetail = true"
            >
              <template #icon><Eye :size="16" /></template>
            </n-button>
          </div>

          <div v-if="selectedTask" class="selected-task">
            <n-tag :type="statusType(selectedTask.status)" size="small">
              {{ statusLabel(selectedTask.status) }}
            </n-tag>
            <strong>{{ selectedTask.prompt || "空提示词" }}</strong>
            <span>{{ selectedTask.providerName }} · {{ selectedTask.model }}</span>
            <p v-if="selectedTask.error">{{ selectedTask.error }}</p>
          </div>

          <div v-if="currentOutputs.length" class="output-grid">
            <article v-for="output in currentOutputs" :key="output.path" class="output-card">
              <img :src="fileUrl(output.path)" :alt="output.fileName" />
              <footer>
                <span>{{ output.size || selectedTask?.params?.size }} · {{ output.outputFormat }}</span>
                <div>
                  <n-button size="tiny" secondary @click="reveal(output.path)">定位</n-button>
                  <n-button size="tiny" secondary @click="saveOutputToGallery(output)">入图库</n-button>
                </div>
              </footer>
            </article>
          </div>
          <div v-else class="empty-panel">选择已完成任务后预览结果</div>
        </aside>

        <div
          class="panel-resizer"
          role="separator"
          aria-label="调整结果预览和工作台宽度"
          @pointerdown="startPanelResize('composer', $event)"
        ></div>

        <section class="composer-column">
          <div class="section-head">
            <div>
              <h2>生成工作台</h2>
              <p>OpenAI 兼容 Images API</p>
            </div>
            <n-button size="small" type="primary" :loading="submitting" @click="submitTask">
              <template #icon><WandSparkles :size="17" /></template>
              加入队列
            </n-button>
          </div>

          <div class="control-surface">
            <n-form label-placement="top" :show-feedback="false">
              <div class="control-grid">
                <n-form-item label="尺寸">
                  <n-select v-model:value="form.size" :options="sizeOptions" size="small" />
                </n-form-item>
                <n-form-item label="质量">
                  <n-select v-model:value="form.quality" :options="qualityOptions" size="small" />
                </n-form-item>
                <n-form-item label="格式">
                  <n-select v-model:value="form.outputFormat" :options="formatOptions" size="small" />
                </n-form-item>
                <n-form-item label="数量">
                  <n-input-number v-model:value="form.count" :min="1" :max="8" size="small" />
                </n-form-item>
                <n-form-item label="背景">
                  <n-select v-model:value="form.background" :options="backgroundOptions" size="small" />
                </n-form-item>
                <n-form-item label="压缩">
                  <n-input-number
                    v-model:value="form.outputCompression"
                    size="small"
                    clearable
                    :min="0"
                    :max="100"
                    placeholder="可空"
                  />
                </n-form-item>
                <n-form-item label="保真">
                  <n-select v-model:value="form.inputFidelity" :options="fidelityOptions" size="small" />
                </n-form-item>
              </div>
            </n-form>
          </div>

          <n-input
            ref="promptInput"
            v-model:value="form.prompt"
            type="textarea"
            class="prompt-input"
            :resizable="false"
            placeholder="写下你要生成的画面、风格、主体、光线和构图"
          />

          <div class="quick-bar">
            <n-button size="small" secondary @click="showTemplateDrawer = true">
              <template #icon><BookOpen :size="15" /></template>
              插入模板
            </n-button>
            <n-button size="small" secondary @click="showSnippetModal = true">
              <template #icon><Layers :size="15" /></template>
              插入片段
            </n-button>
            <n-button size="small" secondary @click="showGalleryDrawer = true">
              <template #icon><Image :size="15" /></template>
              从图库选图
            </n-button>
            <n-button size="small" secondary @click="addReferenceImages">
              <template #icon><Upload :size="15" /></template>
              添加参考图
            </n-button>
          </div>

          <div class="reference-strip">
            <div v-for="(item, index) in references" :key="item.path" class="reference-tile">
              <img :src="item.previewUrl" :alt="item.fileName" />
              <button type="button" title="移除参考图" @click="references.splice(index, 1)">
                <XCircle :size="16" />
              </button>
            </div>
            <button class="reference-add" type="button" @click="addReferenceImages">
              <Plus :size="18" />
              <span>参考图</span>
            </button>
          </div>
        </section>
      </section>

      <ApiSourceDialog
        v-model:show="showApiDialog"
        :settings="settings"
        @save="saveApiSettings"
      />

      <n-drawer v-model:show="showGalleryDrawer" :width="560" placement="right">
        <n-drawer-content title="图库">
          <template #header-extra>
            <n-button size="small" type="primary" @click="addGalleryImages">
              <template #icon><Plus :size="15" /></template>
              添加
            </n-button>
          </template>
          <n-input v-model:value="galleryQuery" clearable placeholder="搜索图库" class="drawer-search">
            <template #prefix><Search :size="15" /></template>
          </n-input>
          <div class="gallery-grid">
            <article v-for="item in filteredGallery" :key="item.id" class="gallery-card">
              <img :src="fileUrl(item.path)" :alt="item.name" />
              <div>
                <strong>{{ item.name }}</strong>
                <span>{{ item.category }}</span>
                <p v-if="item.note">{{ item.note }}</p>
              </div>
              <footer>
                <n-button size="tiny" secondary @click="useGalleryItem(item)">使用</n-button>
                <n-button size="tiny" quaternary @click="editGalleryItem(item)">编辑</n-button>
                <n-button size="tiny" quaternary type="error" @click="deleteGallery(item.id)">删</n-button>
              </footer>
            </article>
          </div>
        </n-drawer-content>
      </n-drawer>

      <n-drawer v-model:show="showTemplateDrawer" :width="620" placement="right">
        <n-drawer-content title="提示词模板">
          <template #header-extra>
            <n-button size="small" type="primary" @click="newTemplateFromPrompt">
              <template #icon><Plus :size="15" /></template>
              新建
            </n-button>
          </template>
          <n-input v-model:value="templateQuery" clearable placeholder="搜索模板" class="drawer-search">
            <template #prefix><Search :size="15" /></template>
          </n-input>
          <div class="template-list">
            <article v-for="template in filteredTemplates" :key="template.id" class="template-card">
              <header>
                <strong>{{ template.title }}</strong>
                <n-tag size="small">{{ template.category }}</n-tag>
              </header>
              <p>{{ template.content }}</p>
              <footer>
                <n-button size="tiny" secondary @click="insertTemplate(template, false)">插入</n-button>
                <n-button size="tiny" secondary @click="insertTemplate(template, true)">替换</n-button>
                <n-button size="tiny" quaternary @click="editTemplate(template)">编辑</n-button>
                <n-button size="tiny" quaternary type="error" @click="deletePromptTemplate(template.id)">删</n-button>
              </footer>
            </article>
          </div>
        </n-drawer-content>
      </n-drawer>

      <n-modal v-model:show="showSnippetModal" preset="card" title="提示词片段" class="wide-modal">
        <div class="snippet-toolbar">
          <n-input v-model:value="snippetQuery" clearable placeholder="搜索片段">
            <template #prefix><Search :size="15" /></template>
          </n-input>
          <n-button size="small" type="primary" @click="newSnippet">
            <template #icon><Plus :size="15" /></template>
            新建
          </n-button>
        </div>
        <div class="snippet-grid">
          <article v-for="snippet in filteredSnippets" :key="snippet.id" class="snippet-card">
            <header>
              <strong>~{{ snippet.tag }}</strong>
              <n-tag size="small">{{ snippet.category }}</n-tag>
            </header>
            <p>{{ snippet.content }}</p>
            <footer>
              <n-button size="tiny" secondary @click="insertText(snippet.content)">插入</n-button>
              <n-button size="tiny" quaternary @click="editSnippet(snippet)">编辑</n-button>
              <n-button size="tiny" quaternary type="error" @click="deletePromptSnippet(snippet.id)">删</n-button>
            </footer>
          </article>
        </div>
      </n-modal>

      <n-modal v-model:show="showSettingsDialog" preset="card" title="系统设置" class="settings-modal">
        <n-form label-placement="top" :show-feedback="false">
          <n-form-item label="输出目录">
            <n-input v-model:value="settingsDraft.outputDir" placeholder="默认写入应用数据目录">
              <template #suffix>
                <n-button size="small" text @click="chooseOutputDir"><FolderOpen :size="16" /></n-button>
              </template>
            </n-input>
          </n-form-item>
          <div class="switch-list">
            <label>
              <span>自动启动队列</span>
              <n-switch v-model:value="settingsDraft.autoStartQueue" />
            </label>
            <label>
              <span>失败自动重试一次</span>
              <n-switch v-model:value="settingsDraft.autoRetry" />
            </label>
            <label>
              <span>应用内完成提示</span>
              <n-switch v-model:value="settingsDraft.notificationsEnabled" />
            </label>
          </div>
        </n-form>
        <template #footer>
          <n-button size="small" @click="showSettingsDialog = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveStorageSettings">
            <template #icon><Save :size="16" /></template>
            保存设置
          </n-button>
        </template>
      </n-modal>

      <n-modal v-model:show="showSnippetEditor" preset="card" title="编辑片段" class="editor-modal">
        <n-form label-placement="top" :show-feedback="false">
          <div class="two-col">
            <n-form-item label="短标签">
              <n-input v-model:value="snippetDraft.tag" placeholder="例如 portrait" />
            </n-form-item>
            <n-form-item label="分类">
              <n-input v-model:value="snippetDraft.category" />
            </n-form-item>
          </div>
          <n-form-item label="标题">
            <n-input v-model:value="snippetDraft.title" />
          </n-form-item>
          <n-form-item label="内容">
            <n-input v-model:value="snippetDraft.content" type="textarea" :autosize="{ minRows: 5 }" />
          </n-form-item>
        </n-form>
        <template #footer>
          <n-button size="small" @click="showSnippetEditor = false">取消</n-button>
          <n-button size="small" type="primary" @click="savePromptSnippet">保存片段</n-button>
        </template>
      </n-modal>

      <n-modal v-model:show="showTemplateEditor" preset="card" title="编辑模板" class="editor-modal">
        <n-form label-placement="top" :show-feedback="false">
          <div class="two-col">
            <n-form-item label="标题">
              <n-input v-model:value="templateDraft.title" />
            </n-form-item>
            <n-form-item label="分类">
              <n-input v-model:value="templateDraft.category" />
            </n-form-item>
          </div>
          <n-form-item label="内容">
            <n-input v-model:value="templateDraft.content" type="textarea" :autosize="{ minRows: 8 }" />
          </n-form-item>
          <n-form-item label="备注">
            <n-input v-model:value="templateDraft.notes" />
          </n-form-item>
          <n-checkbox v-model:checked="templateDraft.favorite">收藏</n-checkbox>
        </n-form>
        <template #footer>
          <n-button size="small" @click="showTemplateEditor = false">取消</n-button>
          <n-button size="small" type="primary" @click="savePromptTemplate">保存模板</n-button>
        </template>
      </n-modal>

      <n-modal v-model:show="showGalleryEditor" preset="card" title="编辑图库条目" class="editor-modal">
        <n-form label-placement="top" :show-feedback="false">
          <n-form-item label="名称">
            <n-input v-model:value="galleryDraft.name" />
          </n-form-item>
          <n-form-item label="分类">
            <n-input v-model:value="galleryDraft.category" />
          </n-form-item>
          <n-form-item label="备注">
            <n-input v-model:value="galleryDraft.note" type="textarea" :autosize="{ minRows: 3 }" />
          </n-form-item>
        </n-form>
        <template #footer>
          <n-button size="small" @click="showGalleryEditor = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveGalleryEdit">保存图库</n-button>
        </template>
      </n-modal>

      <n-modal v-model:show="showTaskDetail" preset="card" title="任务详情" class="wide-modal">
        <div v-if="selectedTask" class="detail-grid">
          <div>
            <h3>{{ selectedTask.providerName }}</h3>
            <p>{{ selectedTask.prompt }}</p>
            <n-descriptions :column="2" size="small" bordered>
              <n-descriptions-item label="状态">{{ statusLabel(selectedTask.status) }}</n-descriptions-item>
              <n-descriptions-item label="模型">{{ selectedTask.model }}</n-descriptions-item>
              <n-descriptions-item label="尺寸">{{ selectedTask.params.size }}</n-descriptions-item>
              <n-descriptions-item label="质量">{{ selectedTask.params.quality }}</n-descriptions-item>
              <n-descriptions-item label="数量">{{ selectedTask.params.count }}</n-descriptions-item>
              <n-descriptions-item label="格式">{{ selectedTask.params.outputFormat }}</n-descriptions-item>
            </n-descriptions>
          </div>
          <div class="detail-output-list">
            <img v-for="output in selectedTask.outputs" :key="output.path" :src="fileUrl(output.path)" :alt="output.fileName" />
          </div>
        </div>
      </n-modal>
    </main>
  </n-config-provider>
</template>

<script setup>
import { computed, defineComponent, h, onMounted, onUnmounted, reactive, ref, resolveComponent, watch } from "vue";
import {
  ArrowUp,
  BookOpen,
  Copy,
  Eye,
  FolderOpen,
  Image,
  Layers,
  Plus,
  RotateCcw,
  Save,
  Search,
  Settings,
  SlidersHorizontal,
  Trash2,
  Upload,
  WandSparkles,
  XCircle,
} from "@lucide/vue";
import logoUrl from "./assets/app-icon.png";
import { convertFileSrc, invoke, openDialog } from "./tauri";

const themeOverrides = {
  common: {
    primaryColor: "#9B7BEE",
    primaryColorHover: "#AA8CF6",
    primaryColorPressed: "#7F62D5",
    primaryColorSuppl: "#B9A7F8",
    infoColor: "#8FC7FF",
    successColor: "#67B99A",
    warningColor: "#F3A76D",
    errorColor: "#E8788F",
    borderRadius: "7px",
    fontFamily:
      "Nunito, Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
  },
  Button: {
    borderRadiusMedium: "7px",
    borderRadiusSmall: "6px",
  },
  Card: {
    borderRadius: "8px",
  },
};

const statusText = ref("启动中");
const statusTone = ref("busy");
const dataDir = ref("");
const settings = ref(defaultSettings());
const settingsDraft = reactive(defaultSettings());
const history = ref([]);
const queue = reactive({ waiting: [], running: [], recent: [], workerActive: false, updatedAt: "" });
const gallery = reactive({ items: [], categories: [] });
const snippets = ref([]);
const templates = ref([]);
const references = ref([]);
const selectedTaskId = ref("");
const submitting = ref(false);
const historyQuery = ref("");
const galleryQuery = ref("");
const snippetQuery = ref("");
const templateQuery = ref("");

const showApiDialog = ref(false);
const showGalleryDrawer = ref(false);
const showTemplateDrawer = ref(false);
const showSnippetModal = ref(false);
const showSettingsDialog = ref(false);
const showSnippetEditor = ref(false);
const showTemplateEditor = ref(false);
const showGalleryEditor = ref(false);
const showTaskDetail = ref(false);

const snippetDraft = reactive(emptySnippet());
const templateDraft = reactive(emptyTemplate());
const galleryDraft = reactive({ id: "", name: "", category: "", note: "" });

const form = reactive({
  providerId: "",
  prompt: "",
  size: "1024x1024",
  quality: "auto",
  outputFormat: "png",
  count: 1,
  background: "",
  outputCompression: null,
  inputFidelity: "",
  moderation: "",
});

const panelSizes = reactive({
  queue: 310,
  composer: 420,
});

const workspaceStyle = computed(() => ({
  gridTemplateColumns: `${panelSizes.queue}px 10px minmax(0, 1fr) 10px ${panelSizes.composer}px`,
}));

const sizeOptions = [
  { label: "1024 x 1024", value: "1024x1024" },
  { label: "1536 x 1024", value: "1536x1024" },
  { label: "1024 x 1536", value: "1024x1536" },
  { label: "Auto", value: "auto" },
];
const qualityOptions = ["auto", "high", "medium", "low"].map((value) => ({ label: value, value }));
const formatOptions = ["png", "jpeg", "webp"].map((value) => ({ label: value, value }));
const backgroundOptions = [
  { label: "默认", value: "" },
  { label: "auto", value: "auto" },
  { label: "transparent", value: "transparent" },
  { label: "opaque", value: "opaque" },
];
const fidelityOptions = [
  { label: "默认", value: "" },
  { label: "high", value: "high" },
  { label: "low", value: "low" },
];

let pollTimer = 0;

const providerOptions = computed(() =>
  settings.value.providers.map((provider) => ({
    label: provider.name,
    value: provider.id,
  })),
);

const activeProvider = computed(() =>
  settings.value.providers.find((provider) => provider.id === form.providerId)
  || settings.value.providers.find((provider) => provider.id === settings.value.activeProviderId)
  || settings.value.providers[0],
);

const runningAndWaiting = computed(() => [...queue.running, ...queue.waiting]);

const filteredHistory = computed(() => {
  const query = historyQuery.value.trim().toLowerCase();
  const items = history.value;
  if (!query) return items;
  return items.filter((task) =>
    [task.id, task.prompt, task.providerName, task.model]
      .filter(Boolean)
      .join(" ")
      .toLowerCase()
      .includes(query),
  );
});

const selectedTask = computed(() =>
  history.value.find((task) => task.id === selectedTaskId.value)
  || queue.running.find((task) => task.id === selectedTaskId.value)
  || queue.waiting.find((task) => task.id === selectedTaskId.value)
  || null,
);

const currentOutputs = computed(() => selectedTask.value?.outputs || []);

const filteredGallery = computed(() => {
  const query = galleryQuery.value.trim().toLowerCase();
  if (!query) return gallery.items;
  return gallery.items.filter((item) =>
    [item.name, item.category, item.note].join(" ").toLowerCase().includes(query),
  );
});

const filteredSnippets = computed(() => {
  const query = snippetQuery.value.trim().toLowerCase();
  if (!query) return snippets.value;
  return snippets.value.filter((item) =>
    [item.tag, item.title, item.category, item.content].join(" ").toLowerCase().includes(query),
  );
});

const filteredTemplates = computed(() => {
  const query = templateQuery.value.trim().toLowerCase();
  if (!query) return templates.value;
  return templates.value.filter((item) =>
    [item.title, item.shortTitle, item.category, item.content, item.notes].join(" ").toLowerCase().includes(query),
  );
});

watch(showSettingsDialog, (show) => {
  if (show) Object.assign(settingsDraft, deepClone(settings.value));
});

onMounted(async () => {
  await refreshAll();
  pollTimer = window.setInterval(refreshQueueOnly, 1600);
});

onUnmounted(() => {
  window.clearInterval(pollTimer);
});

async function refreshAll() {
  try {
    const state = await invoke("load_app_state");
    applyState(state);
    setStatus("就绪", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function refreshQueueOnly() {
  try {
    const snapshot = await invoke("queue_snapshot");
    applyQueue(snapshot);
  } catch {
    // Polling should stay quiet; explicit actions still surface errors.
  }
}

function startPanelResize(target, event) {
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = panelSizes[target];

  const move = (moveEvent) => {
    const total = Math.max(0, window.innerWidth - 28 - 20);
    const resultMin = 430;
    if (target === "queue") {
      const max = Math.max(260, total - panelSizes.composer - resultMin);
      panelSizes.queue = clamp(startWidth + moveEvent.clientX - startX, 260, Math.min(500, max));
    } else {
      const max = Math.max(360, total - panelSizes.queue - resultMin);
      panelSizes.composer = clamp(startWidth - (moveEvent.clientX - startX), 360, Math.min(560, max));
    }
  };

  const stop = () => {
    document.body.classList.remove("resizing-panels");
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", stop);
    window.removeEventListener("pointercancel", stop);
  };

  document.body.classList.add("resizing-panels");
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", stop);
  window.addEventListener("pointercancel", stop);
}

function applyState(state) {
  settings.value = normalizeSettingsForUi(state.settings || defaultSettings());
  form.providerId = form.providerId || settings.value.activeProviderId;
  history.value = state.history || [];
  applyQueue(state.queue || {});
  Object.assign(gallery, state.gallery || { items: [], categories: [] });
  snippets.value = state.snippets || [];
  templates.value = state.templates || [];
  dataDir.value = state.dataDir || "";
  ensureSelectedTask();
}

function applyQueue(snapshot) {
  queue.waiting = snapshot.waiting || [];
  queue.running = snapshot.running || [];
  queue.recent = snapshot.recent || [];
  queue.workerActive = Boolean(snapshot.workerActive);
  queue.updatedAt = snapshot.updatedAt || "";
  if (snapshot.recent) {
    history.value = snapshot.recent;
  }
  ensureSelectedTask();
}

function ensureSelectedTask() {
  if (selectedTask.value) return;
  const next = queue.running[0] || queue.waiting[0] || history.value[0];
  selectedTaskId.value = next?.id || "";
}

async function submitTask() {
  if (!form.prompt.trim()) {
    setStatus("提示词不能为空", "error");
    return;
  }
  if (!activeProvider.value?.apiKey) {
    setStatus("请先在 API 源里填写 API Key", "error");
    showApiDialog.value = true;
    return;
  }
  submitting.value = true;
  try {
    const request = {
      providerId: form.providerId || settings.value.activeProviderId,
      prompt: form.prompt,
      referencePaths: references.value.map((item) => item.path),
      size: form.size,
      quality: form.quality,
      outputFormat: form.outputFormat,
      count: Number(form.count) || 1,
      background: form.background || "",
      outputCompression: form.outputCompression ?? null,
      inputFidelity: form.inputFidelity || "",
      moderation: form.moderation || "",
    };
    const task = await invoke("enqueue_generation", { request });
    selectedTaskId.value = task.id;
    setStatus("任务已加入队列", "ok");
    await refreshQueueOnly();
  } catch (error) {
    setStatus(String(error), "error");
  } finally {
    submitting.value = false;
  }
}

async function addReferenceImages() {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    for (const path of paths) {
      const preview = await invoke("reference_from_path", { path });
      if (!references.value.some((item) => item.path === preview.path)) {
        references.value.push({ ...preview, previewUrl: preview.dataUrl });
      }
    }
    if (paths.length) setStatus(`已添加 ${paths.length} 张参考图`, "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function cancelTask(task) {
  try {
    await invoke("cancel_task", { taskId: task.id });
    setStatus("任务已取消", "ok");
    await refreshQueueOnly();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function retryTask(task) {
  try {
    await invoke("retry_task", { taskId: task.id });
    selectedTaskId.value = task.id;
    setStatus("任务已重新排队", "ok");
    await refreshQueueOnly();
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function promoteTask(task) {
  try {
    const snapshot = await invoke("promote_task", { taskId: task.id });
    applyQueue(snapshot);
    setStatus("已移到队首", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function saveApiSettings(nextSettings) {
  try {
    const saved = await invoke("save_settings", { settings: nextSettings });
    settings.value = normalizeSettingsForUi(saved);
    form.providerId = settings.value.activeProviderId;
    showApiDialog.value = false;
    setStatus("API 源已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function chooseOutputDir() {
  const selected = await openDialog({
    directory: true,
    multiple: false,
    defaultPath: settingsDraft.outputDir || undefined,
    canCreateDirectories: true,
  });
  if (typeof selected === "string") {
    settingsDraft.outputDir = selected;
  }
}

async function saveStorageSettings() {
  try {
    const saved = await invoke("save_settings", { settings: deepClone(settingsDraft) });
    settings.value = normalizeSettingsForUi(saved);
    showSettingsDialog.value = false;
    setStatus("设置已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function addGalleryImages() {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    for (const path of paths) {
      const next = await invoke("add_gallery_item", {
        payload: { path, category: "默认", name: fileName(path), note: "" },
      });
      Object.assign(gallery, next);
    }
    if (paths.length) setStatus("图片已加入图库", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function useGalleryItem(item) {
  if (!references.value.some((refItem) => refItem.path === item.path)) {
    references.value.push({
      path: item.path,
      fileName: item.name,
      mimeType: item.mimeType,
      previewUrl: fileUrl(item.path),
    });
  }
  setStatus("图库图片已加入参考图", "ok");
}

function editGalleryItem(item) {
  Object.assign(galleryDraft, {
    id: item.id,
    name: item.name,
    category: item.category,
    note: item.note,
  });
  showGalleryEditor.value = true;
}

async function saveGalleryEdit() {
  try {
    const next = await invoke("update_gallery_item", { payload: deepClone(galleryDraft) });
    Object.assign(gallery, next);
    showGalleryEditor.value = false;
    setStatus("图库已更新", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deleteGallery(id) {
  if (!window.confirm("删除这个图库条目？")) return;
  try {
    const next = await invoke("delete_gallery_item", { itemId: id });
    Object.assign(gallery, next);
    setStatus("图库条目已删除", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function saveOutputToGallery(output) {
  try {
    const next = await invoke("add_gallery_item", {
      payload: { path: output.path, category: "生成结果", name: output.fileName, note: selectedTask.value?.prompt || "" },
    });
    Object.assign(gallery, next);
    setStatus("结果已保存到图库", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function newSnippet() {
  Object.assign(snippetDraft, emptySnippet());
  showSnippetEditor.value = true;
}

function editSnippet(snippet) {
  Object.assign(snippetDraft, deepClone(snippet));
  showSnippetEditor.value = true;
}

async function savePromptSnippet() {
  try {
    snippets.value = await invoke("save_snippet", { snippet: deepClone(snippetDraft) });
    showSnippetEditor.value = false;
    setStatus("片段已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deletePromptSnippet(id) {
  try {
    snippets.value = await invoke("delete_snippet", { snippetId: id });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function newTemplateFromPrompt() {
  Object.assign(templateDraft, {
    ...emptyTemplate(),
    title: form.prompt.trim().slice(0, 24) || "新模板",
    content: form.prompt,
  });
  showTemplateEditor.value = true;
}

function editTemplate(template) {
  Object.assign(templateDraft, deepClone(template));
  showTemplateEditor.value = true;
}

async function savePromptTemplate() {
  try {
    templates.value = await invoke("save_template", { template: deepClone(templateDraft) });
    showTemplateEditor.value = false;
    setStatus("模板已保存", "ok");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function deletePromptTemplate(id) {
  try {
    templates.value = await invoke("delete_template", { templateId: id });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

async function insertTemplate(template, replace) {
  if (replace) {
    form.prompt = template.content;
  } else {
    insertText(template.content);
  }
  templates.value = await invoke("mark_template_used", { templateId: template.id });
}

function insertText(text) {
  const glue = form.prompt.trim() ? "\n" : "";
  form.prompt = `${form.prompt}${glue}${text}`;
}

async function reveal(path) {
  try {
    await invoke("reveal_path", { path });
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function fileUrl(path) {
  return `${convertFileSrc(path)}?v=${encodeURIComponent(path)}`;
}

function setStatus(message, tone = "idle") {
  statusText.value = message;
  statusTone.value = tone;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function statusType(status) {
  if (status === "completed") return "success";
  if (status === "failed" || status === "cancelled") return "error";
  if (status === "running" || status === "cancelling") return "warning";
  return "info";
}

function statusLabel(status) {
  return {
    queued: "排队",
    running: "运行",
    completed: "完成",
    failed: "失败",
    cancelled: "取消",
    cancelling: "取消中",
  }[status] || status;
}

function shortId(id) {
  return String(id || "").slice(0, 8);
}

function fileName(path) {
  return String(path || "").split(/[\\/]/).pop() || "image";
}

function deepClone(value) {
  return JSON.parse(JSON.stringify(value));
}

function defaultSettings() {
  return {
    activeProviderId: "default",
    providers: [defaultProvider()],
    outputDir: null,
    inputDir: null,
    autoStartQueue: true,
    autoRetry: false,
    notificationsEnabled: true,
  };
}

function defaultProvider(index = 1) {
  return {
    id: index === 1 ? "default" : `provider-${Date.now()}`,
    name: index === 1 ? "Default" : `Provider ${index}`,
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    imageModel: "gpt-image-2",
    imagesConcurrency: 4,
    enabled: true,
    notes: "",
  };
}

function normalizeSettingsForUi(value) {
  const next = { ...defaultSettings(), ...value };
  next.providers = Array.isArray(next.providers) && next.providers.length ? next.providers : [defaultProvider()];
  next.activeProviderId = next.activeProviderId || next.providers[0].id;
  return next;
}

function emptySnippet() {
  return { id: "", tag: "", title: "", category: "常用", content: "", createdAt: "", updatedAt: "" };
}

function emptyTemplate() {
  return {
    id: "",
    title: "",
    shortTitle: "",
    category: "常用",
    content: "",
    notes: "",
    tags: [],
    favorite: false,
    usageCount: 0,
    modelHint: "",
    createdAt: "",
    updatedAt: "",
  };
}

const TaskCard = defineComponent({
  name: "TaskCard",
  props: {
    task: { type: Object, required: true },
    selected: { type: Boolean, default: false },
  },
  emits: ["select", "cancel", "retry", "promote"],
  setup(props, { emit }) {
    return () =>
      h(
        "article",
        {
          class: ["task-card", props.selected && "selected"],
          onClick: () => emit("select", props.task),
        },
        [
          h("header", [
            h("strong", props.task.prompt || "空提示词"),
            h("span", shortId(props.task.id)),
          ]),
          h("p", `${props.task.providerName || "API"} · ${props.task.model || ""}`),
          h("footer", [
            h(
              "span",
              { class: ["task-status", props.task.status] },
              statusLabel(props.task.status),
            ),
            h("div", { class: "task-actions" }, [
              props.task.status === "queued"
                ? h(
                    "button",
                    {
                      type: "button",
                      title: "移到队首",
                      onClick: (event) => {
                        event.stopPropagation();
                        emit("promote", props.task);
                      },
                    },
                    [h(ArrowUp, { size: 14 })],
                  )
                : null,
              props.task.status === "queued" || props.task.status === "running"
                ? h(
                    "button",
                    {
                      type: "button",
                      title: "取消任务",
                      onClick: (event) => {
                        event.stopPropagation();
                        emit("cancel", props.task);
                      },
                    },
                    [h(XCircle, { size: 14 })],
                  )
                : null,
              props.task.status === "failed" || props.task.status === "cancelled"
                ? h(
                    "button",
                    {
                      type: "button",
                      title: "重试任务",
                      onClick: (event) => {
                        event.stopPropagation();
                        emit("retry", props.task);
                      },
                    },
                    [h(RotateCcw, { size: 14 })],
                  )
                : null,
            ]),
          ]),
        ],
      );
  },
});

const ApiSourceDialog = defineComponent({
  name: "ApiSourceDialog",
  props: {
    show: { type: Boolean, default: false },
    settings: { type: Object, required: true },
  },
  emits: ["update:show", "save"],
  setup(props, { emit }) {
    const draft = reactive(defaultSettings());
    const selectedId = ref("default");
    const showImport = ref(false);
    const importText = ref("");
    const importError = ref("");

    watch(
      () => props.show,
      (show) => {
        if (!show) return;
        Object.assign(draft, normalizeSettingsForUi(deepClone(props.settings)));
        selectedId.value = draft.activeProviderId;
      },
      { immediate: true },
    );

    const selectedProvider = computed(() =>
      draft.providers.find((provider) => provider.id === selectedId.value) || draft.providers[0],
    );

    function addProvider() {
      const provider = defaultProvider(draft.providers.length + 1);
      provider.id = `provider-${Date.now()}`;
      draft.providers.push(provider);
      selectedId.value = provider.id;
      draft.activeProviderId = provider.id;
    }

    function copyProvider() {
      const source = selectedProvider.value;
      if (!source) return;
      const provider = deepClone(source);
      provider.id = `${source.id}-copy-${Date.now()}`;
      provider.name = `${source.name} Copy`;
      draft.providers.push(provider);
      selectedId.value = provider.id;
    }

    function importProviders() {
      importError.value = "";
      let value;
      try {
        value = JSON.parse(importText.value);
      } catch (error) {
        importError.value = `JSON 解析失败：${error.message}`;
        return;
      }
      if (!value || typeof value !== "object" || Array.isArray(value)) {
        importError.value = "请粘贴对象格式的 API 配置 JSON";
        return;
      }

      const imported = [];
      for (const [key, item] of Object.entries(value)) {
        if (!item || typeof item !== "object") {
          importError.value = `「${key}」不是有效配置`;
          return;
        }
        imported.push({
          ...defaultProvider(draft.providers.length + imported.length + 1),
          id: providerIdFromImportKey(key),
          name: providerNameFromImportKey(key),
          baseUrl: item.openAiBaseUrl || item.baseUrl || "",
          apiKey: item.openAiApiKey || item.apiKey || "",
          imageModel: item.openAiModelId || item.imageModel || "gpt-image-2",
        });
      }
      if (!imported.length) {
        importError.value = "没有可导入的 API 源";
        return;
      }

      for (const provider of imported) {
        const existing = draft.providers.findIndex((item) => item.id === provider.id);
        if (existing >= 0) {
          draft.providers[existing] = provider;
        } else {
          draft.providers.push(provider);
        }
      }
      selectedId.value = imported[imported.length - 1].id;
      draft.activeProviderId = selectedId.value;
      showImport.value = false;
      importText.value = "";
    }

    function deleteProvider() {
      if (draft.providers.length <= 1) return;
      draft.providers = draft.providers.filter((provider) => provider.id !== selectedId.value);
      selectedId.value = draft.providers[0].id;
      draft.activeProviderId = selectedId.value;
    }

    function moveProvider(offset) {
      const index = draft.providers.findIndex((provider) => provider.id === selectedId.value);
      const nextIndex = index + offset;
      if (index < 0 || nextIndex < 0 || nextIndex >= draft.providers.length) return;
      const [item] = draft.providers.splice(index, 1);
      draft.providers.splice(nextIndex, 0, item);
    }

    function save() {
      draft.activeProviderId = selectedId.value;
      emit("save", deepClone(draft));
    }

    return () =>
      h(
        resolveNaive("n-modal"),
        {
          show: props.show,
          "onUpdate:show": (value) => emit("update:show", value),
          preset: "card",
          title: "API 源管理",
          class: "api-modal",
        },
        {
          default: () => [
              h("div", { class: "api-manager" }, [
                h("aside", { class: "provider-list" }, [
                  h("div", { class: "provider-list-actions" }, [
                    h(
                      resolveNaive("n-button"),
                      { size: "small", type: "primary", onClick: addProvider },
                      { icon: () => h(Plus, { size: 15 }), default: () => "新增" },
                    ),
                    h(
                      resolveNaive("n-button"),
                      { size: "small", secondary: true, onClick: () => (showImport.value = true) },
                      { icon: () => h(Upload, { size: 15 }), default: () => "导入" },
                    ),
                    h(
                      resolveNaive("n-button"),
                      { size: "small", secondary: true, onClick: copyProvider },
                      { icon: () => h(Copy, { size: 15 }), default: () => "复制" },
                    ),
                  ]),
                  draft.providers.map((provider) =>
                    h(
                      "button",
                      {
                        type: "button",
                        class: ["provider-card", selectedId.value === provider.id && "active"],
                        onClick: () => {
                          selectedId.value = provider.id;
                          draft.activeProviderId = provider.id;
                        },
                      },
                      [
                        h("strong", provider.name),
                        h("span", `Images API · ${provider.imageModel}`),
                        h("small", provider.apiKey ? "API Key 已保存" : "未填写 API Key"),
                      ],
                    ),
                  ),
                ]),
                selectedProvider.value
                  ? h("section", { class: "provider-editor" }, [
                      h("div", { class: "provider-editor-head" }, [
                        h("h3", "Provider"),
                        h("div", [
                          h(
                            resolveNaive("n-button"),
                            { size: "tiny", quaternary: true, onClick: () => moveProvider(-1) },
                            { icon: () => h(ArrowUp, { size: 14 }) },
                          ),
                          h(
                            resolveNaive("n-button"),
                            { size: "tiny", quaternary: true, type: "error", onClick: deleteProvider },
                            { icon: () => h(Trash2, { size: 14 }) },
                          ),
                        ]),
                      ]),
                      h(resolveNaive("n-form"), { labelPlacement: "top", showFeedback: false }, () => [
                        h("div", { class: "two-col" }, [
                          h(resolveNaive("n-form-item"), { label: "名称" }, () =>
                            h(resolveNaive("n-input"), {
                              value: selectedProvider.value.name,
                              "onUpdate:value": (value) => (selectedProvider.value.name = value),
                            }),
                          ),
                          h(resolveNaive("n-form-item"), { label: "ID" }, () =>
                            h(resolveNaive("n-input"), {
                              value: selectedProvider.value.id,
                              "onUpdate:value": (value) => {
                                selectedProvider.value.id = value;
                                selectedId.value = value;
                              },
                            }),
                          ),
                        ]),
                        h(resolveNaive("n-form-item"), { label: "Base URL" }, () =>
                          h(resolveNaive("n-input"), {
                            value: selectedProvider.value.baseUrl,
                            "onUpdate:value": (value) => (selectedProvider.value.baseUrl = value),
                          }),
                        ),
                        h(resolveNaive("n-form-item"), { label: "API Key" }, () =>
                          h(resolveNaive("n-input"), {
                            value: selectedProvider.value.apiKey,
                            type: "password",
                            showPasswordOn: "click",
                            "onUpdate:value": (value) => (selectedProvider.value.apiKey = value),
                          }),
                        ),
                        h("div", { class: "two-col" }, [
                          h(resolveNaive("n-form-item"), { label: "图像模型" }, () =>
                            h(resolveNaive("n-input"), {
                              value: selectedProvider.value.imageModel,
                              "onUpdate:value": (value) => (selectedProvider.value.imageModel = value),
                            }),
                          ),
                          h(resolveNaive("n-form-item"), { label: "并发" }, () =>
                            h(resolveNaive("n-input-number"), {
                              value: selectedProvider.value.imagesConcurrency,
                              min: 1,
                              max: 32,
                              "onUpdate:value": (value) => (selectedProvider.value.imagesConcurrency = value || 1),
                            }),
                          ),
                        ]),
                        h(resolveNaive("n-form-item"), { label: "备注" }, () =>
                          h(resolveNaive("n-input"), {
                            value: selectedProvider.value.notes,
                            type: "textarea",
                            autosize: { minRows: 3 },
                            "onUpdate:value": (value) => (selectedProvider.value.notes = value),
                          }),
                        ),
                      ]),
                    ])
                  : null,
              ]),
              h(
                resolveNaive("n-modal"),
                {
                  show: showImport.value,
                  "onUpdate:show": (value) => (showImport.value = value),
                  preset: "card",
                  title: "导入 API 源",
                  class: "editor-modal",
                },
                {
                  default: () => [
                    h(resolveNaive("n-input"), {
                      value: importText.value,
                      type: "textarea",
                      autosize: { minRows: 12 },
                      placeholder: "粘贴 JSON 配置",
                      "onUpdate:value": (value) => (importText.value = value),
                    }),
                    importError.value ? h("p", { class: "import-error" }, importError.value) : null,
                  ],
                  footer: () => [
                    h(resolveNaive("n-button"), { size: "small", onClick: () => (showImport.value = false) }, () => "取消"),
                    h(resolveNaive("n-button"), { size: "small", type: "primary", onClick: importProviders }, () => "导入"),
                  ],
                },
              ),
          ],
          footer: () => [
            h(resolveNaive("n-button"), { size: "small", onClick: () => emit("update:show", false) }, () => "取消"),
            h(resolveNaive("n-button"), { size: "small", type: "primary", onClick: save }, () => "保存 API 源"),
          ],
        },
      );
  },
});

function resolveNaive(name) {
  return resolveComponent(
    name
      .split("-")
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(""),
  );
}

function providerNameFromImportKey(key) {
  return String(key).split("-")[0].trim() || "Imported";
}

function providerIdFromImportKey(key) {
  return String(key)
    .split("-")
    .slice(0, 2)
    .join("-")
    .replace(/\([^)]*\)/g, "")
    .replace(/[^A-Za-z0-9-]/g, "")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "") || `provider-${Date.now()}`;
}
</script>
