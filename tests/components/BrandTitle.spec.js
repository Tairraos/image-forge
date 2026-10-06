import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import BrandTitle from '../../src/components/snippets/BrandTitle.vue';

describe('BrandTitle', () => {
  it('按主题切换浅色/深色标题图', () => {
    const light = mount(BrandTitle, { props: { theme: 'light' } });
    expect(light.attributes('src')).toContain('title-light');
    const dark = mount(BrandTitle, { props: { theme: 'dark' } });
    expect(dark.attributes('src')).toContain('title-dark');
    // 默认浅色
    const fallback = mount(BrandTitle);
    expect(fallback.attributes('src')).toContain('title-light');
  });
});
