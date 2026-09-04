// AgentLibraryPanel.spec.js — 图片库面板：日分组 grid、hover 浮层、参考图缩略与事件载荷
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const agentLibraryMock = vi.fn();
vi.mock('../../src/api/index.js', () => ({
  agentLibrary: (...args) => agentLibraryMock(...args),
}));

import AgentLibraryPanel from '../../src/components/AgentLibraryPanel.vue';

const globalStubs = {
  stubs: {
    'n-tooltip': { template: '<span><slot name="trigger" /></span>' },
    'n-popselect': { template: '<span><slot /></span>' },
  },
};

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
  return mount(AgentLibraryPanel, { global: globalStubs });
}

beforeEach(() => {
  agentLibraryMock.mockReset();
});

describe('AgentLibraryPanel', () => {
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
});
