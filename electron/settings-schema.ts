export type ShortcutMode = 'overlay' | 'app';

export type TabsStyle = 'pills' | 'attached' | 'attached-reverse' | 'recents' | 'trail' | 'deck' | 'dial';

const TABS_STYLES: readonly TabsStyle[] = ['pills', 'attached', 'attached-reverse', 'recents', 'trail', 'deck', 'dial'];

export type ThemeId = 'dark' | 'light' | 'solarized-light' | 'violet-light' | 'github-light' | 'github-dark-dimmed';
export type ThemeKind = 'dark' | 'light';
export type ThemePreference = ThemeId | 'system';

/** Light/dark family of every theme (mirrors theme.model.ts on the renderer side). */
export const THEME_KINDS: Readonly<Record<ThemeId, ThemeKind>> = {
  'dark': 'dark',
  'light': 'light',
  'solarized-light': 'light',
  'violet-light': 'light',
  'github-light': 'light',
  'github-dark-dimmed': 'dark',
};

function isThemeId(value: unknown): value is ThemeId {
  return typeof value === 'string' && Object.keys(THEME_KINDS).includes(value);
}

/** A theme id of the expected family, the fallback otherwise (a light theme cannot be the 'system' dark one). */
function sanitizeThemePreference(value: unknown, fallback: ThemePreference): ThemePreference {
  return isThemeId(value) || value === 'system' ? value : fallback;
}

function sanitizeThemeOfKind(value: unknown, kind: ThemeKind, fallback: ThemeId): ThemeId {
  return isThemeId(value) && THEME_KINDS[value] === kind ? value : fallback;
}

export type FormattingBarMode = 'full' | 'compact';
export type FormattingBarStyle = 'pinned' | 'floating';
export type FormattingBarPosition = 'top' | 'bottom' | 'left' | 'right';

const FORMATTING_BAR_MODES: readonly FormattingBarMode[] = ['full', 'compact'];
const FORMATTING_BAR_STYLES: readonly FormattingBarStyle[] = ['pinned', 'floating'];
const FORMATTING_BAR_POSITIONS: readonly FormattingBarPosition[] = ['top', 'bottom', 'left', 'right'];

/** Folders skipped when scanning a collection (mirrors DEFAULT_COLLECTION_EXCLUDES of collections-scan.ts). */
const DEFAULT_COLLECTIONS_EXCLUDE = [
  'node_modules', 'bin', 'obj', 'build', 'dist', 'out', 'target', 'coverage', 'vendor',
  '__pycache__', 'venv', '.venv', '.git', '.angular', '.next', '.nuxt', '.vs', '.vscode', '.idea',
];
const MAX_EXCLUDES = 100;
const MAX_EXCLUDE_LENGTH = 100;

function sanitizeExcludes(value: unknown, fallback: string[]): string[] {
  if (!Array.isArray(value)) {
    return fallback;
  }
  const names = value.filter((name): name is string => typeof name === 'string').map(name => name.trim())
    .filter(name => name.length > 0 && name.length <= MAX_EXCLUDE_LENGTH && !/[\\/]/.test(name));
  return [...new Set(names)].slice(0, MAX_EXCLUDES);
}

export interface OverlayPosition {
  x: number;
  y: number;
}

export interface AppSettings {
  globalShortcut: string;
  shortcutMode: ShortcutMode;
  overlayWidth: number;
  overlayHeight: number;
  /** true: Enter sends (Shift+Enter = new line). false: Enter = new line (Ctrl+Enter sends). */
  enterToSend: boolean;
  launchAtStartup: boolean;
  /** 'system' follows the OS color scheme. */
  theme: ThemePreference;
  /** Theme used by 'system' while the OS is in dark mode. */
  preferredDarkTheme: ThemeId;
  /** Theme used by 'system' while the OS is in light mode. */
  preferredLightTheme: ThemeId;
  /** true: updates download in the background and install when the app quits. false: the user is asked first. */
  autoUpdate: boolean;
  /** In-app shortcuts changed by the user, by shortcut id; absent = default (the renderer owns the id list). */
  appShortcuts: Record<string, string>;
  formattingBarEnabled: boolean;
  formattingBarMode: FormattingBarMode;
  formattingBarStyle: FormattingBarStyle;
  formattingBarPosition: FormattingBarPosition;
  showLineNumbers: boolean;
  wordWrap: boolean;
  /** true: the note always ends with an empty line, added automatically. */
  trailingLine: boolean;
  /** true: the h1/h2/h3 outline is shown on the right of the open note. */
  showOutline: boolean;
  /** true: the editor scrollbar only shows while the mouse is over it. */
  hideScrollbar: boolean;
  /** true: a note opens in preview (rendered, read-only) instead of its markdown source. */
  openInPreview: boolean;
  /** Folder names never listed in the collections (hidden folders are always skipped). */
  collectionsExclude: string[];
  tabsStyle: TabsStyle;
  /** Last position chosen by the user; null until the overlay has been moved once. */
  overlayPosition: OverlayPosition | null;
}

/** Settings the renderer is allowed to change (the position is owned by the main process). */
export type SettingsPatch = Partial<Omit<AppSettings, 'overlayPosition'>>;

export const OVERLAY_MIN_WIDTH = 200;
export const OVERLAY_MAX_WIDTH = 800;
export const OVERLAY_MIN_HEIGHT = 200;
export const OVERLAY_MAX_HEIGHT = 1200;

