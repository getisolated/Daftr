import { Language } from './settings-schema';

/** Texts of the main process (tray menu, native dialogs); the ones of the windows live in src/app/core/i18n. */
const MESSAGES = {
  en: {
    'tray.openApp': 'Open Everything Note',
    'tray.quickNote': 'Quick note',
    'tray.settings': 'Settings…',
    'tray.quit': 'Quit',
    'dialog.openFolder': 'Open Folder as Collection',
    'dialog.openFile': 'Open Markdown File',
    'filter.markdown': 'Markdown',
    'filter.html': 'HTML page',
    'filter.zip': 'ZIP archive',
  },
  fr: {
    'tray.openApp': 'Ouvrir Everything Note',
    'tray.quickNote': 'Note rapide',
    'tray.settings': 'Paramètres…',
    'tray.quit': 'Quitter',
    'dialog.openFolder': 'Ouvrir un dossier comme collection',
    'dialog.openFile': 'Ouvrir un fichier Markdown',
    'filter.markdown': 'Markdown',
    'filter.html': 'Page HTML',
    'filter.zip': 'Archive ZIP',
  },
} as const satisfies Record<Language, Record<string, string>>;

export type MessageKey = keyof (typeof MESSAGES)['en'];

export function message(language: Language, key: MessageKey): string {
  return MESSAGES[language][key];
}
