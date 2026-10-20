import { Language } from './language.model';
import { PluralKey, TRANSLATIONS, TranslationKey } from './translations';

export type TranslationParams = Record<string, string | number>;

/** Translates a key into the current language; `{name}` placeholders are filled from the params. */
export type Translator = (key: TranslationKey, params?: TranslationParams) => string;

/** Same as Translator, picking the `.one` / `.other` form of the key for the count (`{count}` is filled in). */
export type PluralTranslator = (key: PluralKey, count: number, params?: TranslationParams) => string;

function interpolate(text: string, params: TranslationParams | undefined): string {
  if (!params) {
    return text;
  }

  return text.replace(/\{(?<name>\w+)\}/g, (placeholder, name: string) => String(params[name] ?? placeholder));
}

/** Pure translation, usable outside Angular (CodeMirror extensions, helpers, tests). */
export function translate(language: Language, key: TranslationKey, params?: TranslationParams): string {
  return interpolate(TRANSLATIONS[language][key], params);
}

export function translatePlural(language: Language, key: PluralKey, count: number, params?: TranslationParams): string {
  const form = new Intl.PluralRules(language).select(count) === 'one' ? 'one' : 'other';
  const text = TRANSLATIONS[language][`${key}.${form}` as TranslationKey];

  return interpolate(text, { count, ...params });
}
