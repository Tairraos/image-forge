import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { defaultSettings } from '../../src/lib/models';

const listModels = vi.fn();
vi.mock('../../src/api/index.js', () => ({ listProviderModels: (...args) => listModels(...args) }));
import ApiSourcePanel from '../../src/components/dialogs/ApiSourcePanel.vue';

function mountPanel() {
  const settings = defaultSettings();
  settings.providers = [
    {
      id: 'source-1',
      name: '测试绘图',
      baseUrl: 'https://example.test/v1',
      apiKey: 'test-key',
      proxyUrl: '',
      modelType: 'image-gpt',
      imageModel: 'gpt-image-2',
      imagesConcurrency: 1,
    },
  ];
  return mount(ApiSourcePanel, { props: { show: true, settings, kind: 'image' } });
}

describe('ApiSourcePanel native form', () => {
  it('编辑原生字段并通过表单保存，获取的模型可用于输入建议', async () => {
    listModels.mockResolvedValue(['gpt-image-2', 'gpt-image-1']);
    const wrapper = mountPanel();
    await wrapper.get('[aria-label="编辑"]').trigger('click');
    await wrapper.get('input[placeholder="例如 OpenAI / 自建服务"]').setValue('新名称');
    await wrapper
      .get('input[placeholder="https://api.openai.com/v1"]')
      .setValue('https://model.test/v1');
    await wrapper.get('input[type="password"]').setValue('new-test-key');
    await wrapper.get('.model-select-row button').trigger('click');
    await flushPromises();
    expect(
      wrapper.findAll('datalist option').map((option) => option.attributes('value'))
    ).toContain('gpt-image-1');
    await wrapper.get('input[list]').setValue('gpt-image-1');
    await wrapper.get('.provider-form select').setValue('image-gemini');
    await wrapper.get('.provider-form').trigger('submit');
    expect(wrapper.emitted('save')[0][0].providers[0]).toMatchObject({
      name: '新名称',
      baseUrl: 'https://model.test/v1',
      apiKey: 'new-test-key',
      imageModel: 'gpt-image-1',
      modelType: 'image-gemini',
    });
    expect(wrapper.find('.provider-form').exists()).toBe(false);
  });

  it('模型获取失败后显示错误并恢复按钮', async () => {
    listModels.mockRejectedValue(new Error('模型服务不可用'));
    const wrapper = mountPanel();
    await wrapper.get('[aria-label="编辑"]').trigger('click');
    await wrapper.get('.model-select-row button').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="status"]').text()).toContain('模型服务不可用');
    expect(wrapper.get('.model-select-row button').element.disabled).toBe(false);
    expect(wrapper.emitted('save')).toBeUndefined();
  });
});
