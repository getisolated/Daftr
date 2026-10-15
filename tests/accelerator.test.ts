import { describe, it, expect } from 'vitest';
import { acceleratorToCodeMirrorKey, formatAccelerator, keyEventToAccelerator } from '../src/app/core/utils/accelerator.util';

const ev = (code: string, mods: Partial<{ key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }> = {}) =>
  ({ key: '', code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods });

describe('keyEventToAccelerator', () => {
  it('builds an accelerator from modifiers + letter', () => {
    expect(keyEventToAccelerator(ev('KeyN', { ctrlKey: true, altKey: true }))).toBe('CommandOrControl+Alt+N');
  });

  it('follows the keyboard layout for letters (AZERTY: physical KeyA types Q)', () => {
    expect(keyEventToAccelerator(ev('KeyA', { key: 'q', ctrlKey: true, altKey: true }))).toBe('CommandOrControl+Alt+Q');
  });

  it('falls back to the physical key when the layout key is not a letter (AltGr symbol)', () => {
    expect(keyEventToAccelerator(ev('KeyE', { key: '€', ctrlKey: true, altKey: true }))).toBe('CommandOrControl+Alt+E');
  });

  it('follows the layout for punctuation (AZERTY: the "," key is the physical M key)', () => {
    expect(keyEventToAccelerator(ev('KeyM', { key: ',', ctrlKey: true }))).toBe('CommandOrControl+,');
    expect(keyEventToAccelerator(ev('Comma', { key: ',', ctrlKey: true }))).toBe('CommandOrControl+,');
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

describe('acceleratorToCodeMirrorKey', () => {
  it('maps modifiers and lowercases letters', () => {
    expect(acceleratorToCodeMirrorKey('CommandOrControl+Shift+B')).toBe('Mod-Shift-b');
    expect(acceleratorToCodeMirrorKey('CommandOrControl+Alt+K')).toBe('Mod-Alt-k');
  });

  it('maps named keys', () => {
    expect(acceleratorToCodeMirrorKey('Alt+Up')).toBe('Alt-ArrowUp');
    expect(acceleratorToCodeMirrorKey('CommandOrControl+Return')).toBe('Mod-Enter');
    expect(acceleratorToCodeMirrorKey('F5')).toBe('F5');
    expect(acceleratorToCodeMirrorKey('CommandOrControl+,')).toBe('Mod-,');
  });

  it('returns an empty key for an empty accelerator', () => {
    expect(acceleratorToCodeMirrorKey('')).toBe('');
  });
});
