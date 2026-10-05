import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import AgentWorkspace from '../../src/components/AgentWorkspace.vue';

describe('AgentWorkspace', () => {
  it('选择、删除会话并转发任务组跳转', async () => {
    const sessions = [
      { id: 'old', title: '较早对话', createdAt: '2026-07-19T08:00:00Z' },
      { id: 'new', title: '较晚对话', createdAt: '2026-07-20T08:00:00Z' },
    ];
    const wrapper = mount(AgentWorkspace, {
      props: {
        sessions,
        currentSession: sessions[0],
        providerId: 'chat',
        imageProviderId: 'image',
        libraryCount: 12,
      },
      global: {
        stubs: {
          AgentComposer: true,
          AgentMessageList: {
            emits: ['open-task-group'],
            template:
              '<button class="open-group" @click="$emit(\'open-task-group\', { id: \'group-1\' })">打开</button>',
          },
        },
      },
    });
    expect(wrapper.findAll('.agent-session-title').map((item) => item.text())).toEqual([
      '较早对话',
      '较晚对话',
    ]);
    expect(wrapper.get('.function-bar-item-count').text()).toBe('12');
    await wrapper.findAll('.agent-session-item')[1].trigger('click');
    expect(wrapper.emitted('select')).toEqual([['new']]);
    await wrapper.findAll('.agent-session-delete')[0].trigger('click');
    expect(wrapper.emitted('delete-session')).toEqual([['old']]);
    await wrapper.get('.open-group').trigger('click');
    expect(wrapper.emitted('open-task-group')).toEqual([[{ id: 'group-1' }]]);
  });

  it('不在 Agent 顶栏重复显示模型选择', () => {
    const wrapper = mount(AgentWorkspace, {
      props: {
        providerId: 'chat-1',
        imageProviderId: 'image-1',
      },
      global: { stubs: { AgentComposer: true, AgentMessageList: true } },
    });
    expect(wrapper.find('.agent-model-selects').exists()).toBe(false);
  });

  it('右键会话标题弹出菜单，改名进入 inline 编辑并提交', async () => {
    const sessions = [{ id: 's1', title: '旧名字', createdAt: '2026-10-06T08:00:00Z' }];
    const wrapper = mount(AgentWorkspace, {
      props: { sessions, currentSession: sessions[0], providerId: 'c', imageProviderId: 'i' },
      global: { stubs: { AgentComposer: true, AgentMessageList: true } },
    });
    await wrapper.findAll('.agent-session-item')[0].trigger('contextmenu', {
      clientX: 30,
      clientY: 40,
    });
    const menu = wrapper.get('.session-context-menu');
    await menu.get('button').trigger('click');
    const input = wrapper.get('.agent-session-rename');
    expect(input.element.value).toBe('旧名字');
    await input.setValue('手动改的名字');
    await input.trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('rename-session')).toEqual([
      [{ sessionId: 's1', title: '手动改的名字' }],
    ]);
    expect(wrapper.find('.session-context-menu').exists()).toBe(false);
  });

  it('顶部标题尾部显示会话日期，改名时隐藏', async () => {
    const sessions = [{ id: 's1', title: '旅行插画', createdAt: '2026-10-06T08:00:00Z' }];
    const wrapper = mount(AgentWorkspace, {
      props: { sessions, currentSession: sessions[0], providerId: 'c', imageProviderId: 'i' },
      global: { stubs: { AgentComposer: true, AgentMessageList: true } },
    });
    const title = wrapper.get('.info-area-title');
    expect(title.text()).toContain('旅行插画');
    expect(title.get('.info-area-title-date').text()).toBe('2026-10-06');
    await title.trigger('click');
    // 进入改名：日期随标题一起切换成输入框
    expect(wrapper.find('.info-area-title-date').exists()).toBe(false);
    const input = wrapper.get('.info-area-title-rename');
    expect(input.element.value).toBe('旅行插画');
    await input.setValue('新标题');
    await input.trigger('blur');
    expect(wrapper.emitted('rename-session')).toEqual([[{ sessionId: 's1', title: '新标题' }]]);
    // 模拟父组件应用改名后再看展示：标题更新、日期重新出现
    const renamed = { id: 's1', title: '新标题', createdAt: '2026-10-06T08:00:00Z' };
    await wrapper.setProps({ sessions: [renamed], currentSession: renamed });
    expect(wrapper.get('.info-area-title').text()).toContain('新标题');
    expect(wrapper.get('.info-area-title-date').text()).toBe('2026-10-06');
  });
});
