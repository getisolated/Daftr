import { TranslationKey } from '../i18n/translations';

export type ThemeId = 'dark' | 'light' | 'solarized-light' | 'violet-light' | 'github-light' | 'github-dark-dimmed';

/** Light or dark family of a theme: drives CodeMirror's base styles and the 'system' preference. */
export type ThemeKind = 'dark' | 'light';

export interface ThemeDefinition {
  id: ThemeId;
  /** Translation key of the theme name; use `themeLabel(id, t)` to display it. */
  labelKey: TranslationKey;
  kind: ThemeKind;
}

/** A theme, or 'system' to follow the OS color scheme. */
export type ThemeChoice = ThemeId | 'system';

/** One entry of the theme picker. */
export interface ThemePickerItem {
  id: ThemeChoice;
  label: string;
  detail: string;
}

/** Every theme of the app, in the order of the theme picker. Each one has a `:root[data-theme='…']` block in styles.scss. */
export const THEMES: readonly ThemeDefinition[] = [
  { id: 'light', labelKey: 'settings.theme.light', kind: 'light' },
  { id: 'github-light', labelKey: 'settings.theme.githubLight', kind: 'light' },
  { id: 'solarized-light', labelKey: 'settings.theme.solarizedLight', kind: 'light' },
  { id: 'violet-light', labelKey: 'settings.theme.violetLight', kind: 'light' },
  { id: 'dark', labelKey: 'settings.theme.dark', kind: 'dark' },
  { id: 'github-dark-dimmed', labelKey: 'settings.theme.githubDarkDimmed', kind: 'dark' },
];
