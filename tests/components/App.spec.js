import { flushPromises, shallowMount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../../src/App.vue';
import AgentWorkspace from '../../src/components/AgentWorkspace.vue';
import ApiSourceDialog from '../../src/components/dialogs/ApiSourceDialog.vue';
import TemplateEditorDialog from '../../src/components/dialogs/TemplateEditorDialog.vue';
import * as api from '../../src/api/index.js';

vi.mock('../../src/api/index.js', () => ({
  loadAppState: vi.fn(),
  listAgentSessions: vi.fn(),
  createAgentSession: vi.fn(),
  getAgentSession: vi.fn(),
  sendAgentMessage: vi.fn(),
  createAgentDirectImageTask: vi.fn(),
  referenceFromPath: vi.fn(),
  getTaskStatus: vi.fn(),
  queueSnapshot: vi.fn(),
}));
vi.mock('../../src/tauri', () => ({
  listenDragDrop: vi.fn(async () => () => {}),
  listenEvent: vi.fn(async () => () => {}),
  listenWindowState: vi.fn(async () => () => {}),
  restoreWindowState: vi.fn(async () => {}),
  openDialog: vi.fn(async () => ['/reference.png']),
  saveDialog: vi.fn(),
  convertFileSrc: (path) => path,
}));

const settings = {
  providers: [
    { id: 'chat-1', name: '对话', modelType: 'chat', imageModel: 'chat-model', apiKey: 'test' },
    {
      id: 'image-1',
      name: '绘画',
      modelType: 'image-gpt',
      imageModel: 'image-model',
      apiKey: 'test',
    },
  ],
};
const initialSession = {
  id: 'session-1',
  title: '第一条对话',
  updatedAt: '2026-09-08',
  messages: [],
};
const otherSession = {
  id: 'session-2',
  title: '另一条对话',
  updatedAt: '2026-09-07',
  messages: [],
};
const emptyQueue = { waiting: [], running: [], recent: [], workerActive: false };

beforeEach(() => {
  vi.resetAllMocks();
  window.__TAURI_INTERNALS__ = {};
  const storage = new Map();
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => storage.set(key, value),
    },
  });
  api.loadAppState.mockResolvedValue({ settings, history: [], templates: [], queue: emptyQueue });
  api.listAgentSessions.mockResolvedValue([initialSession, otherSession]);
  api.getAgentSession.mockImplementation(async (id) =>
    structuredClone(id === 'session-1' ? initialSession : otherSession)
  );
  api.createAgentSession.mockResolvedValue({ id: 'new-session', messages: [] });
  api.queueSnapshot.mockResolvedValue(emptyQueue);
  api.getTaskStatus.mockResolvedValue([]);
  api.referenceFromPath.mockResolvedValue({
    path: '/reference.png',
    fileName: 'reference.png',
    mimeType: 'image/png',
    dataUrl: 'data:image/png;base64,AA==',
  });
});

async function mountApp() {
  const wrapper = shallowMount(App, {
    global: {
      stubs: {
        AppShell: { template: '<div><slot /><slot name="footer" /><slot name="dialogs" /></div>' },
      },
    },
  });
  await flushPromises();
  return { wrapper, workspace: wrapper.findComponent(AgentWorkspace) };
}

