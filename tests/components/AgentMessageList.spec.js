import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { describe, expect, it, vi } from 'vitest';
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
    expect(wrapper.text()).toContain('创建绘画任务');
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
    expect(wrapper.get('.agent-tool-card').text()).toContain('查看生成进度');
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
    const list = wrapper.get('.agent-message-list').element;
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

  it('较长的新回复仍跟随底部，上翻后可一键回到最新消息', async () => {
    const wrapper = mount(AgentMessageList, {
      props: { messages: [baseMessage], sessionId: 'first' },
    });
    const list = wrapper.get('.agent-message-list').element;
    Object.defineProperties(list, {
      scrollHeight: {
        configurable: true,
        get: () => (list.textContent.includes('长回复') ? 1800 : 1000),
      },
      clientHeight: { configurable: true, value: 400 },
    });
    list.scrollTop = 600;
    await wrapper.setProps({ streamText: '长回复' });
    await nextTick();
    expect(list.scrollTop).toBe(1800);
    list.scrollTop = 200;
    await wrapper.get('.agent-message-list').trigger('scroll');
    expect(wrapper.find('[aria-label="回到最新消息"]').exists()).toBe(true);
    await wrapper.setProps({ streamText: '长回复，继续输出' });
    expect(list.scrollTop).toBe(200);
    await wrapper.get('[aria-label="回到最新消息"]').trigger('click');
    expect(list.scrollTop).toBe(1800);
    expect(wrapper.find('[aria-label="回到最新消息"]').exists()).toBe(false);
    list.scrollTop = 100;
    await wrapper.setProps({ sessionId: 'second' });
    await nextTick();
    expect(list.scrollTop).toBe(1800);
  });

  it('分别展示排队、取消和失败状态，失败项可重试', async () => {
    const groups = ['queued', 'cancelling', 'failed'].map((status) => ({
      id: status,
      role: 'tool',
      taskGroup: {
        id: status,
        status,
        errors: status === 'failed' ? ['配额不足，请检查 API 额度'] : [],
      },
    }));
    const wrapper = mount(AgentMessageList, { props: { messages: groups } });
    expect(wrapper.text()).toContain('已加入队列');
    expect(wrapper.text()).toContain('轮到此任务后会自动开始');
    expect(wrapper.text()).toContain('正在取消');
    expect(wrapper.text()).not.toContain('服务器已经连接');
    const cancelling = wrapper.get('[data-status="cancelling"]');
    expect(cancelling.get('button').attributes('disabled')).toBeDefined();
    const failed = wrapper.get('[data-status="failed"]');
    expect(failed.get('details').text()).toContain('配额不足');
    await failed.get('button').trigger('click');
    expect(wrapper.emitted('retry-task-group')[0][0].id).toBe('failed');
  });

  it('一个回复里的多次工具调用都可见，附图可以预览', async () => {
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [
          {
            ...baseMessage,
            attachments: [{ id: 'reference', path: '/ref.png', fileName: '参考图.png' }],
            toolCalls: [
              { id: 'first', name: 'list_templates', status: 'completed' },
              { id: 'second', name: 'create_image_tasks', status: 'completed' },
            ],
          },
        ],
      },
    });
    expect(wrapper.findAll('.agent-tool-card')).toHaveLength(2);
    await wrapper.get('[aria-label="查看参考图 1"]').trigger('click');
    expect(wrapper.emitted('preview-images')[0][0]).toEqual({
      items: [{ path: '/ref.png', title: '参考图.png' }],
      index: 0,
    });
  });

  it('复制消息反馈成功或失败', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const original = Object.getOwnPropertyDescriptor(navigator, 'clipboard');
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    try {
      const wrapper = mount(AgentMessageList, { props: { messages: [baseMessage] } });
      await wrapper.get('[aria-label="复制消息"]').trigger('click');
      await nextTick();
      expect(writeText).toHaveBeenCalledWith('**完成**');
      expect(wrapper.find('[aria-label="已复制"]').exists()).toBe(true);
      writeText.mockRejectedValueOnce(new Error('permission denied'));
      await wrapper.get('[aria-label="已复制"]').trigger('click');
      await nextTick();
      expect(wrapper.text()).toContain('复制失败');
    } finally {
      if (original) Object.defineProperty(navigator, 'clipboard', original);
      else delete navigator.clipboard;
    }
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
