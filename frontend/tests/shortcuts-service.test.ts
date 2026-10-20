import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { Injector } from '@angular/core';
import { resolveAppShortcuts } from '../src/app/core/models/app-shortcuts.model';
import { Platform, PlatformService } from '../src/app/core/services/platform.service';
import { SettingsService } from '../src/app/core/services/settings.service';
import { ShortcutsService, isMacMenuAccelerator, primaryModifierMatches } from '../src/app/core/services/shortcuts.service';

// The real SettingsService talks to Tauri: only `appShortcuts` is read by ShortcutsService
vi.mock('../src/app/core/services/settings.service', () => ({ SettingsService: class {} }));

// No DOM in the test environment: a window that only records the keydown listener, and an Element class for `instanceof`
let keydown: ((event: KeyboardEvent) => void) | null = null;
vi.stubGlobal('Element', class {});
vi.stubGlobal('window', {
  addEventListener: (type: string, listener: (event: KeyboardEvent) => void) => { if (type === 'keydown') keydown = listener; },
  removeEventListener: () => undefined,
});

interface FakeKeyEvent {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  target: null;
  defaultPrevented: boolean;
  preventDefault: () => void;
  stopPropagation: () => void;
}

const press = (code: string, key: string, mods: Partial<Pick<FakeKeyEvent, 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>> = {}): FakeKeyEvent => {
  const event: FakeKeyEvent = {
    key, code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods, target: null, defaultPrevented: false,
    preventDefault: () => { event.defaultPrevented = true; },
    stopPropagation: () => undefined,
  };
  keydown?.(event as unknown as KeyboardEvent);
  return event;
};

function createService(platform: Platform, overrides: Record<string, string> = {}): ShortcutsService {
  const platformService = { platform, isMac: platform === 'mac', isWindows: platform === 'windows' };
  const settings = { appShortcuts: () => resolveAppShortcuts(overrides, platform) };
  const injector = Injector.create({
    providers: [
      { provide: PlatformService, useValue: platformService },
      { provide: SettingsService, useValue: settings },
      { provide: ShortcutsService, useClass: ShortcutsService, deps: [] },
    ],
  });
  return injector.get(ShortcutsService);
}

