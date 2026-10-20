import type { Platform } from '../services/platform.service';

interface KeyEventLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  /** Legacy key code; for letters, browsers derive it from the unmodified character of the key in the current layout. */
  keyCode?: number;
}

const NAMED_KEYS: Record<string, string> = {
  Space: 'Space', Enter: 'Return', Tab: 'Tab', Backspace: 'Backspace', Delete: 'Delete', Insert: 'Insert',
  Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
  Minus: '-', Equal: '=', Comma: ',', Period: '.', Slash: '/', Backslash: '\\',
  Semicolon: ';', Quote: "'", BracketLeft: '[', BracketRight: ']', Backquote: '`',
};

const PUNCTUATION_KEYS = new Set(['-', '=', ',', '.', '/', '\\', ';', "'", '[', ']', '`']);

/**
 * Electron accelerator key name, or null for modifiers/unsupported keys.
 * Letters follow the keyboard layout (`key`), because Electron resolves an accelerator letter through the
 * active layout: on AZERTY the physical "Q" key (`code` KeyA) must be recorded as Q, not A. `key` is not a
 * plain letter when Option (macOS) or AltGr (Windows) produces a symbol: `keyCode` still names the letter of
 * the key in the current layout (WebKit and Chromium derive it from the unmodified character); the physical
 * key is the last resort.
 */
function keyName(key: string, code: string, keyCode?: number): string | null {
  if (/^[a-zA-Z]$/.test(key)) {
    return key.toUpperCase();
  }
  // Punctuation also follows the layout: on AZERTY the "," key is the physical M key (code KeyM)
  if (PUNCTUATION_KEYS.has(key)) {
    return key;
  }
  if (keyCode !== undefined && keyCode >= 65 && keyCode <= 90) {
    return String.fromCharCode(keyCode);
  }
  const letter = /^Key(?<letter>[A-Z])$/.exec(code)?.groups?.['letter'];
  if (letter) {
    return letter;
  }
  const digit = /^Digit(?<digit>[0-9])$/.exec(code)?.groups?.['digit'];
  if (digit) {
    return digit;
  }
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) {
    return code;
  }
  return NAMED_KEYS[code] ?? null;
}

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'AltGraph', 'Meta']);

/** True while the key pressed is a modifier alone (the start of typing a combination). */
export function isModifierKey(event: Pick<KeyEventLike, 'key'>): boolean {
  return MODIFIER_KEYS.has(event.key);
}

export interface AcceleratorOptions {
  /** Accept a key without modifier: the second key of a chord (`Ctrl+K, V`) does not need one. */
  allowBareKey?: boolean;
  /**
   * Platform the event comes from (default `windows`). On macOS ⌘ is the primary modifier, recorded as
   * `CommandOrControl`, and ⌃ is recorded as `Control`; elsewhere Ctrl is `CommandOrControl` and the
   * Windows key is `Super`.
   */
  platform?: Platform;
}

/**
 * Converts a keydown event into an Electron accelerator (e.g. `CommandOrControl+Alt+N`).
 * Returns null while only modifiers are pressed, or when the combination is not usable as a global
 * shortcut (a bare key without modifier would hijack normal typing system-wide; F-keys are the exception).
 */
export function keyEventToAccelerator(event: KeyEventLike, options: AcceleratorOptions = {}): string | null {
  const key = keyName(event.key, event.code, event.keyCode);
  if (!key) {
    return null;
  }

  const parts: string[] = [];
  if (options.platform === 'mac') {
    if (event.metaKey) {
      parts.push('CommandOrControl');
    }
    if (event.ctrlKey) {
      parts.push('Control');
    }
  } else {
    if (event.ctrlKey) {
      parts.push('CommandOrControl');
    }
    if (event.metaKey) {
      parts.push('Super');
    }
  }
  if (event.altKey) {
    parts.push('Alt');
  }
  if (event.shiftKey) {
    parts.push('Shift');
  }

  const isFunctionKey = /^F\d+$/.test(key);
  if (parts.length === 0 && !isFunctionKey && !options.allowBareKey) {
    return null;
  }
  return [...parts, key].join('+');
}

/**
 * Physical key (`KeyboardEvent.code`) of a letter or digit key, null for any other key. Stored with the global
 * shortcut: macOS registers global shortcuts by physical key, and the recorded letter follows the layout.
 */
export function physicalKeyOf(event: Pick<KeyEventLike, 'code'>): string | null {
  return /^Key[A-Z]$|^Digit[0-9]$/.test(event.code) ? event.code : null;
}

