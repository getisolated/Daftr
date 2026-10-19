import { app, Menu, Tray } from 'electron/main';
import * as path from 'path';
import { message } from './i18n';
import { Language } from './settings-schema';

export interface TrayActions {
  openApp: () => void;
  quickNote: () => void;
  openSettings: () => void;
  quit: () => void;
}

let tray: Tray | null = null;
let trayActions: TrayActions | null = null;

/** Creates the notification-area icon: the app keeps running there when its window is closed. */
export function createTray(actions: TrayActions, language: Language): Tray {
  trayActions = actions;
  tray = new Tray(path.join(__dirname, '../assets/logo.ico'));
  tray.setToolTip(app.getName());
  tray.on('click', actions.openApp);
  setTrayLanguage(language);
  return tray;
}

/** Builds the menu in a language; called again when the user changes it. */
export function setTrayLanguage(language: Language): void {
  if (!tray || !trayActions) {
    return;
  }

  const actions = trayActions;
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: message(language, 'tray.openApp'), click: actions.openApp },
    { label: message(language, 'tray.quickNote'), click: actions.quickNote },
    { label: message(language, 'tray.settings'), click: actions.openSettings },
    { type: 'separator' },
    { label: message(language, 'tray.quit'), click: actions.quit },
  ]));
}

export function destroyTray(): void {
  tray?.destroy();
  tray = null;
  trayActions = null;
}
