import { describe, it, expect } from 'vitest';
import { LANGUAGES, isLanguage, resolveLanguage } from '../src/app/core/i18n/language.model';
import { translate, translatePlural } from '../src/app/core/i18n/translate';
import { EN, TRANSLATIONS, TranslationKey } from '../src/app/core/i18n/translations';

const PLACEHOLDER = /\{(\w+)\}/g;

function placeholders(text: string): string[] {
  return [...text.matchAll(PLACEHOLDER)].map(match => match[1] as string).sort();
}

describe('translations', () => {
  const keys = Object.keys(EN) as TranslationKey[];

  it('defines the same keys in every language', () => {
    for (const { id } of LANGUAGES) {
      expect(Object.keys(TRANSLATIONS[id]).sort()).toEqual([...keys].sort());
    }
  });

  // A sentence cut in pieces around a link: the French one needs no tail
  const MAY_BE_EMPTY: readonly string[] = ['workspace.graph.hintAfter'];

  it('never leaves a text empty', () => {
    for (const { id } of LANGUAGES) {
      keys.filter(key => !MAY_BE_EMPTY.includes(key) || id === 'en')
        .forEach(key => expect(TRANSLATIONS[id][key].trim(), `${id}:${key}`).not.toBe(''));
    }
  });

  it('uses the same placeholders in every language', () => {
    for (const key of keys) {
      expect(placeholders(TRANSLATIONS.fr[key]), key).toEqual(placeholders(EN[key]));
    }
  });

  it('has both the .one and .other form of every plural', () => {
    for (const key of keys) {
      if (key.endsWith('.one')) {
        expect(keys, key).toContain(`${key.slice(0, -4)}.other`);
      }
      if (key.endsWith('.other')) {
        expect(keys, key).toContain(`${key.slice(0, -6)}.one`);
      }
    }
  });
});

describe('translate', () => {
  it('translates a key in the requested language', () => {
    expect(translate('en', 'common.cancel')).toBe('Cancel');
    expect(translate('fr', 'common.cancel')).toBe('Annuler');
  });

  it('keeps unknown placeholders and ignores params without placeholder', () => {
    expect(translate('en', 'common.cancel', { name: 'x' })).toBe('Cancel');
  });
});

describe('translatePlural', () => {
  const pluralBases = (Object.keys(EN) as TranslationKey[]).filter(key => key.endsWith('.one')).map(key => key.slice(0, -4));

  it('fills the count in the form matching it', () => {
    for (const base of pluralBases as Parameters<typeof translatePlural>[1][]) {
      expect(translatePlural('en', base, 1)).not.toContain('{count}');
      expect(translatePlural('fr', base, 5)).not.toContain('{count}');
    }
  });
});

describe('resolveLanguage', () => {
  it('maps a locale to its language, English by default', () => {
    expect(resolveLanguage('fr')).toBe('fr');
    expect(resolveLanguage('fr-FR')).toBe('fr');
    expect(resolveLanguage('fr_CA')).toBe('fr');
    expect(resolveLanguage('FR-be')).toBe('fr');
    expect(resolveLanguage('en-US')).toBe('en');
    expect(resolveLanguage('de-DE')).toBe('en');
    expect(resolveLanguage('')).toBe('en');
    expect(resolveLanguage(undefined)).toBe('en');
  });

  it('only accepts supported languages', () => {
    expect(isLanguage('fr')).toBe(true);
    expect(isLanguage('de')).toBe(false);
    expect(isLanguage(null)).toBe(false);
  });
});