/** Modifier symbols of macOS, in the order Apple displays them (⌃ ⌥ ⇧ ⌘). */
const MAC_MODIFIERS: [symbol: string, names: string[]][] = [
  ['⌃', ['Control', 'Ctrl']],
  ['⌥', ['Alt', 'Option']],
  ['⇧', ['Shift']],
  ['⌘', ['CommandOrControl', 'CmdOrCtrl', 'Command', 'Cmd', 'Super', 'Meta']],
];

const MAC_KEY_SYMBOLS: Record<string, string> = {
  Return: '↩', Enter: '↩', Tab: '⇥', Backspace: '⌫', Delete: '⌦', Escape: '⎋', Space: '␣',
  Up: '↑', Down: '↓', Left: '←', Right: '→', PageUp: '⇞', PageDown: '⇟', Home: '↖', End: '↘',
};

/** One accelerator (no chord) in macOS notation: `CommandOrControl+Shift+F` → `⇧⌘F`. */
function formatMacAccelerator(accelerator: string): string {
  const parts = accelerator.split('+');
  // `+` cannot be a key name (`=` is recorded instead), so the last part is always the key
  const key = parts.pop() ?? '';
  const symbols = MAC_MODIFIERS.filter(([, names]) => parts.some(part => names.includes(part))).map(([symbol]) => symbol);
  const keyLabel = MAC_KEY_SYMBOLS[key] ?? (key.length === 1 ? key.toUpperCase() : key);
  return [...symbols, keyLabel].join('');
}

/**
 * Human-readable form of an accelerator: `Ctrl+Shift+F` (Windows, Linux) or `⇧⌘F` (macOS).
 * On macOS the two keys of a chord are separated by a space (`⌘K ⌘T`); elsewhere they keep the `, `.
 */
export function formatAccelerator(accelerator: string, platform: Platform = 'windows'): string {
  if (platform === 'mac') {
    return accelerator ? accelerator.split(', ').map(formatMacAccelerator).join(' ') : '';
  }
  return accelerator
    .replace(/CommandOrControl/g, 'Ctrl')
    .replace(/\bControl\b/g, 'Ctrl')
    .replace(/Super/g, 'Win')
    .replace(/Return/g, 'Enter');
}

/**
 * Combinations macOS keeps for itself (application switcher, Spotlight, Hide, Minimize, Quit, screenshots…):
 * the app would never receive them, or would break the system action. Accelerators as produced by
 * `keyEventToAccelerator` with `platform: 'mac'`.
 */
const MAC_RESERVED_ACCELERATORS = new Set([
  'CommandOrControl+Q', 'CommandOrControl+H', 'CommandOrControl+Alt+H', 'CommandOrControl+M',
  'CommandOrControl+Tab', 'CommandOrControl+Shift+Tab', 'CommandOrControl+Space', 'CommandOrControl+Alt+Space',
  'CommandOrControl+Control+Q', 'CommandOrControl+Control+Space', 'CommandOrControl+Control+F',
  'CommandOrControl+Shift+3', 'CommandOrControl+Shift+4', 'CommandOrControl+Shift+5',
]);

/** True when the operating system keeps this combination for itself, so it cannot be used as a shortcut. */
export function isReservedAccelerator(accelerator: string, platform: Platform = 'windows'): boolean {
  return platform === 'mac' && MAC_RESERVED_ACCELERATORS.has(accelerator);
}

const CODEMIRROR_MODIFIER_NAMES: Record<string, string> = {
  CommandOrControl: 'Mod', Control: 'Ctrl', Super: 'Meta',
};

const CODEMIRROR_KEY_NAMES: Record<string, string> = {
  Return: 'Enter', Up: 'ArrowUp', Down: 'ArrowDown', Left: 'ArrowLeft', Right: 'ArrowRight',
};

/**
 * Converts an accelerator into a CodeMirror key name (`CommandOrControl+Shift+B` → `Mod-Shift-b`).
 * Returns an empty string for an empty accelerator: CodeMirror then ignores the binding.
 */
export function acceleratorToCodeMirrorKey(accelerator: string): string {
  if (!accelerator) {
    return '';
  }
  const parts = accelerator.split('+');
  const key = parts.pop() ?? '';
  // CodeMirror parses modifiers in any order; `Mod` is ⌘ on macOS and Ctrl elsewhere, `Ctrl` is always ⌃/Ctrl
  const modifiers = parts.map(part => CODEMIRROR_MODIFIER_NAMES[part] ?? part);
  const name = CODEMIRROR_KEY_NAMES[key] ?? (key.length === 1 ? key.toLowerCase() : key);
  return [...modifiers, name].join('-');
}
