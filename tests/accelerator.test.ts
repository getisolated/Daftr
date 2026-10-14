import { describe, it, expect } from 'vitest';
import { formatAccelerator, keyEventToAccelerator } from '../src/app/core/utils/accelerator.util';

const ev = (code: string, mods: Partial<{ ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }> = {}) =>
  ({ code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods });

describe('keyEventToAccelerator', () => {
  it('builds an accelerator from modifiers + letter', () => {
    expect(keyEventToAccelerator(ev('KeyN', { ctrlKey: true, altKey: true }))).toBe('CommandOrControl+Alt+N');
  });

  it('supports digits, named keys and shift', () => {
    expect(keyEventToAccelerator(ev('Digit5', { ctrlKey: true, shiftKey: true }))).toBe('CommandOrControl+Shift+5');
    expect(keyEventToAccelerator(ev('Space', { altKey: true }))).toBe('Alt+Space');
    expect(keyEventToAccelerator(ev('ArrowUp', { ctrlKey: true }))).toBe('CommandOrControl+Up');
  });

  it('returns null while only a modifier is pressed', () => {
    expect(keyEventToAccelerator(ev('ControlLeft', { ctrlKey: true }))).toBeNull();
  });

  it('refuses a bare key without modifier', () => {
    expect(keyEventToAccelerator(ev('KeyA'))).toBeNull();
  });

  it('allows a bare function key', () => {
    expect(keyEventToAccelerator(ev('F9'))).toBe('F9');
  });
});

describe('formatAccelerator', () => {
  it('prints a readable shortcut', () => {
    expect(formatAccelerator('CommandOrControl+Alt+N')).toBe('Ctrl+Alt+N');
  });
});
