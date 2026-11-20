import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { translate } from '../src/app/core/i18n/translate';
import { Tab } from '../src/app/core/models/note.model';
import { Profile } from '../src/app/core/models/profile.model';
import { HoverIntent, profileMenuPosition } from '../src/app/core/utils/profile-menu.util';
import {
  iconMimeType, passwordChange, profileDisplayName, profileErrorMessage, profileInitial, profileStorageKey, squareCrop,
} from '../src/app/core/utils/profile.util';
import { dropTabsWhere } from '../src/app/core/utils/tabs-closing.util';
import { transferMenu, transferTarget } from '../src/app/core/utils/transfer-menu.util';

const en = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>): string => translate('en', key, params);
const fr = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>): string => translate('fr', key, params);

function profile(id: string, overrides: Partial<Profile> = {}): Profile {
  return { id, name: null, hasIcon: false, protected: false, isDefault: id === 'default', ...overrides };
}

describe('profileStorageKey', () => {
  it('keeps the keys of before the profiles for the default profile', () => {
    expect(profileStorageKey('daftr-session', 'default')).toBe('daftr-session');
    expect(profileStorageKey('daftr-session', null)).toBe('daftr-session');
  });

  it('gives every other profile its own key', () => {
    expect(profileStorageKey('daftr-session', 'a1b2c3')).toBe('daftr-session@a1b2c3');
    expect(profileStorageKey('daftr-sidebar', 'a1b2c3')).not.toBe(profileStorageKey('daftr-sidebar', 'ffff'));
  });
});

describe('squareCrop', () => {
  it('takes the centred square of a landscape image and scales it down', () => {
    expect(squareCrop(1000, 600, 256)).toEqual({ sx: 200, sy: 0, size: 600, target: 256 });
  });

  it('takes the centred square of a portrait image', () => {
    expect(squareCrop(300, 501, 256)).toEqual({ sx: 0, sy: 100, size: 300, target: 256 });
  });

  it('never scales a small image up', () => {
    expect(squareCrop(64, 64, 256)).toEqual({ sx: 0, sy: 0, size: 64, target: 64 });
    expect(squareCrop(0, 0, 256).size).toBe(1);
  });
});

describe('iconMimeType', () => {
  it('recognizes PNG and WebP by their signature', () => {
    expect(iconMimeType(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0]))).toBe('image/png');
    const webp = new TextEncoder().encode('RIFF\0\0\0\0WEBPVP8 ');
    expect(iconMimeType(webp)).toBe('image/webp');
  });

  it('refuses anything else', () => {
    expect(iconMimeType(new TextEncoder().encode('GIF89a'))).toBeNull();
    expect(iconMimeType(new Uint8Array())).toBeNull();
  });
});

describe('profile names', () => {
  it('shows the initial of the name, or nothing without a name', () => {
    expect(profileInitial('  élise')).toBe('É');
    expect(profileInitial('😀 fun')).toBe('😀');
    expect(profileInitial('   ')).toBeNull();
    expect(profileInitial(null)).toBeNull();
  });

  it('falls back to a translated placeholder', () => {
    expect(profileDisplayName(profile('default'), fr)).toBe('Profil par défaut');
    expect(profileDisplayName(profile('x1'), en)).toBe('Unnamed profile');
    expect(profileDisplayName(profile('x1', { name: 'Work' }), en)).toBe('Work');
  });
});

describe('passwordChange', () => {
  const fields = { current: '', next: '', confirm: '', remove: false };

  it('keeps the password when no new one is typed', () => {
    expect(passwordChange(false, fields)).toEqual({ kind: 'keep' });
    expect(passwordChange(true, { ...fields, current: 'old' })).toEqual({ kind: 'keep' });
  });

  it('sets a first password once confirmed', () => {
    expect(passwordChange(false, { ...fields, next: 'pw', confirm: 'pw' })).toEqual({ kind: 'set', current: null, next: 'pw' });
    expect(passwordChange(false, { ...fields, next: 'pw', confirm: 'pW' })).toBe('profiles.settings.passwordMismatch');
  });

  it('asks the current password to change or remove an existing one', () => {
    expect(passwordChange(true, { ...fields, next: 'new', confirm: 'new' })).toBe('profiles.settings.currentRequired');
    expect(passwordChange(true, { ...fields, current: 'old', next: 'new', confirm: 'new' }))
      .toEqual({ kind: 'set', current: 'old', next: 'new' });
    expect(passwordChange(true, { ...fields, remove: true })).toBe('profiles.settings.currentRequired');
    expect(passwordChange(true, { ...fields, current: 'old', remove: true })).toEqual({ kind: 'remove', current: 'old' });
  });

  it('ignores the removal box of an unprotected profile', () => {
    expect(passwordChange(false, { ...fields, remove: true })).toEqual({ kind: 'keep' });
  });
});

