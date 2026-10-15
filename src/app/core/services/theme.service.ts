import { Injectable, computed, effect, inject, signal } from '@angular/core';
import { ResolvedTheme } from '../models/settings.model';
import { THEME_STORAGE_KEY, oppositeTheme, resolveTheme } from '../utils/theme.util';
import { SettingsService } from './settings.service';

const DARK_SCHEME_QUERY = '(prefers-color-scheme: dark)';

@Injectable({ providedIn: 'root' })
export class ThemeService {
  private readonly settingsService = inject(SettingsService);
  private readonly systemPrefersDark = signal(false);

  /** Theme in effect, 'system' being resolved against the OS color scheme. */
  readonly theme = computed<ResolvedTheme>(() => resolveTheme(this.settingsService.settings().theme, this.systemPrefersDark()));

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
      // Settings only arrive asynchronously from the main process: this cache lets the next launch paint the right theme at once
      try {
        localStorage.setItem(THEME_STORAGE_KEY, this.settingsService.settings().theme);
      } catch {
        // Storage unavailable: the theme is simply applied after Angular boots
      }
    });
  }

  /** Flips light/dark and persists it as the explicit preference. */
  async toggle(): Promise<void> {
    await this.settingsService.update({ theme: oppositeTheme(this.theme()) });
  }
}
