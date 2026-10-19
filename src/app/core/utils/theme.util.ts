import { ThemePreference } from '../models/settings.model';
import { THEMES, ThemeChoice, ThemeDefinition, ThemeId, ThemeKind, ThemePickerItem } from '../models/theme.model';

/** localStorage key mirroring the preference, read by the inline script of index.html before the first paint. */
export const THEME_STORAGE_KEY = 'evn-theme';
/** localStorage keys mirroring the themes used by 'system' in dark / light mode (same purpose). */
export const DARK_THEME_STORAGE_KEY = 'evn-theme-dark';
export const LIGHT_THEME_STORAGE_KEY = 'evn-theme-light';

export interface SystemThemes {
  dark: ThemeId;
  light: ThemeId;
}

export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean, systemThemes: SystemThemes): ThemeId {
  if (preference === 'system') {
    return systemPrefersDark ? systemThemes.dark : systemThemes.light;
  }
  return preference;
}

export function findTheme(id: ThemeId): ThemeDefinition | undefined {
  return THEMES.find(theme => theme.id === id);
}

/** Unknown ids are dark, like the default `:root` block of styles.scss. */
export function themeKind(id: ThemeId): ThemeKind {
  return findTheme(id)?.kind ?? 'dark';
}

/** Entries of the theme picker: 'system' first (with what it resolves to), then every theme; the current choice is flagged. */
export function buildThemePickerItems(systemThemes: SystemThemes, current: ThemeChoice): ThemePickerItem[] {
  const darkLabel = findTheme(systemThemes.dark)?.label ?? systemThemes.dark;
  const lightLabel = findTheme(systemThemes.light)?.label ?? systemThemes.light;
  const choices: ThemePickerItem[] = [
    { id: 'system', label: 'System', detail: `Follows Windows: ${darkLabel} / ${lightLabel}` },
    ...THEMES.map(theme => ({ id: theme.id, label: theme.label, detail: theme.kind === 'dark' ? 'Dark theme' : 'Light theme' })),
  ];

  return choices.map(item => (item.id === current ? { ...item, detail: `${item.detail} · Current` } : item));
}
