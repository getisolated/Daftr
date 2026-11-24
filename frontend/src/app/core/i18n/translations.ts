import { EN_COLLECTION_SYNC } from './translations/en/collection-sync';
import { EN_COMMON } from './translations/en/common';
import { EN_EDITOR } from './translations/en/editor';
import { EN_PROFILES } from './translations/en/profiles';
import { EN_SETTINGS } from './translations/en/settings';
import { EN_SIDEBAR } from './translations/en/sidebar';
import { EN_SYNC } from './translations/en/sync';
import { EN_UI } from './translations/en/ui';
import { EN_WORKSPACE } from './translations/en/workspace';
import { FR_COLLECTION_SYNC } from './translations/fr/collection-sync';
import { FR_COMMON } from './translations/fr/common';
import { FR_EDITOR } from './translations/fr/editor';
import { FR_PROFILES } from './translations/fr/profiles';
import { FR_SETTINGS } from './translations/fr/settings';
import { FR_SIDEBAR } from './translations/fr/sidebar';
import { FR_SYNC } from './translations/fr/sync';
import { FR_UI } from './translations/fr/ui';
import { FR_WORKSPACE } from './translations/fr/workspace';
import { Language } from './language.model';

/** English is the reference: every key exists there first, the French dictionary must define the same ones. */
export const EN = {
  ...EN_COMMON,
  ...EN_UI,
  ...EN_SETTINGS,
  ...EN_SIDEBAR,
  ...EN_EDITOR,
  ...EN_WORKSPACE,
  ...EN_PROFILES,
  ...EN_SYNC,
  ...EN_COLLECTION_SYNC,
} as const;

export type TranslationKey = keyof typeof EN;

/** Keys with a `.one` / `.other` form, named without the suffix. */
export type PluralKey = TranslationKey extends infer Key ? (Key extends `${infer Base}.one` ? Base : never) : never;

const FR: Record<TranslationKey, string> = {
  ...FR_COMMON,
  ...FR_UI,
  ...FR_SETTINGS,
  ...FR_SIDEBAR,
  ...FR_EDITOR,
  ...FR_WORKSPACE,
  ...FR_PROFILES,
  ...FR_SYNC,
  ...FR_COLLECTION_SYNC,
};

export const TRANSLATIONS: Record<Language, Record<TranslationKey, string>> = { en: EN, fr: FR };
