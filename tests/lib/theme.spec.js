import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  applyTheme,
  readThemePreference,
  resolveTheme,
  saveThemePreference,
  THEME_KEY,
} from '../../src/lib/theme';

const storageDescriptor = Object.getOwnPropertyDescriptor(window, 'localStorage');

beforeEach(() => {
  vi.restoreAllMocks();
  const values = new Map();
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => values.set(key, String(value)),
    },
  });
});

afterAll(() => {
  if (storageDescriptor) Object.defineProperty(window, 'localStorage', storageDescriptor);
});

describe('theme', () => {
  it('记住选择，缺省和无效值跟随系统', () => {
    expect(readThemePreference()).toBe('system');
    expect(saveThemePreference('dark')).toBe(true);
    expect(readThemePreference()).toBe('dark');
    window.localStorage.setItem(THEME_KEY, 'invalid');
    expect(readThemePreference()).toBe('system');
  });

  it('跟随系统变化，显式主题不受系统影响', () => {
    vi.spyOn(window, 'matchMedia').mockReturnValue({ matches: true });
    expect(applyTheme('system')).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(applyTheme('light')).toBe('light');
    expect(document.documentElement.style.colorScheme).toBe('light');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });

  it('存储不可用时仍能显示和切换主题', () => {
    vi.spyOn(window.localStorage, 'getItem').mockImplementation(() => {
      throw new Error('disabled');
    });
    vi.spyOn(window.localStorage, 'setItem').mockImplementation(() => {
      throw new Error('disabled');
    });
    expect(readThemePreference()).toBe('system');
    expect(saveThemePreference('light')).toBe(false);
    expect(applyTheme('light')).toBe('light');
  });
});
