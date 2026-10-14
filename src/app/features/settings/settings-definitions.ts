import { AppSettings } from '../../core/models/settings.model';

export type SettingControl = 'checkbox' | 'number' | 'select' | 'shortcut';

export interface SettingDefinition {
  key: keyof AppSettings;
  category: string;
  title: string;
  description: string;
  control: SettingControl;
  min?: number;
  max?: number;
  step?: number;
  options?: { value: string; label: string }[];
}

export interface SettingsSection {
  category: string;
  settings: SettingDefinition[];
}

/** Single source of truth for the settings editor: adding a setting = adding an entry here. */
export const SETTING_DEFINITIONS: SettingDefinition[] = [
  {
    key: 'launchAtStartup',
    category: 'General',
    title: 'Launch At Startup',
    description: 'Start Everything Note silently in the system tray when you log in to Windows.',
    control: 'checkbox',
  },
  {
    key: 'globalShortcut',
    category: 'Global Shortcut',
    title: 'Shortcut',
    description: 'System-wide shortcut that works even when the app is in the tray. Click, then press the new key combination.',
    control: 'shortcut',
  },
  {
    key: 'shortcutMode',
    category: 'Global Shortcut',
    title: 'Action',
    description: 'What the global shortcut opens.',
    control: 'select',
    options: [
      { value: 'overlay', label: 'Quick-note overlay' },
      { value: 'app', label: 'Full app' },
    ],
  },
  {
    key: 'overlayWidth',
    category: 'Quick Note Overlay',
    title: 'Width',
    description: 'Width of the overlay, in pixels.',
    control: 'number',
    min: 200,
    max: 800,
    step: 10,
  },
  {
    key: 'overlayHeight',
    category: 'Quick Note Overlay',
    title: 'Height',
    description: 'Height of the overlay, in pixels.',
    control: 'number',
    min: 200,
    max: 1200,
    step: 10,
  },
  {
    key: 'enterToSend',
    category: 'Quick Note Overlay',
    title: 'Enter To Send',
    description: 'Enter sends the note (Shift+Enter inserts a new line). When unchecked, Enter inserts a new line and Ctrl+Enter sends.',
    control: 'checkbox',
  },
];

/** Case-insensitive match on category, title, description or key; every word of the query must match. */
export function filterSettings(definitions: SettingDefinition[], query: string): SettingDefinition[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return definitions;
  return definitions.filter(def => {
    const haystack = `${def.category} ${def.title} ${def.description} ${def.key}`.toLowerCase();
    return words.every(word => haystack.includes(word));
  });
}

/** Groups definitions by category, keeping the declaration order. */
export function groupByCategory(definitions: SettingDefinition[]): SettingsSection[] {
  const sections = new Map<string, SettingDefinition[]>();
  for (const def of definitions) {
    const list = sections.get(def.category) ?? [];
    list.push(def);
    sections.set(def.category, list);
  }
  return [...sections].map(([category, settings]) => ({ category, settings }));
}
