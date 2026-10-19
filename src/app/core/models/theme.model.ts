export type ThemeId = 'dark' | 'light' | 'solarized-light' | 'violet-light' | 'github-light' | 'github-dark-dimmed';

/** Light or dark family of a theme: drives CodeMirror's base styles and the 'system' preference. */
export type ThemeKind = 'dark' | 'light';

export interface ThemeDefinition {
  id: ThemeId;
  label: string;
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
  { id: 'light', label: 'Light', kind: 'light' },
  { id: 'github-light', label: 'GitHub Light', kind: 'light' },
  { id: 'solarized-light', label: 'Solarized Light', kind: 'light' },
  { id: 'violet-light', label: 'Violet Light', kind: 'light' },
  { id: 'dark', label: 'Dark', kind: 'dark' },
  { id: 'github-dark-dimmed', label: 'GitHub Dark Dimmed', kind: 'dark' },
];
