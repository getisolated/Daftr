import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { app } from 'electron/main';
import { loadSettings } from '../electron/settings';

describe('loadSettings language', () => {
  let directory = '';

  beforeEach(() => {
    directory = mkdtempSync(join(tmpdir(), 'evnote-settings-'));
    app.getPath = (): string => directory;
  });

  afterEach(() => {
    rmSync(directory, { recursive: true, force: true });
  });

  function storedLanguage(): unknown {
    return JSON.parse(readFileSync(join(directory, 'settings.json'), 'utf-8')).language;
  }

  it('follows the OS locale on the first launch and remembers it', () => {
    app.getLocale = (): string => 'fr-FR';

    expect(loadSettings().language).toBe('fr');
    expect(storedLanguage()).toBe('fr');
  });

  it('falls back to English for a locale that is not translated', () => {
    app.getLocale = (): string => 'de-DE';

    expect(loadSettings().language).toBe('en');
  });

  it('picks the OS locale for settings saved before the language existed', () => {
    app.getLocale = (): string => 'fr';
    writeFileSync(join(directory, 'settings.json'), JSON.stringify({ wordWrap: false }), 'utf-8');

    const settings = loadSettings();

    expect(settings.language).toBe('fr');
    expect(settings.wordWrap).toBe(false);
  });

  it('keeps the language the user chose, whatever the OS locale', () => {
    app.getLocale = (): string => 'fr';
    writeFileSync(join(directory, 'settings.json'), JSON.stringify({ language: 'en' }), 'utf-8');

    expect(loadSettings().language).toBe('en');
    expect(storedLanguage()).toBe('en');
  });
});
