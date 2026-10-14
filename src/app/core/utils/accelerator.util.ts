interface KeyEventLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
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
 * plain letter when AltGr produces a symbol; the physical key is the best remaining guess then.
 */
function keyName(key: string, code: string): string | null {
  if (/^[a-zA-Z]$/.test(key)) {
    return key.toUpperCase();
  }
  // Punctuation also follows the layout: on AZERTY the "," key is the physical M key (code KeyM)
  if (PUNCTUATION_KEYS.has(key)) {
    return key;
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

/**
 * Converts a keydown event into an Electron accelerator (e.g. `CommandOrControl+Alt+N`).
 * Returns null while only modifiers are pressed, or when the combination is not usable as a global
 * shortcut (a bare key without modifier would hijack normal typing system-wide; F-keys are the exception).
 */
export function keyEventToAccelerator(event: KeyEventLike): string | null {
  const key = keyName(event.key, event.code);
  if (!key) {
    return null;
  }

  const parts: string[] = [];
  if (event.ctrlKey) {
    parts.push('CommandOrControl');
  }
  if (event.metaKey) {
    parts.push('Super');
  }
  if (event.altKey) {
    parts.push('Alt');
  }
  if (event.shiftKey) {
    parts.push('Shift');
  }

  const isFunctionKey = /^F\d+$/.test(key);
  if (parts.length === 0 && !isFunctionKey) {
    return null;
  }
  return [...parts, key].join('+');
}

/** Human-readable form of an accelerator. */
export function formatAccelerator(accelerator: string): string {
  return accelerator.replace(/CommandOrControl/g, 'Ctrl').replace(/Super/g, 'Win').replace(/Return/g, 'Enter');
}

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
  const modifiers = parts.map(part => (part === 'CommandOrControl' ? 'Mod' : part === 'Super' ? 'Meta' : part));
  const name = CODEMIRROR_KEY_NAMES[key] ?? (key.length === 1 ? key.toLowerCase() : key);
  return [...modifiers, name].join('-');
}
