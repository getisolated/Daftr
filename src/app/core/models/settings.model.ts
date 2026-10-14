export type ShortcutMode = 'overlay' | 'app';

export interface AppSettings {
  globalShortcut: string;
  shortcutMode: ShortcutMode;
  overlayWidth: number;
  overlayHeight: number;
  /** true: Enter sends (Shift+Enter = new line). false: Enter = new line (Ctrl+Enter sends). */
  enterToSend: boolean;
  launchAtStartup: boolean;
  /** true: updates download in the background and install when the app quits. false: the user is asked first. */
  autoUpdate: boolean;
  /** In-app shortcuts changed by the user, by shortcut id (see APP_SHORTCUTS); absent = default. */
  appShortcuts: Record<string, string>;
}

export type SettingsPatch = Partial<AppSettings>;

export interface SettingsUpdateResult {
  ok: boolean;
  error?: 'shortcut-unavailable';
  settings: AppSettings;
}

export const DEFAULT_SETTINGS: AppSettings = {
  globalShortcut: 'CommandOrControl+Alt+N',
  shortcutMode: 'overlay',
  overlayWidth: 250,
  overlayHeight: 350,
  enterToSend: true,
  launchAtStartup: false,
  autoUpdate: false,
  appShortcuts: {},
};

export const OVERLAY_NOTE_TAG = 'overlay';
