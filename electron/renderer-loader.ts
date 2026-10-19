import { BrowserWindow } from 'electron/main';
import * as path from 'path';

const isDev = process.env['NODE_ENV'] === 'development';

/** Loads the Angular app in a window; `hash` selects which root component main.ts bootstraps. */
export function loadRenderer(win: BrowserWindow, hash?: string): void {
  if (isDev) {
    win.loadURL(`http://localhost:4200${hash ? `/#${hash}` : ''}`);
  } else {
    win.loadFile(path.join(__dirname, '../frontend/dist/browser/index.html'), hash ? { hash } : undefined);
  }
}
