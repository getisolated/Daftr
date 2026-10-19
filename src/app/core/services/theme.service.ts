import { Injectable, computed, effect, inject, signal } from '@angular/core';
import { ThemeChoice, ThemeId, ThemeKind } from '../models/theme.model';
import {
  DARK_THEME_STORAGE_KEY, LIGHT_THEME_STORAGE_KEY, THEME_STORAGE_KEY, resolveTheme, themeKind,
} from '../utils/theme.util';
import { SettingsService } from './settings.service';

const DARK_SCHEME_QUERY = '(prefers-color-scheme: dark)';

@Injectable({ providedIn: 'root' })
export class ThemeService {
  private readonly settingsService = inject(SettingsService);
  private readonly systemPrefersDark = signal(false);
  /** Choice highlighted in the theme picker, shown without being saved; null = the saved preference. */
  private readonly previewed = signal<ThemeChoice | null>(null);

  /** Theme in effect: the previewed choice, if any, otherwise the saved one ('system' resolved against the OS). */
  readonly theme = computed<ThemeId>(() => {
    const settings = this.settingsService.settings();
    const systemThemes = { dark: settings.preferredDarkTheme, light: settings.preferredLightTheme };
    const choice = this.previewed() ?? settings.theme;
    return resolveTheme(choice, this.systemPrefersDark(), systemThemes);
  });

  readonly kind = computed<ThemeKind>(() => themeKind(this.theme()));

  constructor() {
    const query = window.matchMedia(DARK_SCHEME_QUERY);
    this.systemPrefersDark.set(query.matches);
    query.addEventListener('change', event => this.systemPrefersDark.set(event.matches));

    effect(() => {
      // Until then the settings are mere defaults: keep what the inline script of index.html applied
      if (!this.settingsService.loaded()) {
        return;
      }

      document.documentElement.setAttribute('data-theme', this.theme());
    });

    effect(() => {
      if (!this.settingsService.loaded()) {
        return;
      }

      const { theme, preferredDarkTheme, preferredLightTheme } = this.settingsService.settings();
      // Settings only arrive asynchronously from the main process: this cache lets the next launch paint the right theme at once
      try {
        localStorage.setItem(THEME_STORAGE_KEY, theme);
        localStorage.setItem(DARK_THEME_STORAGE_KEY, preferredDarkTheme);
        localStorage.setItem(LIGHT_THEME_STORAGE_KEY, preferredLightTheme);
      } catch {
        // Storage unavailable: the theme is simply applied after Angular boots
      }
    });
  }

  /** Shows a theme without saving it (theme picker); null goes back to the saved preference. */
  preview(choice: ThemeChoice | null): void {
    this.previewed.set(choice);
  }

  /** Saves the choice as the preference and ends the preview (in that order: no flash of the previous theme). */
  async select(choice: ThemeChoice): Promise<void> {
    await this.settingsService.update({ theme: choice });
    this.previewed.set(null);
  }
}
