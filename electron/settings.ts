import { app } from 'electron/main';
import * as fs from 'fs';
import * as path from 'path';
import { AppSettings, DEFAULT_SETTINGS, isLanguage, resolveLanguage, sanitizeSettings } from './settings-schema';

let current: AppSettings = { ...DEFAULT_SETTINGS };

function settingsPath(): string {
  return path.join(app.getPath('userData'), 'settings.json');
}

export function loadSettings(): AppSettings {
  let hasLanguage = false;
  try {
    const raw = JSON.parse(fs.readFileSync(settingsPath(), 'utf-8'));
    hasLanguage = isLanguage(raw?.language);
    current = sanitizeSettings(raw);
  } catch {
    // First launch or corrupted file: defaults are fine
    current = { ...DEFAULT_SETTINGS };
  }
  // No language chosen yet (first launch, or settings saved before the language existed): follow the OS locale
  if (!hasLanguage) {
    updateSettings({ language: resolveLanguage(app.getLocale()) });
  }
  return current;
}

export function getSettings(): AppSettings {
  return current;
}

/** Merges a partial update (validated) and persists it. */
export function updateSettings(patch: Partial<AppSettings>): AppSettings {
  current = sanitizeSettings({ ...current, ...patch }, current);
  try {
    fs.writeFileSync(settingsPath(), JSON.stringify(current, null, 2), 'utf-8');
  } catch (err) {
    console.error('[Settings] Failed to persist settings:', err);
  }
  return current;
}
