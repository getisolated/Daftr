import { describe, it, expect } from 'vitest';
import { isReservedAccelerator } from '../src/app/core/utils/accelerator.util';
import { translate } from '../src/app/core/i18n/translate';
import { APP_SHORTCUTS, defaultAcceleratorFor, findShortcutConflict, resolveAppShortcuts, shortcutLabel } from '../src/app/core/models/app-shortcuts.model';

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

describe('platform defaults', () => {
  const byId = (id: string) => APP_SHORTCUTS.find(s => s.id === id)!;

  it('cycles tabs with ⌃Tab on macOS and Ctrl+Tab on Windows', () => {
    expect(defaultAcceleratorFor(byId('tab.next'), 'mac')).toBe('Control+Tab');
    expect(defaultAcceleratorFor(byId('tab.prev'), 'mac')).toBe('Control+Shift+Tab');
    expect(defaultAcceleratorFor(byId('tab.next'), 'windows')).toBe('CommandOrControl+Tab');
    expect(defaultAcceleratorFor(byId('tab.prev'))).toBe('CommandOrControl+Shift+Tab');
  });

  it('moves Replace off ⌘H (Hide) on macOS', () => {
    expect(defaultAcceleratorFor(byId('editor.replace'), 'mac')).toBe('CommandOrControl+Alt+F');
    expect(defaultAcceleratorFor(byId('editor.replace'), 'windows')).toBe('CommandOrControl+H');
  });

  it('keeps every other default identical on both systems', () => {
    const different = APP_SHORTCUTS.filter(s => defaultAcceleratorFor(s, 'mac') !== defaultAcceleratorFor(s, 'windows')).map(s => s.id);
    expect(different).toEqual(['tab.next', 'tab.prev', 'editor.replace']);
    expect(APP_SHORTCUTS.every(s => defaultAcceleratorFor(s, 'linux') === s.defaultAccelerator)).toBe(true);
  });

  it('has unique macOS defaults, none of them reserved by macOS and none clashing with another', () => {
    const resolved = resolveAppShortcuts({}, 'mac');
    const accelerators = Object.values(resolved);
    expect(new Set(accelerators).size).toBe(accelerators.length);
    for (const shortcut of APP_SHORTCUTS) {
      for (const part of resolved[shortcut.id].split(', ')) {
        expect(isReservedAccelerator(part, 'mac')).toBe(false);
      }
      expect(findShortcutConflict(shortcut.id, resolved[shortcut.id], resolved)).toBeNull();
    }
  });

  it('resolves the macOS defaults, overrides still winning without migration', () => {
    expect(resolveAppShortcuts({}, 'mac')['tab.next']).toBe('Control+Tab');
    expect(resolveAppShortcuts({}, 'mac')['note.new']).toBe('CommandOrControl+N');
    expect(resolveAppShortcuts({ 'tab.next': 'CommandOrControl+Alt+Right' }, 'mac')['tab.next']).toBe('CommandOrControl+Alt+Right');
    expect(resolveAppShortcuts({})['tab.next']).toBe('CommandOrControl+Tab');
  });
});