describe('profileErrorMessage', () => {
  it('translates a wrong password and wraps the other failures', () => {
    expect(profileErrorMessage(new Error('Wrong password'), fr)).toBe('Mot de passe incorrect.');
    expect(profileErrorMessage(new Error('disk full'), en)).toBe('The operation failed: disk full');
  });
});

describe('transfer menu', () => {
  it('lists the other profiles, a protected one marked by a lock', () => {
    const menu = transferMenu(en, [profile('default'), profile('w1', { name: 'Work', protected: true })]);

    expect(menu?.label).toBe('Transfer to Profile');
    expect(menu?.children?.map(child => [child.id, child.label, child.icon !== undefined])).toEqual([
      ['transfer:default', 'Default profile', false],
      ['transfer:w1', 'Work', true],
    ]);
  });

  it('has no entry without another profile', () => {
    expect(transferMenu(en, [])).toBeNull();
  });

  it('reads the target profile back from the action', () => {
    expect(transferTarget('transfer:w1')).toBe('w1');
    expect(transferTarget('transfer:')).toBeNull();
    expect(transferTarget('moveTo')).toBeNull();
  });
});

describe('profileMenuPosition', () => {
  const viewport = { width: 1200, height: 800 };

  it('opens on the right of the item, overlapping it, bottom edges aligned', () => {
    expect(profileMenuPosition({ right: 260, bottom: 770 }, viewport, 240)).toEqual({ left: 256, bottom: 30 });
  });

  it('stays inside the window', () => {
    expect(profileMenuPosition({ right: 1150, bottom: 799 }, viewport, 240)).toEqual({ left: 952, bottom: 8 });
  });
});

describe('HoverIntent', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('opens after a short hover and closes after the pointer left', () => {
    const states: boolean[] = [];
    const hover = new HoverIntent(open => states.push(open), 80, 250);

    hover.enter();
    vi.advanceTimersByTime(80);
    hover.leave();
    vi.advanceTimersByTime(249);
    expect(states).toEqual([true]);

    vi.advanceTimersByTime(1);
    expect(states).toEqual([true, false]);
  });

  it('stays open while the pointer travels from the item to the menu', () => {
    const states: boolean[] = [];
    const hover = new HoverIntent(open => states.push(open), 80, 250);

    hover.enter();
    vi.advanceTimersByTime(100);
    hover.leave();
    vi.advanceTimersByTime(150);
    hover.enter();
    vi.advanceTimersByTime(1000);

    expect(states).not.toContain(false);
  });

  it('does not open when the pointer only passes by', () => {
    const states: boolean[] = [];
    const hover = new HoverIntent(open => states.push(open), 80, 250);

    hover.enter();
    vi.advanceTimersByTime(40);
    hover.cancel();
    vi.advanceTimersByTime(1000);

    expect(states).toEqual([]);
  });
});

describe('dropTabsWhere', () => {
  function tab(noteId: number, isPinned = false): Tab {
    return { noteId, title: `n${noteId}`, isDirty: false, isPinned };
  }

  it('removes the transferred documents, pinned tabs included', () => {
    const result = dropTabsWhere([tab(1, true), tab(2), tab(3)], 3, t => t.noteId !== 3);

    expect(result.tabs.map(t => t.noteId)).toEqual([3]);
    expect(result.activeIndex).toBe(0);
  });

  it('goes home when the active tab left', () => {
    expect(dropTabsWhere([tab(1), tab(2)], 2, t => t.noteId === 2)).toEqual({ tabs: [tab(1)], activeIndex: -1 });
  });
});
