import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { describe, expect, it } from 'vitest';
import AgentMessageList from '../../src/components/AgentMessageList.vue';

const baseMessage = {
  id: 'message-1',
  role: 'assistant',
  content: '**完成**',
  createdAt: '2026-07-20T08:00:00Z',
};

describe('AgentMessageList', () => {
  it('渲染 Markdown、流式消息和工具状态', () => {
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [
          baseMessage,
          {
            id: 'tool-1',
            role: 'tool',
            content: '',
            toolCall: { name: 'create_image_tasks', status: 'running' },
          },
        ],
        busy: true,
        streamText: '正在**生成**',
      },
    });
    expect(wrapper.html()).toContain('<strong>完成</strong>');
    expect(wrapper.html()).toContain('<strong>生成</strong>');
    expect(wrapper.text()).toContain('create_image_tasks');
    expect(wrapper.text()).toContain('执行中');
    expect(wrapper.text()).toContain('工具');
  });

  it('隐藏成功工具的原始结果，只显示可换行的错误', () => {
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [
          {
            id: 'status',
            role: 'tool',
            content: '{"error":null,"result":[]}',
            toolCall: { name: 'get_task_status', status: 'completed', error: null },
          },
          {
            id: 'failed',
            role: 'tool',
            content: '{"error":"请求失败","result":null}',
            toolCall: { name: 'create_image_tasks', status: 'failed', error: '很长的错误信息' },
          },
        ],
      },
    });
    expect(wrapper.text()).not.toContain('{"error"');
    expect(wrapper.get('.agent-tool-card').text()).toContain('get_task_status');
    expect(wrapper.text()).toContain('很长的错误信息');
  });

  it('工具失败时可通过详情按钮展开完整错误与结果', async () => {
    const longError = '请求失败：'.repeat(30);
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [
          {
            id: 'failed-tool',
            role: 'tool',
            content: '',
            toolCall: {
              name: 'create_image_tasks',
              status: 'failed',
              error: longError,
              result: { code: 500 },
            },
          },
        ],
      },
    });
    // 默认折叠：超长错误只显示预览
    expect(wrapper.text()).toContain('…');
    expect(wrapper.find('[data-testid="tool-error-detail"]').exists()).toBe(false);
    await wrapper.get('.agent-tool-detail-toggle').trigger('click');
    const detail = wrapper.get('[data-testid="tool-error-detail"]');
    expect(detail.text()).toContain(longError);
    expect(detail.text()).toContain('"code": 500');
    await wrapper.get('.agent-tool-detail-toggle').trigger('click');
    expect(wrapper.find('[data-testid="tool-error-detail"]').exists()).toBe(false);
  });

  it('提交交互问题', async () => {
    const questionMessage = {
      ...baseMessage,
      id: 'question',
      questions: [{ key: 'style', label: '风格', placeholder: '请输入' }],
    };
    const taskGroup = { id: 'group-1', taskIds: ['task-1'], status: 'completed' };
    const wrapper = mount(AgentMessageList, {
      props: { messages: [questionMessage, { ...baseMessage, id: 'group', taskGroup }] },
    });
    await wrapper.get('textarea').setValue('水彩');
    expect(wrapper.emitted('update-answer')).toEqual([[{ key: 'style', value: '水彩' }]]);
    await wrapper.get('.agent-question-actions button').trigger('click');
    expect(wrapper.emitted('answer-questions')[0][0].id).toBe('question');
  });

  it('仅在接近底部时自动跟随流式内容', async () => {
    const wrapper = mount(AgentMessageList, { props: { messages: [baseMessage] } });
    const list = wrapper.element;
    Object.defineProperties(list, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 400 },
    });
    list.scrollTop = 100;
    await wrapper.setProps({ streamText: '远离底部' });
    await nextTick();
    expect(list.scrollTop).toBe(100);
    list.scrollTop = 520;
    await wrapper.setProps({ streamText: '接近底部' });
    await nextTick();
    expect(list.scrollTop).toBe(1000);
  });

  it('在任务组中显示生成图片并打开大图', async () => {
    const images = [
      { path: '/tmp/one.png', title: '第一张' },
      { path: '/tmp/two.png', title: '第二张' },
    ];
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [
          { ...baseMessage, id: 'images', taskGroup: { id: 'group', status: 'completed', images } },
        ],
      },
    });
    expect(wrapper.findAll('.agent-generated-thumb img')).toHaveLength(2);
    await wrapper.findAll('.agent-generated-thumb')[1].trigger('click');
    expect(wrapper.emitted('preview-images')).toEqual([[{ items: images, index: 1 }]]);
  });
});
