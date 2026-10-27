import { TranslationKey } from '../i18n/translations';
import { Translator } from '../i18n/translate';
import type { Platform } from '../services/platform.service';

export interface AppShortcutDefinition {
  id: string;
  /** Translation key of the name shown in the settings and in conflicts (see `shortcutLabel`). */
  labelKey: TranslationKey;
  labelParams?: Record<string, string | number>;
  /**
   * Electron-style accelerator, as produced by `keyEventToAccelerator` (e.g. `CommandOrControl+Shift+F`).
   * A chord is two accelerators separated by `, ` (e.g. `CommandOrControl+K, CommandOrControl+T`).
   */
  defaultAccelerator: string;
  /**
   * Default on macOS when it differs from `defaultAccelerator` (read through `defaultAcceleratorFor`).
   * `CommandOrControl` already means ⌘ there, so only the combinations macOS uses differently need one.
   */
  macDefaultAccelerator?: string;
}

/** Every in-app shortcut the user can rebind. Global (system-wide) shortcut lives in its own setting. */
export const APP_SHORTCUTS: AppShortcutDefinition[] = [
  { id: 'note.new', labelKey: 'settings.shortcut.note.new', defaultAccelerator: 'CommandOrControl+N' },
  { id: 'search.global', labelKey: 'settings.shortcut.search.global', defaultAccelerator: 'CommandOrControl+Shift+F' },
  { id: 'palette.commands', labelKey: 'settings.shortcut.palette.commands', defaultAccelerator: 'F1' },
  { id: 'settings.open', labelKey: 'settings.shortcut.settings.open', defaultAccelerator: 'CommandOrControl+,' },
  { id: 'layout.toggle', labelKey: 'settings.shortcut.layout.toggle', defaultAccelerator: 'Alt+W' },
  { id: 'editor.wordWrap', labelKey: 'settings.shortcut.editor.wordWrap', defaultAccelerator: 'Alt+Z' },
  // Not Ctrl+B: it is Bold in the editor
  { id: 'theme.select', labelKey: 'settings.shortcut.theme.select', defaultAccelerator: 'CommandOrControl+K, CommandOrControl+T' },
  // Like VS Code: Ctrl+K then Z alone
  { id: 'layout.zen', labelKey: 'settings.shortcut.layout.zen', defaultAccelerator: 'CommandOrControl+K, Z' },
  { id: 'formattingBar.toggle', labelKey: 'settings.shortcut.formattingBar.toggle', defaultAccelerator: 'Alt+T' },
  { id: 'sidebar.toggle', labelKey: 'settings.shortcut.sidebar.toggle', defaultAccelerator: 'CommandOrControl+Shift+E' },
  { id: 'tab.close', labelKey: 'settings.shortcut.tab.close', defaultAccelerator: 'CommandOrControl+W' },
  // ⌘Tab is the macOS application switcher: tabs cycle with ⌃Tab there, like Safari and VS Code
  { id: 'tab.next', labelKey: 'settings.shortcut.tab.next', defaultAccelerator: 'CommandOrControl+Tab', macDefaultAccelerator: 'Control+Tab' },
  { id: 'tab.prev', labelKey: 'settings.shortcut.tab.prev', defaultAccelerator: 'CommandOrControl+Shift+Tab', macDefaultAccelerator: 'Control+Shift+Tab' },
  { id: 'tab.pin', labelKey: 'settings.shortcut.tab.pin', defaultAccelerator: 'Alt+P' },
  ...Array.from({ length: 9 }, (_, i) => ({
    id: `tab.goto.${i + 1}`, labelKey: 'settings.shortcut.tab.goto' as const, labelParams: { number: i + 1 }, defaultAccelerator: `CommandOrControl+Alt+${i + 1}`,
  })),
  { id: 'editor.find', labelKey: 'settings.shortcut.editor.find', defaultAccelerator: 'CommandOrControl+F' },
  // ⌘H hides the application on macOS: ⌥⌘F there, like VS Code and Xcode
  { id: 'editor.replace', labelKey: 'settings.shortcut.editor.replace', defaultAccelerator: 'CommandOrControl+H', macDefaultAccelerator: 'CommandOrControl+Alt+F' },
  { id: 'editor.preview', labelKey: 'settings.shortcut.editor.preview', defaultAccelerator: 'CommandOrControl+Shift+V' },
  // Like VS Code: Ctrl+K then V alone
  { id: 'editor.previewSide', labelKey: 'settings.shortcut.editor.previewSide', defaultAccelerator: 'CommandOrControl+K, V' },
  { id: 'editor.save', labelKey: 'settings.shortcut.editor.save', defaultAccelerator: 'CommandOrControl+S' },
  { id: 'editor.bold', labelKey: 'settings.shortcut.editor.bold', defaultAccelerator: 'CommandOrControl+B' },
  { id: 'editor.italic', labelKey: 'settings.shortcut.editor.italic', defaultAccelerator: 'CommandOrControl+I' },
  // Ctrl+D adds the next match to the selection, like VS Code
  { id: 'editor.draft', labelKey: 'settings.shortcut.editor.draft', defaultAccelerator: 'CommandOrControl+Shift+D' },
  { id: 'editor.selectNextMatch', labelKey: 'settings.shortcut.editor.selectNextMatch', defaultAccelerator: 'CommandOrControl+D' },
  { id: 'editor.selectAllMatches', labelKey: 'settings.shortcut.editor.selectAllMatches', defaultAccelerator: 'CommandOrControl+Shift+L' },
  // Not Ctrl+K: it starts the chords (theme)
  { id: 'editor.link', labelKey: 'settings.shortcut.editor.link', defaultAccelerator: 'CommandOrControl+Shift+K' },
];

/** Default accelerator of a shortcut on the given platform. */
export function defaultAcceleratorFor(shortcut: AppShortcutDefinition, platform: Platform = 'windows'): string {
  return platform === 'mac' ? shortcut.macDefaultAccelerator ?? shortcut.defaultAccelerator : shortcut.defaultAccelerator;
}

/**
 * Accelerator in effect for every shortcut: the user's override when there is one, the platform default otherwise.
 * Overrides are never migrated: `CommandOrControl+…` stays valid on both systems (Ctrl on Windows, ⌘ on macOS).
 */
export function resolveAppShortcuts(overrides: Record<string, string>, platform: Platform = 'windows'): Record<string, string> {
  const resolved: Record<string, string> = {};
  for (const shortcut of APP_SHORTCUTS) {
    resolved[shortcut.id] = overrides[shortcut.id] ?? defaultAcceleratorFor(shortcut, platform);
  }
  return resolved;
}

/** Same combination, or one being the first key of the other's chord (which would then never fire). */
function acceleratorsClash(first: string, second: string): boolean {
  return first === second || first.startsWith(`${second}, `) || second.startsWith(`${first}, `);
}

/** Another shortcut already using this accelerator, if any (a key combination can only trigger one action). */
export function findShortcutConflict(
  id: string,
  accelerator: string,
  resolved: Record<string, string>,
): AppShortcutDefinition | null {
  return APP_SHORTCUTS.find(other => other.id !== id && acceleratorsClash(resolved[other.id] ?? '', accelerator)) ?? null;
}

/** Translated name of a shortcut. */
export function shortcutLabel(shortcut: AppShortcutDefinition, t: Translator): string {
  return t(shortcut.labelKey, shortcut.labelParams);
}
