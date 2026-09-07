import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import AgentComposer from '../../src/components/AgentComposer.vue';

describe('AgentComposer', () => {
  it('底部原生控件发出比例和分辨率变更，忙碌时禁止发送', async () => {
    const wrapper = mount(AgentComposer, {
      props: { providerId: 'chat', imageProviderId: 'image' },
    });
    await wrapper.get('.agent-composer-footer select[aria-label="图片比例"]').setValue('16:9');
    await wrapper.get('.agent-composer-footer select[aria-label="图片分辨率"]').setValue('2k');
    expect(wrapper.emitted('update:ratio')).toEqual([['16:9']]);
    expect(wrapper.emitted('update:resolution')).toEqual([['2k']]);
    await wrapper.get('textarea').setValue('保留的草稿');
    await wrapper.setProps({ busy: true });
    await wrapper.get('textarea').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('send')).toBeUndefined();
    expect(wrapper.get('textarea').element.value).toBe('保留的草稿');
    await wrapper.get('[aria-label="停止生成"]').trigger('click');
    expect(wrapper.emitted('stop')).toHaveLength(1);
  });

  it('普通消息发送给对话模型', async () => {
    const wrapper = mount(AgentComposer, {
      props: { providerId: 'chat', imageProviderId: 'image' },
    });
    await wrapper.get('textarea').setValue('你好 Agent');
    await wrapper.get('.agent-send-button').trigger('click');
    expect(wrapper.emitted('send')).toEqual([[{ content: '你好 Agent', drawThisTurn: false }]]);
  });

  it('回车和 Command 回车发送，Shift 回车保留换行', async () => {
    const wrapper = mount(AgentComposer, {
      props: { providerId: 'chat', imageProviderId: 'image' },
    });
    const textarea = wrapper.get('textarea');
    await textarea.setValue('第一条');
    await textarea.trigger('keydown', { key: 'Enter' });
    await textarea.setValue('需要换行');
    await textarea.trigger('keydown', { key: 'Enter', shiftKey: true });
    await textarea.trigger('keydown', { key: 'Enter', isComposing: true });
    await textarea.setValue('第二条');
    await textarea.trigger('keydown', { key: 'Enter', metaKey: true });
    expect(wrapper.emitted('send')).toEqual([
      [{ content: '第一条', drawThisTurn: false }],
      [{ content: '第二条', drawThisTurn: false }],
    ]);
  });

  it('勾选本轮绘画后发送绘画请求，并支持停止与参考图删除', async () => {
    const wrapper = mount(AgentComposer, {
      props: {
        providerId: 'chat',
        imageProviderId: 'image',
        attachments: [{ id: 'ref-1', dataUrl: 'data:image/png;base64,AA==', fileName: 'ref.png' }],
      },
    });
    await wrapper.get('input[type="checkbox"]').setValue(true);
    expect(wrapper.text()).toContain('直接绘画');
    await wrapper.get('textarea').setValue('画一只猫');
    await wrapper.get('.agent-send-button').trigger('click');
    expect(wrapper.emitted('send')).toEqual([[{ content: '画一只猫', drawThisTurn: true }]]);
    await wrapper.get('button[aria-label="移除参考图"]').trigger('click');
    expect(wrapper.emitted('remove-attachment')).toEqual([['ref-1']]);
    await wrapper.setProps({ busy: true });
    expect(wrapper.text()).toContain('停止');
  });

  it('挂载时显示已有草稿（图片库引用到 Agent 场景）', () => {
    const wrapper = mount(AgentComposer, {
      props: {
        providerId: 'chat',
        imageProviderId: 'image',
        draft: '一首诗的配图',
      },
    });
    expect(wrapper.get('textarea').element.value).toBe('一首诗的配图');
  });

  it('模板选择器：插入模板内容并上报 apply-template', async () => {
    const wrapper = mount(AgentComposer, {
      props: {
        providerId: 'chat',
        imageProviderId: 'image',
        templates: [
          { id: 'tpl-1', title: '电影海报', content: '一张{主题}的电影海报', referencePaths: [] },
        ],
      },
    });
    const picker = wrapper.findAll('.template-picker-item');
    expect(picker).toHaveLength(1);
    await picker[0].findAll('button')[0].trigger('click');
    expect(wrapper.emitted('apply-template')).toEqual([
      [
        {
          template: {
            id: 'tpl-1',
            title: '电影海报',
            content: '一张{主题}的电影海报',
            referencePaths: [],
          },
        },
      ],
    ]);
    expect(wrapper.get('textarea').element.value).toBe('一张{主题}的电影海报');
  });

  it('再次插入模板时追加而不是覆盖，占位符模板展示 AI 填充按钮', async () => {
    const wrapper = mount(AgentComposer, {
      props: {
        providerId: 'chat',
        imageProviderId: 'image',
        templates: [
          { id: 'tpl-1', title: '海报', content: '海报模板{主题}', referencePaths: [] },
          { id: 'tpl-2', title: '无占位', content: '纯文本模板', referencePaths: [] },
        ],
      },
    });
    const items = wrapper.findAll('.template-picker-item');
    expect(items[1].findAll('button').some((b) => b.text().includes('AI 填充'))).toBe(false);
    const fillButtons = items[0].findAll('button');
    expect(fillButtons.some((b) => b.text().includes('AI 填充'))).toBe(true);

    await wrapper.get('textarea').setValue('已有内容');
    await items[0].findAll('button')[0].trigger('click');
    expect(wrapper.get('textarea').element.value).toBe('已有内容\n\n海报模板{主题}');

    await items[0]
      .findAll('button')
      .find((b) => b.text().includes('AI 填充'))
      .trigger('click');
    expect(wrapper.emitted('fill-template')).toEqual([
      [{ template: { id: 'tpl-1', title: '海报', content: '海报模板{主题}', referencePaths: [] } }],
    ]);
  });
});
