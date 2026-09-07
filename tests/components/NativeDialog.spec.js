import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import NativeDialog from '../../src/components/dialogs/NativeDialog.vue';
import ConfirmDialog from '../../src/components/dialogs/ConfirmDialog.vue';

describe('NativeDialog', () => {
  it('同步原生弹窗的显示、关闭与可访问标题', async () => {
    const wrapper = mount(NativeDialog, {
      props: { show: true, title: '原生设置' },
      slots: { default: '<input aria-label="名称" />' },
    });
    const dialog = wrapper.get('dialog');
    expect(dialog.element.open).toBe(true);
    expect(dialog.attributes('aria-labelledby')).toBe(wrapper.get('h2').attributes('id'));
    await wrapper.setProps({ show: false });
    expect(dialog.element.open).toBe(false);
    expect(wrapper.find('input').exists()).toBe(false);
    await wrapper.setProps({ show: true });
    await dialog.trigger('cancel');
    expect(wrapper.emitted('update:show').at(-1)).toEqual([false]);
  });

  it('确认弹窗的遮罩不关闭，Escape 返回取消而不会确认', async () => {
    const wrapper = mount(ConfirmDialog, { props: { show: true, message: '是否删除？' } });
    const dialog = wrapper.get('dialog');
    dialog.element.dispatchEvent(
      new MouseEvent('pointerdown', { bubbles: true, clientX: -20, clientY: -20 })
    );
    dialog.element.dispatchEvent(
      new MouseEvent('click', { bubbles: true, clientX: -20, clientY: -20 })
    );
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted('update:show')).toBeUndefined();
    await dialog.trigger('cancel');
    expect(wrapper.emitted('cancel')).toHaveLength(1);
    expect(wrapper.emitted('confirm')).toBeUndefined();
  });
});
