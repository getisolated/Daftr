/** Languages the app is translated in. */
export type Language = 'en' | 'fr';

export const DEFAULT_LANGUAGE: Language = 'en';

/** Each language is named in itself, so it stays recognizable whatever the current language is. */
export const LANGUAGES: readonly { id: Language; label: string }[] = [
  { id: 'en', label: 'English' },
  { id: 'fr', label: 'Français' },
];

export function isLanguage(value: unknown): value is Language {
  return LANGUAGES.some(language => language.id === value);
}

/** Language matching a locale such as `fr-FR` or `en_US`; English when the locale is not (yet) translated. */
export function resolveLanguage(locale: string | null | undefined): Language {
  const primary = (locale ?? '').toLowerCase().split(/[-_]/)[0];
  const match = LANGUAGES.find(language => language.id === primary);

  return match?.id ?? DEFAULT_LANGUAGE;
}
