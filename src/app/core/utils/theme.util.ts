import { Translator, translate } from '../i18n/translate';
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

/** Translated name of a theme ('system' included); an unknown id is shown as is. */
export function themeLabel(id: ThemeChoice, t: Translator): string {
  if (id === 'system') {
    return t('settings.theme.system');
  }
  const theme = findTheme(id);
  return theme ? t(theme.labelKey) : id;
}

const ENGLISH: Translator = (key, params) => translate('en', key, params);

/** Entries of the theme picker: 'system' first (with what it resolves to), then every theme; the current choice is flagged. */
export function buildThemePickerItems(systemThemes: SystemThemes, current: ThemeChoice, t: Translator = ENGLISH): ThemePickerItem[] {
  const choices: ThemePickerItem[] = [
    {
      id: 'system',
      label: themeLabel('system', t),
      detail: t('settings.theme.followsWindows', { dark: themeLabel(systemThemes.dark, t), light: themeLabel(systemThemes.light, t) }),
    },
    ...THEMES.map(theme => ({
      id: theme.id,
      label: t(theme.labelKey),
      detail: t(theme.kind === 'dark' ? 'settings.theme.darkKind' : 'settings.theme.lightKind'),
    })),
  ];

  return choices.map(item => (item.id === current ? { ...item, detail: t('settings.theme.current', { detail: item.detail }) } : item));
}
