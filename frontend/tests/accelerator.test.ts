import { describe, it, expect } from 'vitest';
import { acceleratorToCodeMirrorKey, formatAccelerator, isModifierKey, isReservedAccelerator, keyEventToAccelerator, physicalKeyOf } from '../src/app/core/utils/accelerator.util';

const ev = (code: string, mods: Partial<{ key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean; keyCode: number }> = {}) =>
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

  it('names the layout letter from keyCode when Option produces a symbol (macOS AZERTY)', () => {
    // ⌥⌘ + the key marked "Q" on AZERTY (physical KeyA) types '‡', keyCode 81 = Q
    expect(keyEventToAccelerator(ev('KeyA', { key: '‡', keyCode: 81, metaKey: true, altKey: true }), { platform: 'mac' })).toBe('CommandOrControl+Alt+Q');
    // ⌥⌘ + the key marked "A" on AZERTY (physical KeyQ) types 'æ', keyCode 65 = A
    expect(keyEventToAccelerator(ev('KeyQ', { key: 'æ', keyCode: 65, metaKey: true, altKey: true }), { platform: 'mac' })).toBe('CommandOrControl+Alt+A');
    // Option dead keys (⌥E, ⌥N…) report 'Dead'
    expect(keyEventToAccelerator(ev('KeyE', { key: 'Dead', keyCode: 69, altKey: true }), { platform: 'mac' })).toBe('Alt+E');
  });

  it('names the layout letter from keyCode on QWERTY too', () => {
    expect(keyEventToAccelerator(ev('KeyQ', { key: 'œ', keyCode: 81, metaKey: true, altKey: true }), { platform: 'mac' })).toBe('CommandOrControl+Alt+Q');
    expect(keyEventToAccelerator(ev('KeyE', { key: '€', keyCode: 69, ctrlKey: true, altKey: true }))).toBe('CommandOrControl+Alt+E');
  });

  it('ignores a keyCode that is not a letter', () => {
    expect(keyEventToAccelerator(ev('Digit2', { key: '~', keyCode: 50, ctrlKey: true, altKey: true }))).toBe('CommandOrControl+Alt+2');
    expect(keyEventToAccelerator(ev('F5', { key: 'F5', keyCode: 116, ctrlKey: true }))).toBe('CommandOrControl+F5');
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

  it('allows a bare key when asked (second key of a chord)', () => {
    expect(keyEventToAccelerator(ev('KeyV', { key: 'v' }), { allowBareKey: true })).toBe('V');
    expect(keyEventToAccelerator(ev('KeyT', { key: 't', ctrlKey: true }), { allowBareKey: true })).toBe('CommandOrControl+T');
  });

  it('still returns null for an unsupported key, even bare keys allowed', () => {
    expect(keyEventToAccelerator(ev('Escape', { key: 'Escape' }), { allowBareKey: true })).toBeNull();
    expect(keyEventToAccelerator(ev('ShiftLeft', { key: 'Shift', shiftKey: true }), { allowBareKey: true })).toBeNull();
  });
});

describe('physicalKeyOf', () => {
  it('keeps the code of a letter or digit key', () => {
    expect(physicalKeyOf({ code: 'KeyA' })).toBe('KeyA');
    expect(physicalKeyOf({ code: 'Digit0' })).toBe('Digit0');
  });

  it('returns null for any other key', () => {
    ['F5', 'Space', 'Semicolon', 'Numpad1', 'ArrowUp', ''].forEach(code => expect(physicalKeyOf({ code })).toBeNull());
  });
});