describe('工作台发送与草稿', () => {
  it('配置缺少 Key 时保留原文和绘画模式', async () => {
    api.loadAppState.mockResolvedValueOnce({
      settings: { providers: settings.providers.map((provider) => ({ ...provider, apiKey: '' })) },
      history: [],
      templates: [],
      queue: emptyQueue,
    });
    const { wrapper, workspace } = await mountApp();
    workspace.vm.$emit('update:draft', '一只猫');
    workspace.vm.$emit('update:drawThisTurn', true);
    workspace.vm.$emit('send', { content: '一只猫', drawThisTurn: true });
    await flushPromises();
    expect(workspace.props('draft')).toBe('一只猫');
    expect(workspace.props('drawThisTurn')).toBe(true);
    expect(wrapper.findComponent(ApiSourceDialog).props('show')).toBe(true);
    expect(api.createAgentDirectImageTask).not.toHaveBeenCalled();
  });

  it('网络失败后恢复提示词及参考图', async () => {
    api.sendAgentMessage.mockRejectedValueOnce(new Error('网络连接失败'));
    const { workspace } = await mountApp();
    workspace.vm.$emit('add-reference');
    await flushPromises();
    workspace.vm.$emit('update:draft', '参考这张图');
    workspace.vm.$emit('send', { content: '参考这张图', drawThisTurn: false });
    await flushPromises();
    expect(workspace.props('draft')).toBe('参考这张图');
    expect(workspace.props('attachments')).toHaveLength(1);
    expect(workspace.props('busy')).toBe(false);
    expect(api.sendAgentMessage.mock.calls[0][3][0]).toMatchObject({ path: '/reference.png' });
    expect(api.sendAgentMessage.mock.calls[0][3][0]).not.toHaveProperty('dataUrl');
  });

  it('防止重复发送，异步回复不会切回旧对话或清空新草稿', async () => {
    let finish;
    api.sendAgentMessage.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        })
    );
    const { workspace } = await mountApp();
    workspace.vm.$emit('update:draft', '第一条消息');
    workspace.vm.$emit('send', { content: '第一条消息', drawThisTurn: false });
    workspace.vm.$emit('send', { content: '第一条消息', drawThisTurn: false });
    await flushPromises();
    expect(api.sendAgentMessage).toHaveBeenCalledTimes(1);
    expect(workspace.props('draft')).toBe('');
    workspace.vm.$emit('select', 'session-2');
    await flushPromises();
    workspace.vm.$emit('update:draft', '另一个想法');
    finish({
      ...initialSession,
      messages: [{ id: 'reply', role: 'assistant', content: '完成了' }],
    });
    await flushPromises();
    expect(workspace.props('currentSession').id).toBe('session-2');
    expect(workspace.props('draft')).toBe('另一个想法');
    expect(workspace.props('busy')).toBe(false);
  });

  it('切换对话会保存各自的草稿，模板引用正确填入 content', async () => {
    const { wrapper, workspace } = await mountApp();
    workspace.vm.$emit('update:draft', '第一份草稿');
    workspace.vm.$emit('select', 'session-2');
    await flushPromises();
    expect(workspace.props('draft')).toBe('');
    workspace.vm.$emit('update:draft', '第二份草稿');
    workspace.vm.$emit('select', 'session-1');
    await flushPromises();
    expect(workspace.props('draft')).toBe('第一份草稿');
    workspace.vm.$emit('add-to-template', { task: { prompt: '图片中的提示词' } });
    await flushPromises();
    expect(wrapper.findComponent(TemplateEditorDialog).props('template').content).toBe(
      '图片中的提示词'
    );
  });

  it('参考图读取期间切换对话，图片仍回到原来的草稿', async () => {
    let finish;
    api.referenceFromPath.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        })
    );
    const { workspace } = await mountApp();
    workspace.vm.$emit('drop-reference', ['/slow.png']);
    await flushPromises();
    workspace.vm.$emit('select', 'session-2');
    await flushPromises();
    finish({ path: '/slow.png', fileName: 'slow.png', dataUrl: 'data:image/png;base64,AA==' });
    await flushPromises();
    expect(workspace.props('attachments')).toEqual([]);
    workspace.vm.$emit('select', 'session-1');
    await flushPromises();
    expect(workspace.props('attachments')[0].path).toBe('/slow.png');
  });

  it('保留快照以外的历史图片，并携带图片预览操作需要的任务上下文', async () => {
    const task = {
      id: 'old-task',
      task_group_id: 'old-group',
      status: 'completed',
      prompt: '旧图片',
      outputs: [{ path: '/out.png', size: '1024x1024' }],
    };
    const session = {
      ...initialSession,
      messages: [
        {
          id: 'group',
          role: 'tool',
          taskGroup: { id: 'old-group', taskIds: [task.id], status: 'completed' },
        },
      ],
    };
    api.loadAppState.mockResolvedValueOnce({
      settings,
      history: [task],
      templates: [],
      queue: emptyQueue,
    });
    api.listAgentSessions.mockResolvedValueOnce([session]);
    api.getTaskStatus.mockResolvedValue([task]);
    const { workspace } = await mountApp();
    const group = workspace.props('messages')[0].taskGroup;
    expect(group.status).toBe('completed');
    expect(group.images[0]).toMatchObject({ path: '/out.png', prompt: '旧图片', task });
  });
});
