import { describe, it, expect } from 'vitest';
import { translate } from '../src/app/core/i18n/translate';
import { APP_SHORTCUTS, findShortcutConflict, resolveAppShortcuts, shortcutLabel } from '../src/app/core/models/app-shortcuts.model';

describe('APP_SHORTCUTS', () => {
  it('has unique ids and unique default accelerators', () => {
    const ids = APP_SHORTCUTS.map(s => s.id);
    const accelerators = APP_SHORTCUTS.map(s => s.defaultAccelerator);
    expect(new Set(ids).size).toBe(ids.length);
    expect(new Set(accelerators).size).toBe(accelerators.length);
  });

  it('has a translated label in every language', () => {
    for (const language of ['en', 'fr'] as const) {
      const labels = APP_SHORTCUTS.map(s => shortcutLabel(s, (key, params) => translate(language, key, params)));
      expect(new Set(labels).size).toBe(labels.length);
    }
    const goto = APP_SHORTCUTS.find(s => s.id === 'tab.goto.2')!;
    expect(shortcutLabel(goto, (key, params) => translate('fr', key, params))).toBe("Aller à l'onglet 2");
  });
});

describe('resolveAppShortcuts', () => {
  it('uses the defaults without overrides', () => {
    expect(resolveAppShortcuts({})['note.new']).toBe('CommandOrControl+N');
  });

  it('lets an override win and leaves the others on their default', () => {
    const resolved = resolveAppShortcuts({ 'note.new': 'CommandOrControl+Alt+M' });
    expect(resolved['note.new']).toBe('CommandOrControl+Alt+M');
    expect(resolved['tab.close']).toBe('CommandOrControl+W');
  });

  it('ignores overrides of unknown shortcuts', () => {
    expect('ghost' in resolveAppShortcuts({ ghost: 'Alt+G' })).toBe(false);
  });
});

describe('findShortcutConflict', () => {
  const resolved = resolveAppShortcuts({});

  it('finds the other shortcut using the accelerator', () => {
    expect(findShortcutConflict('note.new', 'CommandOrControl+W', resolved)?.id).toBe('tab.close');
  });

  it('does not flag a shortcut against itself, nor a free accelerator', () => {
    expect(findShortcutConflict('note.new', 'CommandOrControl+N', resolved)).toBeNull();
    expect(findShortcutConflict('note.new', 'CommandOrControl+Alt+M', resolved)).toBeNull();
  });

  it('refuses the first key of an existing chord, and a chord starting with an existing shortcut', () => {
    expect(findShortcutConflict('note.new', 'CommandOrControl+K', resolved)?.id).toBe('theme.select');
    const custom = resolveAppShortcuts({ 'tab.close': 'Alt+Q' });
    expect(findShortcutConflict('note.new', 'Alt+Q, Alt+R', custom)?.id).toBe('tab.close');
  });

  it('takes overrides into account', () => {
    const custom = resolveAppShortcuts({ 'tab.close': 'Alt+Q' });
    expect(findShortcutConflict('note.new', 'CommandOrControl+W', custom)).toBeNull();
    expect(findShortcutConflict('note.new', 'Alt+Q', custom)?.id).toBe('tab.close');
  });
});