describe('ShortcutsService', () => {
  let service: ShortcutsService;
  let calls: string[];
  const bindAll = (ids: string[]) => ids.forEach(id => service.bindAction(id, () => calls.push(id)));

  beforeEach(() => { calls = []; });
  afterEach(() => service.ngOnDestroy());

  describe('on Windows', () => {
    beforeEach(() => {
      service = createService('windows');
      bindAll(['note.new', 'tab.next', 'tab.prev', 'theme.select', 'editor.previewSide', 'editor.replace']);
    });

    it('runs app actions with Ctrl, not with the Windows key', () => {
      expect(press('KeyN', 'n', { ctrlKey: true }).defaultPrevented).toBe(true);
      press('KeyN', 'n', { metaKey: true });
      press('Tab', 'Tab', { ctrlKey: true });
      press('Tab', 'Tab', { ctrlKey: true, shiftKey: true });
      press('KeyH', 'h', { ctrlKey: true });
      expect(calls).toEqual(['note.new', 'tab.next', 'tab.prev', 'editor.replace']);
    });

    it('runs chords with Ctrl', () => {
      press('KeyK', 'k', { ctrlKey: true });
      press('KeyT', 't', { ctrlKey: true });
      press('KeyK', 'k', { ctrlKey: true });
      press('KeyV', 'v');
      expect(calls).toEqual(['theme.select', 'editor.previewSide']);
    });

    it('matches internal shortcuts with Ctrl (or the Windows key, as before)', () => {
      const reload: string[] = [];
      service.register({ key: 'r', ctrl: true, handler: () => reload.push('r') });
      expect(press('KeyR', 'r', { ctrlKey: true }).defaultPrevented).toBe(true);
      press('KeyR', 'r');
      expect(reload).toEqual(['r']);
    });

    it('consumes the key following a chord start, even Ctrl+C', () => {
      press('KeyK', 'k', { ctrlKey: true });
      expect(press('KeyC', 'c', { ctrlKey: true }).defaultPrevented).toBe(true);
    });
  });

  describe('on macOS', () => {
    beforeEach(() => {
      service = createService('mac');
      bindAll(['note.new', 'tab.next', 'tab.prev', 'theme.select', 'editor.previewSide', 'editor.replace', 'tab.close']);
    });

    it('runs CommandOrControl actions with ⌘ only, ⌃ alone does not trigger them', () => {
      expect(press('KeyN', 'n', { metaKey: true }).defaultPrevented).toBe(true);
      expect(press('KeyN', 'n', { ctrlKey: true }).defaultPrevented).toBe(false);
      press('KeyW', 'w', { metaKey: true });
      press('KeyF', 'ƒ', { metaKey: true, altKey: true });
      expect(calls).toEqual(['note.new', 'tab.close', 'editor.replace']);
    });

    it('cycles tabs with ⌃Tab (⌘Tab belongs to macOS)', () => {
      press('Tab', 'Tab', { ctrlKey: true });
      press('Tab', 'Tab', { ctrlKey: true, shiftKey: true });
      press('Tab', 'Tab', { metaKey: true });
      expect(calls).toEqual(['tab.next', 'tab.prev']);
    });

    it('runs chords with ⌘', () => {
      expect(press('KeyK', 'k', { metaKey: true }).defaultPrevented).toBe(true);
      press('KeyT', 't', { metaKey: true });
      press('KeyK', 'k', { metaKey: true });
      press('KeyV', 'v');
      expect(calls).toEqual(['theme.select', 'editor.previewSide']);
    });

    it('leaves the native menu combinations alone', () => {
      service.register({ key: 'c', ctrl: true, handler: () => calls.push('internal') });
      for (const [code, key, shiftKey] of [['KeyC', 'c', false], ['KeyV', 'v', false], ['KeyX', 'x', false], ['KeyA', 'a', false],
        ['KeyZ', 'z', false], ['KeyZ', 'Z', true], ['KeyQ', 'q', false], ['KeyH', 'h', false], ['KeyM', 'm', false]] as const) {
        expect(press(code, key, { metaKey: true, shiftKey }).defaultPrevented).toBe(false);
      }
      expect(calls).toEqual([]);
    });

    it('lets ⌘Q reach the menu after a chord start, and cancels the chord', () => {
      press('KeyK', 'k', { metaKey: true });
      expect(press('KeyQ', 'q', { metaKey: true }).defaultPrevented).toBe(false);
      press('KeyT', 't', { metaKey: true });
      expect(calls).toEqual([]);
    });

    it('still runs an app action the user bound to a menu combination', () => {
      service.ngOnDestroy();
      service = createService('mac', { 'note.new': 'CommandOrControl+Shift+Z' });
      bindAll(['note.new']);
      expect(press('KeyZ', 'Z', { metaKey: true, shiftKey: true }).defaultPrevented).toBe(true);
      expect(calls).toEqual(['note.new']);
    });

    it('matches internal shortcuts with ⌘, not with ⌃', () => {
      const reload: string[] = [];
      service.register({ key: 'r', ctrl: true, handler: () => reload.push('r') });
      expect(press('KeyR', 'r', { metaKey: true }).defaultPrevented).toBe(true);
      expect(press('KeyR', 'r', { ctrlKey: true }).defaultPrevented).toBe(false);
      expect(press('KeyR', 'r', { ctrlKey: true, metaKey: true }).defaultPrevented).toBe(false);
      expect(reload).toEqual(['r']);
    });
  });
});

describe('primaryModifierMatches', () => {
  it('uses ⌘ on macOS and Ctrl elsewhere', () => {
    expect(primaryModifierMatches(true, { ctrlKey: false, metaKey: true }, true)).toBe(true);
    expect(primaryModifierMatches(true, { ctrlKey: true, metaKey: false }, true)).toBe(false);
    expect(primaryModifierMatches(false, { ctrlKey: false, metaKey: false }, true)).toBe(true);
    expect(primaryModifierMatches(true, { ctrlKey: true, metaKey: false }, false)).toBe(true);
    expect(primaryModifierMatches(false, { ctrlKey: true, metaKey: false }, false)).toBe(false);
  });
});

describe('isMacMenuAccelerator', () => {
  it('only applies on macOS', () => {
    expect(isMacMenuAccelerator('CommandOrControl+C', true)).toBe(true);
    expect(isMacMenuAccelerator('CommandOrControl+C', false)).toBe(false);
    expect(isMacMenuAccelerator('CommandOrControl+N', true)).toBe(false);
    expect(isMacMenuAccelerator(null, true)).toBe(false);
  });
});