export const DEFAULT_SETTINGS: AppSettings = {
  globalShortcut: 'CommandOrControl+Alt+N',
  shortcutMode: 'overlay',
  overlayWidth: 250,
  overlayHeight: 350,
  enterToSend: true,
  launchAtStartup: false,
  theme: 'dark',
  preferredDarkTheme: 'dark',
  preferredLightTheme: 'light',
  autoUpdate: false,
  appShortcuts: {},
  formattingBarEnabled: true,
  formattingBarMode: 'full',
  formattingBarStyle: 'floating',
  formattingBarPosition: 'top',
  showLineNumbers: false,
  wordWrap: true,
  trailingLine: false,
  showOutline: false,
  hideScrollbar: false,
  openInPreview: false,
  collectionsExclude: DEFAULT_COLLECTIONS_EXCLUDE,
  tabsStyle: 'pills',
  overlayPosition: null,
};

function clampInt(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, Math.round(value)));
}

const SHORTCUT_ID_PATTERN = /^[a-z0-9.-]{1,64}$/;
const MAX_ACCELERATOR_LENGTH = 64;
const MAX_APP_SHORTCUTS = 100;

function sanitizeAppShortcuts(value: unknown, fallback: Record<string, string>): Record<string, string> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return fallback;
  }
  const shortcuts: Record<string, string> = {};
  for (const [id, accelerator] of Object.entries(value).slice(0, MAX_APP_SHORTCUTS)) {
    if (SHORTCUT_ID_PATTERN.test(id) && typeof accelerator === 'string' && accelerator.trim() && accelerator.length <= MAX_ACCELERATOR_LENGTH) {
      shortcuts[id] = accelerator.trim();
    }
  }
  return shortcuts;
}

function sanitizePosition(value: unknown): OverlayPosition | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }
  const { x, y } = value as Record<string, unknown>;
  if (typeof x !== 'number' || typeof y !== 'number' || !Number.isFinite(x) || !Number.isFinite(y)) {
    return null;
  }
  return { x: Math.round(x), y: Math.round(y) };
}

/** Builds a complete, valid settings object from untrusted input (disk or renderer), falling back to current values. */
export function sanitizeSettings(input: unknown, base: AppSettings = DEFAULT_SETTINGS): AppSettings {
  const raw = (typeof input === 'object' && input !== null ? input : {}) as Record<string, unknown>;
  const shortcut = typeof raw['globalShortcut'] === 'string' ? raw['globalShortcut'].trim() : '';
  const mode = raw['shortcutMode'] === 'overlay' || raw['shortcutMode'] === 'app' ? raw['shortcutMode'] : base.shortcutMode;

  return {
    globalShortcut: shortcut || base.globalShortcut,
    shortcutMode: mode,
    overlayWidth: clampInt(raw['overlayWidth'], OVERLAY_MIN_WIDTH, OVERLAY_MAX_WIDTH, base.overlayWidth),
    overlayHeight: clampInt(raw['overlayHeight'], OVERLAY_MIN_HEIGHT, OVERLAY_MAX_HEIGHT, base.overlayHeight),
    enterToSend: typeof raw['enterToSend'] === 'boolean' ? raw['enterToSend'] : base.enterToSend,
    theme: sanitizeThemePreference(raw['theme'], base.theme),
    preferredDarkTheme: sanitizeThemeOfKind(raw['preferredDarkTheme'], 'dark', base.preferredDarkTheme),
    preferredLightTheme: sanitizeThemeOfKind(raw['preferredLightTheme'], 'light', base.preferredLightTheme),
    launchAtStartup: typeof raw['launchAtStartup'] === 'boolean' ? raw['launchAtStartup'] : base.launchAtStartup,
    autoUpdate: typeof raw['autoUpdate'] === 'boolean' ? raw['autoUpdate'] : base.autoUpdate,
    appShortcuts: sanitizeAppShortcuts(raw['appShortcuts'], base.appShortcuts),
    formattingBarEnabled: typeof raw['formattingBarEnabled'] === 'boolean' ? raw['formattingBarEnabled'] : base.formattingBarEnabled,
    formattingBarMode: FORMATTING_BAR_MODES.find(mode => mode === raw['formattingBarMode']) ?? base.formattingBarMode,
    formattingBarStyle: FORMATTING_BAR_STYLES.find(style => style === raw['formattingBarStyle']) ?? base.formattingBarStyle,
    formattingBarPosition: FORMATTING_BAR_POSITIONS.find(position => position === raw['formattingBarPosition']) ?? base.formattingBarPosition,
    showLineNumbers: typeof raw['showLineNumbers'] === 'boolean' ? raw['showLineNumbers'] : base.showLineNumbers,
    wordWrap: typeof raw['wordWrap'] === 'boolean' ? raw['wordWrap'] : base.wordWrap,
    trailingLine: typeof raw['trailingLine'] === 'boolean' ? raw['trailingLine'] : base.trailingLine,
    showOutline: typeof raw['showOutline'] === 'boolean' ? raw['showOutline'] : base.showOutline,
    hideScrollbar: typeof raw['hideScrollbar'] === 'boolean' ? raw['hideScrollbar'] : base.hideScrollbar,
    openInPreview: typeof raw['openInPreview'] === 'boolean' ? raw['openInPreview'] : base.openInPreview,
    collectionsExclude: sanitizeExcludes(raw['collectionsExclude'], base.collectionsExclude),
    tabsStyle: TABS_STYLES.find(style => style === raw['tabsStyle']) ?? base.tabsStyle,
    overlayPosition: 'overlayPosition' in raw ? sanitizePosition(raw['overlayPosition']) : base.overlayPosition,
  };
}