describe('isModifierKey', () => {
  it('recognises a modifier pressed alone', () => {
    ['Control', 'Shift', 'Alt', 'AltGraph', 'Meta'].forEach(key => expect(isModifierKey({ key })).toBe(true));
  });

  it('rejects any other key', () => {
    ['t', 'Escape', 'F1', 'Enter'].forEach(key => expect(isModifierKey({ key })).toBe(false));
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

describe('keyEventToAccelerator on macOS', () => {
  const mac = { platform: 'mac' as const };

  it('records ⌘ as CommandOrControl and ⌃ as Control', () => {
    expect(keyEventToAccelerator(ev('KeyN', { key: 'n', metaKey: true }), mac)).toBe('CommandOrControl+N');
    expect(keyEventToAccelerator(ev('Tab', { key: 'Tab', ctrlKey: true }), mac)).toBe('Control+Tab');
    expect(keyEventToAccelerator(ev('KeyK', { key: 'k', metaKey: true, ctrlKey: true, altKey: true, shiftKey: true }), mac))
      .toBe('CommandOrControl+Control+Alt+Shift+K');
  });

  it('uses the physical key when ⌥ turns the letter into a symbol', () => {
    expect(keyEventToAccelerator(ev('KeyQ', { key: 'œ', metaKey: true, altKey: true }), mac)).toBe('CommandOrControl+Alt+Q');
  });

  it('keeps the Windows mapping by default and on Windows', () => {
    const event = ev('KeyN', { key: 'n', ctrlKey: true, metaKey: true });
    expect(keyEventToAccelerator(event)).toBe('CommandOrControl+Super+N');
    expect(keyEventToAccelerator(event, { platform: 'windows' })).toBe('CommandOrControl+Super+N');
    expect(keyEventToAccelerator(event, { platform: 'linux' })).toBe('CommandOrControl+Super+N');
  });

  it('accepts a bare second chord key', () => {
    expect(keyEventToAccelerator(ev('KeyV', { key: 'v' }), { ...mac, allowBareKey: true })).toBe('V');
    expect(keyEventToAccelerator(ev('KeyV', { key: 'v' }), mac)).toBeNull();
  });
});

describe('formatAccelerator by platform', () => {
  it('keeps the Windows notation', () => {
    expect(formatAccelerator('CommandOrControl+Shift+F', 'windows')).toBe('Ctrl+Shift+F');
    expect(formatAccelerator('Super+Return')).toBe('Win+Enter');
    expect(formatAccelerator('CommandOrControl+K, CommandOrControl+T')).toBe('Ctrl+K, Ctrl+T');
    expect(formatAccelerator('Control+Tab')).toBe('Ctrl+Tab');
  });

  it('uses Apple symbols, in the ⌃ ⌥ ⇧ ⌘ order, without +', () => {
    expect(formatAccelerator('CommandOrControl+N', 'mac')).toBe('⌘N');
    expect(formatAccelerator('CommandOrControl+Shift+F', 'mac')).toBe('⇧⌘F');
    expect(formatAccelerator('CommandOrControl+Control+Alt+Shift+K', 'mac')).toBe('⌃⌥⇧⌘K');
    expect(formatAccelerator('Shift+Alt+CommandOrControl+Control+K', 'mac')).toBe('⌃⌥⇧⌘K');
    expect(formatAccelerator('Control+Shift+Tab', 'mac')).toBe('⌃⇧⇥');
    expect(formatAccelerator('CommandOrControl+,', 'mac')).toBe('⌘,');
    expect(formatAccelerator('CommandOrControl+Alt+1', 'mac')).toBe('⌥⌘1');
    expect(formatAccelerator('Alt+W', 'mac')).toBe('⌥W');
    expect(formatAccelerator('F1', 'mac')).toBe('F1');
    expect(formatAccelerator('', 'mac')).toBe('');
  });

  it('maps named keys to their macOS symbol', () => {
    const cases: [string, string][] = [
      ['Return', '↩'], ['Tab', '⇥'], ['Backspace', '⌫'], ['Delete', '⌦'], ['Escape', '⎋'], ['Space', '␣'],
      ['Up', '↑'], ['Down', '↓'], ['Left', '←'], ['Right', '→'], ['PageUp', '⇞'], ['PageDown', '⇟'], ['Home', '↖'], ['End', '↘'],
    ];
    for (const [key, symbol] of cases) {
      expect(formatAccelerator(`CommandOrControl+${key}`, 'mac')).toBe(`⌘${symbol}`);
    }
    expect(formatAccelerator('CommandOrControl+a', 'mac')).toBe('⌘A');
  });

  it('separates the keys of a chord with a space', () => {
    expect(formatAccelerator('CommandOrControl+K, CommandOrControl+T', 'mac')).toBe('⌘K ⌘T');
    expect(formatAccelerator('CommandOrControl+K, V', 'mac')).toBe('⌘K V');
    expect(formatAccelerator('CommandOrControl+,, V', 'mac')).toBe('⌘, V');
  });
});

describe('isReservedAccelerator', () => {
  it('refuses the combinations macOS keeps for itself', () => {
    for (const accelerator of ['CommandOrControl+Q', 'CommandOrControl+H', 'CommandOrControl+M', 'CommandOrControl+Tab', 'CommandOrControl+Space']) {
      expect(isReservedAccelerator(accelerator, 'mac')).toBe(true);
    }
    expect(isReservedAccelerator('CommandOrControl+Shift+L', 'mac')).toBe(false);
    expect(isReservedAccelerator('Control+Tab', 'mac')).toBe(false);
  });

  it('reserves nothing on Windows', () => {
    expect(isReservedAccelerator('CommandOrControl+Q', 'windows')).toBe(false);
    expect(isReservedAccelerator('CommandOrControl+Q')).toBe(false);
  });
});

describe('acceleratorToCodeMirrorKey with macOS modifiers', () => {
  it('maps Control to Ctrl, CommandOrControl to Mod and Super to Meta', () => {
    expect(acceleratorToCodeMirrorKey('Control+Tab')).toBe('Ctrl-Tab');
    expect(acceleratorToCodeMirrorKey('CommandOrControl+Control+Alt+Shift+K')).toBe('Mod-Ctrl-Alt-Shift-k');
    expect(acceleratorToCodeMirrorKey('Super+B')).toBe('Meta-b');
  });
});
