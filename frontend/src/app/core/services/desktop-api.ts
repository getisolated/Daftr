import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { IpcError } from '../bindings/IpcError';

/** True inside the desktop app (Tauri); false in a plain browser (`ng serve` alone, tests). */
export function inDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Label of the current window: `main` or `overlay` (see the Rust crate `desktop`). */
export function currentWindowLabel(): string | null {
  return inDesktop() ? getCurrentWebviewWindow().label : null;
}

function isIpcError(value: unknown): value is IpcError {
  return typeof value === 'object' && value !== null && typeof (value as IpcError).message === 'string';
}

/** Calls a command of the main process; a failure rejects with an `Error` carrying its message (translated by callers). */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(isIpcError(error) ? error.message : String(error));
  }
}

/**
 * Listens to an event pushed by the main process to this window. Returns an unsubscribe function usable right away,
 * even before the listener is registered.
 */
export function subscribe<T>(event: string, callback: (payload: T) => void): () => void {
  if (!inDesktop()) {
    return () => undefined;
  }

  let unlisten: (() => void) | null = null;
  let cancelled = false;

  void getCurrentWebviewWindow().listen<T>(event, message => callback(message.payload)).then(stop => {
    if (cancelled) {
      stop();
    } else {
      unlisten = stop;
    }
  });

  return () => {
    cancelled = true;
    unlisten?.();
  };
}
