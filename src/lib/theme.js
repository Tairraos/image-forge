export const THEME_KEY = 'image-forge-theme';

export function readThemePreference() {
  try {
    const value = window.localStorage.getItem(THEME_KEY);
    return ['light', 'dark', 'system'].includes(value) ? value : 'system';
  } catch {
    return 'system';
  }
}

export function resolveTheme(preference, systemDark) {
  return preference === 'system' ? (systemDark ? 'dark' : 'light') : preference;
}

export function applyTheme(preference) {
  const theme = resolveTheme(preference, window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
  return theme;
}

export function saveThemePreference(preference) {
  try {
    window.localStorage.setItem(THEME_KEY, preference);
    return true;
  } catch {
    return false;
  }
}
