import { globalShortcut } from 'electron';

let registered: string | null = null;

/**
 * Registers `accelerator` as the single global shortcut. On failure (invalid, or already taken by another
 * application) the previous shortcut is kept, so the user is never left without one.
 */
export function registerGlobalShortcut(accelerator: string, onTrigger: () => void): boolean {
  if (accelerator === registered) {
    return true;
  }

  let ok = false;
  try {
    ok = globalShortcut.register(accelerator, onTrigger);
  } catch {
    ok = false;
  }

  if (ok) {
    if (registered) {
      globalShortcut.unregister(registered);
    }
    registered = accelerator;
  }
  return ok;
}

export function unregisterGlobalShortcuts(): void {
  globalShortcut.unregisterAll();
  registered = null;
}
