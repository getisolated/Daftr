import { ResolvedTheme, ThemePreference } from '../models/settings.model';

/** localStorage key mirroring the preference, read by the inline script of index.html before the first paint. */
export const THEME_STORAGE_KEY = 'evn-theme';

export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean): ResolvedTheme {
  if (preference === 'system') {
    return systemPrefersDark ? 'dark' : 'light';
  }
  return preference;
}

/** The explicit preference that flips the theme currently displayed (leaves 'system' mode on purpose). */
export function oppositeTheme(current: ResolvedTheme): ResolvedTheme {
  return current === 'dark' ? 'light' : 'dark';
}
