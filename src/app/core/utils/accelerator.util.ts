interface KeyEventLike {
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

/** Electron accelerator key name for a physical key (layout independent), or null for modifiers/unsupported keys. */
function keyName(code: string): string | null {
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
  const key = keyName(event.code);
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
