import { DOCUMENT } from '@angular/common';
import { Injectable, computed, effect, inject } from '@angular/core';
import { SettingsService } from '../services/settings.service';
import { Language, resolveLanguage } from './language.model';
import { PluralTranslator, TranslationParams, Translator, translate, translatePlural } from './translate';
import { PluralKey, TranslationKey } from './translations';

@Injectable({ providedIn: 'root' })
export class I18nService {
  private readonly settings = inject(SettingsService);
  private readonly document = inject(DOCUMENT);
  /** Before the main process answers, the language of the OS (same locale as the main process) avoids a flash of English. */
  private readonly systemLanguage = resolveLanguage(navigator.language);

  /** Language in effect; reading it in a template or a computed makes them follow a language change. */
  readonly language = computed<Language>(() => (this.settings.loaded() ? this.settings.settings().language : this.systemLanguage));

  constructor() {
    effect(() => { this.document.documentElement.lang = this.language(); });
  }

  /** Translates a key; usable in templates (`i18n.t('key')`), the language is a signal read. */
  readonly t: Translator = (key: TranslationKey, params?: TranslationParams): string => translate(this.language(), key, params);

  readonly plural: PluralTranslator = (key: PluralKey, count: number, params?: TranslationParams): string =>
    translatePlural(this.language(), key, count, params);
}
