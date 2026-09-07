// AgentLibraryPanel.spec.js — 图片库面板：日分组 grid、hover 浮层、参考图缩略与事件载荷
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const agentLibraryMock = vi.fn();
vi.mock('../../src/api/index.js', () => ({
  agentLibrary: (...args) => agentLibraryMock(...args),
}));

import AgentLibraryPanel from '../../src/components/AgentLibraryPanel.vue';

const taskWithRef = {
  id: 'task-1',
  prompt: '特写，一本放大的日记本',
  model: 'gpt-image-2',
  origin: 'agent',
  status: 'completed',
  referencePaths: ['/refs/a.png'],
  createdAt: '2026-08-22T12:12:09Z',
  updatedAt: '2026-08-22T12:12:09Z',
  completedAt: '2026-08-22T12:12:09Z',
  outputs: [{ path: '/outputs/a.png', fileName: 'a.png', size: '2880x2880' }],
};
const taskNoRef = {
  ...taskWithRef,
  id: 'task-2',
  prompt: '画一个小狗',
  referencePaths: [],
  createdAt: '2026-08-22T09:00:00Z',
  updatedAt: '2026-08-22T09:00:00Z',
  completedAt: '2026-08-22T09:00:00Z',
  outputs: [{ path: '/outputs/b.png', fileName: 'b.png', size: '1024x1024' }],
};

function mountPanel(tasks) {
  agentLibraryMock.mockResolvedValue({ tasks, months: [] });
  return mount(AgentLibraryPanel);
}

beforeEach(() => {
  agentLibraryMock.mockReset();
});

describe('AgentLibraryPanel', () => {
  it('首次加载显示占位状态，失败后可重新加载', async () => {
    let fail;
    agentLibraryMock.mockReturnValueOnce(
      new Promise((_, reject) => {
        fail = reject;
      })
    );
    const wrapper = mount(AgentLibraryPanel);
    expect(wrapper.find('.library-loading-grid').exists()).toBe(true);
    expect(wrapper.text()).not.toContain('没有匹配的图片');
    fail(new Error('网络不可用'));
    await flushPromises();
    expect(wrapper.get('.image-library-empty').text()).toContain('图片库加载失败');
    agentLibraryMock.mockResolvedValueOnce({ tasks: [taskWithRef], months: [] });
    await wrapper.get('.image-library-empty button').trigger('click');
    await flushPromises();
    expect(wrapper.findAll('.library-image-card')).toHaveLength(1);
  });

  it('来源筛选为空时可清除筛选，损坏图片有可读占位', async () => {
    const wrapper = mountPanel([taskWithRef]);
    await flushPromises();
    await wrapper.get('select[aria-label="按来源筛选"]').setValue('direct');
    expect(wrapper.get('.image-library-empty').text()).toContain('没有匹配的图片');
    await wrapper.get('.image-library-empty button').trigger('click');
    await wrapper.get('.library-image-preview img').trigger('error');
    expect(wrapper.get('.library-image-preview').text()).toContain('图片暂时不可用');
    expect(wrapper.get('.library-image-caption').text()).toBe(taskWithRef.prompt);
  });

  it('原生来源选择框筛选图片', async () => {
    const wrapper = mountPanel([taskWithRef, { ...taskNoRef, origin: 'agent-direct' }]);
    await flushPromises();
    await wrapper.get('select[aria-label="按来源筛选"]').setValue('direct');
    expect(wrapper.findAll('.library-image-card')).toHaveLength(1);
    expect(wrapper.get('.library-image-preview img').attributes('alt')).toBe('b.png');
  });

  it('同一天的图片合并在一个 grid 中，一行展示', async () => {
    const wrapper = mountPanel([taskWithRef, taskNoRef]);
    await flushPromises();
    const grids = wrapper.findAll('.image-day-grid');
    expect(grids).toHaveLength(1);
    expect(grids[0].findAll('.library-image-card')).toHaveLength(2);
  });

  it('topbar 显示时间与模型尺寸，footbar 左侧显示参考图缩略', async () => {
    const wrapper = mountPanel([taskWithRef]);
    await flushPromises();
    const card = wrapper.get('.library-image-card');
    const topbar = card.get('.library-image-topbar');
    expect(topbar.text()).toContain('gpt-image-2');
    expect(topbar.text()).toContain('2880x2880');
    expect(card.findAll('.library-image-ref-thumb')).toHaveLength(1);
  });

  it('删除按钮在 footbar 中，不再有「再来一张」', async () => {
    const wrapper = mountPanel([taskWithRef]);
    await flushPromises();
    const card = wrapper.get('.library-image-card');
    expect(card.find('[aria-label="删除任务及图片"]').exists()).toBe(true);
    expect(card.find('[aria-label="再来一张"]').exists()).toBe(false);
  });

  it('删除与引用到 Agent 事件携带正确载荷', async () => {
    const wrapper = mountPanel([taskWithRef]);
    await flushPromises();
    const card = wrapper.get('.library-image-card');
    await card.get('[aria-label="删除任务及图片"]').trigger('click');
    expect(wrapper.emitted('delete-task')).toEqual([[taskWithRef]]);
    await card.get('[aria-label="引用到 Agent"]').trigger('click');
    expect(wrapper.emitted('reference-to-agent')).toEqual([
      [{ task: taskWithRef, output: taskWithRef.outputs[0] }],
    ]);
  });

  it('点击参考图缩略以参考图打开大图查看器', async () => {
    const wrapper = mountPanel([taskWithRef]);
    await flushPromises();
    const card = wrapper.get('.library-image-card');
    await card.get('.library-image-ref-thumb').trigger('click');

    const emitted = wrapper.emitted('preview-images');
    expect(emitted).toHaveLength(1);
    const { items, index } = emitted[0][0];
    expect(index).toBe(0);
    expect(items).toHaveLength(1);
    expect(items[0].path).toBe('/refs/a.png');
    expect(items[0].title).toBe('参考图 1/1');
    expect(items[0].prompt).toBe(taskWithRef.prompt);
    expect(items[0].task).toEqual(taskWithRef);
  });
});
