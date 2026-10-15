import { app, Menu, Tray } from 'electron/main';
import * as path from 'path';

export interface TrayActions {
  openApp: () => void;
  quickNote: () => void;
  openSettings: () => void;
  quit: () => void;
}

let tray: Tray | null = null;

/** Creates the notification-area icon: the app keeps running there when its window is closed. */
export function createTray(actions: TrayActions): Tray {
  tray = new Tray(path.join(__dirname, '../assets/logo.ico'));
  tray.setToolTip(app.getName());
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: 'Open Everything Note', click: actions.openApp },
    { label: 'Quick note', click: actions.quickNote },
    { label: 'Settings…', click: actions.openSettings },
    { type: 'separator' },
    { label: 'Quit', click: actions.quit },
  ]));
  tray.on('click', actions.openApp);
  return tray;
}

export function destroyTray(): void {
  tray?.destroy();
  tray = null;
}
