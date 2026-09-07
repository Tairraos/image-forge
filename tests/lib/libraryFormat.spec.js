import { describe, expect, it } from 'vitest';
import { agentMessagesForDisplay, taskGroupStatus } from '../../src/lib/libraryFormat.js';

describe('对话中的绘画任务', () => {
  it('优先显示尚在进行的任务，结束后准确反映部分失败', () => {
    expect(taskGroupStatus([{ status: 'completed' }, { status: 'running' }])).toBe('running');
    expect(taskGroupStatus([{ status: 'completed' }, { status: 'failed' }])).toBe('failed');
    expect(taskGroupStatus([{ status: 'cancelling' }, { status: 'queued' }])).toBe('cancelling');
    expect(taskGroupStatus([])).toBe('missing');
  });

  it('双端记录都能展示图片、进度和错误，重复结果摘要不进入界面', () => {
    const messages = [
      { id: 'group', taskGroup: { id: 'g', taskIds: ['first', 'second'], status: 'queued' } },
      { id: 'summary', status: 'task_result', content: '[taskGroupId=g] 绘图任务组未全部成功' },
    ];
    const history = [
      {
        id: 'first',
        taskGroupId: 'g',
        status: 'completed',
        prompt: '第一张',
        outputs: [{ path: '/out.png' }],
      },
      { id: 'second', task_group_id: 'g', status: 'failed', error: '配额不足', outputs: [] },
    ];
    const display = agentMessagesForDisplay(messages, history);
    expect(display).toHaveLength(1);
    expect(display[0].taskGroup).toMatchObject({
      status: 'failed',
      progress: { total: 2, completed: 1 },
      errors: ['配额不足'],
    });
    expect(display[0].taskGroup.images[0].task).toBe(history[0]);
    expect(messages).toHaveLength(2);
  });

  it('只有旧结果摘要时还原任务卡片，不显示内部路径或任务 ID', () => {
    const display = agentMessagesForDisplay(
      [
        {
          id: 'old',
          status: 'task_result',
          content: '[taskGroupId=old-group] 已完成：/Users/person/private.png',
        },
      ],
      [
        {
          id: 'task',
          task_group_id: 'old-group',
          status: 'completed',
          outputs: [{ path: '/image.png' }],
        },
      ]
    );
    expect(display[0].content).toBe('');
    expect(display[0].taskGroup).toMatchObject({
      id: 'old-group',
      taskIds: ['task'],
      status: 'completed',
      images: [{ path: '/image.png' }],
    });
  });
});
