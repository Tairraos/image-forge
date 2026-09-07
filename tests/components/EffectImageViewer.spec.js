// EffectImageViewer.spec.js — 大图查看器：标题条、展开面板、参考图叠层与动作事件
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import EffectImageViewer from '../../src/components/dialogs/EffectImageViewer.vue';

const task = {
  id: 'task-1',
  prompt: '特写，一本放大的日记本',
  referencePaths: ['/refs/a.png', '/refs/b.png'],
};

const taskItem = {
  path: '/outputs/a.png',
  fileName: 'a.png',
  title: '特写，一本放大的日记本',
  prompt: '特写，一本放大的日记本',
  revisedPrompt: 'A close-up of an open diary on a wooden desk',
  referencePaths: ['/refs/a.png', '/refs/b.png'],
  time: '2026-08-22T12:12:09Z',
  model: 'gpt-image-2',
  size: '2880x2880',
  task,
};

function mountViewer(extraProps = {}) {
  return mount(EffectImageViewer, {
    props: { show: true, items: [taskItem], ...extraProps },
  });
}

describe('EffectImageViewer', () => {
  it('在多张图片之间循环切换', async () => {
    const wrapper = mountViewer({
      items: [
        { path: '/tmp/one.png', title: '第一张' },
        { path: '/tmp/two.png', title: '第二张' },
      ],
    });
    expect(wrapper.get('img').attributes('alt')).toBe('第一张');
    await wrapper.get("button[aria-label='下一张']").trigger('click');
    expect(wrapper.get('img').attributes('alt')).toBe('第二张');
    await wrapper.get("button[aria-label='上一张']").trigger('click');
    expect(wrapper.get('img').attributes('alt')).toBe('第一张');
  });

  it('标题条显示提示词和图片信息，保留明确的关闭按钮', () => {
    const wrapper = mountViewer();
    const rows = wrapper.findAll('.viewer-prompt-row');
    expect(rows).toHaveLength(2);
    expect(rows[0].text()).toBe('特写，一本放大的日记本');
    expect(rows[1].text()).toBe('A close-up of an open diary on a wooden desk');
    expect(wrapper.get('.viewer-title-info').text()).toContain('gpt-image-2');
    expect(wrapper.get('.viewer-title-info').text()).toContain('2880x2880');
    expect(wrapper.find('[aria-label="关闭预览"]').exists()).toBe(true);
  });

  it('只有原始提示词时标题条单行显示', () => {
    const wrapper = mountViewer({
      items: [{ path: '/x.png', title: 'T', prompt: '只有一条', task }],
    });
    expect(wrapper.findAll('.viewer-prompt-row')).toHaveLength(1);
    expect(wrapper.get('.viewer-prompt-row').text()).toBe('只有一条');
  });

  it('点击标题条向下展开：上方完整提示词，底部按钮与 info 常驻', async () => {
    const wrapper = mountViewer();
    await wrapper.get('.viewer-title-prompts').trigger('click');

    const panel = wrapper.get('.viewer-expand-panel');
    const prompts = panel.get('.viewer-expand-prompts').text();
    expect(prompts).toContain('特写，一本放大的日记本');
    expect(prompts).toContain('A close-up of an open diary on a wooden desk');
    expect(panel.findAll('.library-image-actions button')).toHaveLength(6);
    expect(panel.get('.viewer-title-info').text()).toContain('gpt-image-2');

    await panel.trigger('click');
    expect(wrapper.find('.viewer-expand-panel').exists()).toBe(true);
    await wrapper.get('.viewer-title-prompts').trigger('click');
    expect(wrapper.find('.viewer-expand-panel').exists()).toBe(false);
  });

  it('展开时点击遮罩只收起面板，不关闭查看器', async () => {
    const wrapper = mountViewer();
    await wrapper.get('.viewer-title-prompts').trigger('click');
    await wrapper.get('.effect-image-viewer-stage').trigger('click');
    expect(wrapper.find('.viewer-expand-panel').exists()).toBe(false);
    expect(wrapper.emitted('update:show')).toBeUndefined();
  });

  it('点击参考图缩略放大叠层，点击叠层任意位置关闭', async () => {
    const wrapper = mountViewer();
    const thumbs = wrapper.findAll('.viewer-title-tools .library-image-ref-thumb');
    expect(thumbs).toHaveLength(2);
    await thumbs[1].trigger('click');
    const overlay = wrapper.get('.viewer-ref-overlay');
    expect(overlay.get('img').attributes('alt')).toBe('参考图 2');
    await overlay.trigger('click');
    expect(wrapper.find('.viewer-ref-overlay').exists()).toBe(false);
  });

  it('动作按钮携带任务上下文向外发事件', async () => {
    const wrapper = mountViewer();
    await wrapper.get('.viewer-title-tools [title="下载图片"]').trigger('click');
    await wrapper.get('.viewer-title-tools [title="引用到 Agent"]').trigger('click');
    await wrapper.get('.viewer-title-tools [title="删除任务及图片"]').trigger('click');

    expect(wrapper.emitted('download-output')).toEqual([[taskItem]]);
    expect(wrapper.emitted('reference-to-agent')).toEqual([[{ task, output: taskItem }]]);
    expect(wrapper.emitted('delete-task')).toEqual([[task]]);
  });

  it('无任务上下文（独立图片预览）时不显示操作按钮与缩略', () => {
    const wrapper = mountViewer({ items: [], imagePath: '/tmp/solo.png', title: '效果图' });
    expect(wrapper.find('.library-image-actions').exists()).toBe(false);
    expect(wrapper.find('.library-image-ref-thumbs').exists()).toBe(false);
    expect(wrapper.get('.viewer-prompt-row').text()).toBe('效果图');
  });

  it('从指定索引开始，加载后可查看真实尺寸及切换 100% 缩放', async () => {
    const wrapper = mountViewer({
      items: [{ path: '/first.png' }, { path: '/second.png' }],
      initialIndex: 1,
    });
    const image = wrapper.get('.viewer-main-image');
    expect(image.attributes('src')).toBe('/second.png');
    expect(wrapper.get('.viewer-position').text()).toBe('2 / 2');
    Object.defineProperties(image.element, {
      naturalWidth: { value: 2048 },
      naturalHeight: { value: 1024 },
    });
    await image.trigger('load');
    expect(wrapper.get('.viewer-title-info').text()).toContain('2048 × 1024');
    await wrapper.get('.viewer-zoom-control button:last-child').trigger('click');
    expect(wrapper.get('.viewer-image-canvas').classes()).toContain('zoomed');
    await wrapper.get('[aria-label="下一张"]').trigger('click');
    expect(wrapper.get('.viewer-main-image').attributes('src')).toBe('/first.png');
    expect(wrapper.get('.viewer-image-canvas').classes()).not.toContain('zoomed');
  });

  it('图片读取失败显示可操作的错误，重新加载会重新创建图片元素', async () => {
    const wrapper = mountViewer();
    const image = wrapper.get('.viewer-main-image');
    await image.trigger('error');
    expect(wrapper.get('.viewer-image-state').text()).toContain('暂时无法显示这张图片');
    await wrapper.get('.viewer-image-state button').trigger('click');
    expect(wrapper.get('.viewer-image-state').text()).toContain('正在载入图片');
    expect(wrapper.get('.viewer-main-image').element).not.toBe(image.element);
  });
});
