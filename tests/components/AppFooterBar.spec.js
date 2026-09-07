import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import AppFooterBar from '../../src/components/AppFooterBar.vue';

describe('AppFooterBar', () => {
  it('原生选择框独立切换绘图与对话模型', async () => {
    const wrapper = mount(AppFooterBar, {
      props: {
        imageProviderId: 'image-1',
        imageProviderName: '生图源',
        imageProviderOptions: [
          { label: '生图源', value: 'image-1' },
          { label: '另一个生图源', value: 'image-2' },
        ],
        chatProviderId: 'chat-1',
        chatProviderName: '对话源',
        chatProviderOptions: [
          { label: '对话源', value: 'chat-1' },
          { label: '另一个对话源', value: 'chat-2' },
        ],
      },
    });
    const image = wrapper.get('select[aria-label="绘图模型"]');
    const chat = wrapper.get('select[aria-label="对话模型"]');
    expect(image.element.value).toBe('image-1');
    expect(chat.element.value).toBe('chat-1');
    await image.setValue('image-2');
    await chat.setValue('chat-2');
    expect(wrapper.emitted('select-image-provider')).toEqual([['image-2']]);
    expect(wrapper.emitted('select-chat-provider')).toEqual([['chat-2']]);
  });
});
