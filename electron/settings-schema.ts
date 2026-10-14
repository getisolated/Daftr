export type ShortcutMode = 'overlay' | 'app';

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
  overlayHeight: 500,
  enterToSend: true,
  launchAtStartup: false,
  overlayPosition: null,
};

function clampInt(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, Math.round(value)));
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
    launchAtStartup: typeof raw['launchAtStartup'] === 'boolean' ? raw['launchAtStartup'] : base.launchAtStartup,
    overlayPosition: 'overlayPosition' in raw ? sanitizePosition(raw['overlayPosition']) : base.overlayPosition,
  };
}
