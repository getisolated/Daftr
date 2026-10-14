import { app } from 'electron/main';

/** Passed when Windows starts the app at login: start silently in the tray instead of opening the window. */
export const HIDDEN_ARG = '--hidden';

export function launchedHidden(): boolean {
  return process.argv.includes(HIDDEN_ARG);
}

/** Registers/unregisters the app in the OS login items. */
export function applyLaunchAtStartup(enabled: boolean): void {
  // In dev the login item would point to the bare electron.exe (without the app path): never register it
  if (!app.isPackaged) {
    console.warn('[Startup] Launch at startup is only applied in the packaged app');
    return;
  }
  app.setLoginItemSettings({ openAtLogin: enabled, args: [HIDDEN_ARG] });
}
