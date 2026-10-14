import { describe, it, expect } from 'vitest';
import { DEFAULT_SETTINGS, sanitizeSettings } from '../electron/settings-schema';

describe('sanitizeSettings', () => {
  it('returns defaults for garbage input', () => {
    expect(sanitizeSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(sanitizeSettings('nope')).toEqual(DEFAULT_SETTINGS);
  });

  it('keeps valid values', () => {
    const result = sanitizeSettings({
      globalShortcut: 'Ctrl+Shift+K', shortcutMode: 'app', overlayWidth: 320, overlayHeight: 640, enterToSend: false,
    });
    expect(result).toMatchObject({
      globalShortcut: 'Ctrl+Shift+K', shortcutMode: 'app', overlayWidth: 320, overlayHeight: 640, enterToSend: false,
    });
  });

  it('clamps the overlay size to its bounds', () => {
    expect(sanitizeSettings({ overlayWidth: 10, overlayHeight: 99999 })).toMatchObject({ overlayWidth: 200, overlayHeight: 1200 });
  });

  it('rejects non-finite or non-numeric sizes', () => {
    expect(sanitizeSettings({ overlayWidth: NaN, overlayHeight: '500' })).toMatchObject({ overlayWidth: 250, overlayHeight: 500 });
  });

  it('rejects an unknown shortcut mode and an empty shortcut', () => {
    expect(sanitizeSettings({ shortcutMode: 'weird', globalShortcut: '  ' }))
      .toMatchObject({ shortcutMode: 'overlay', globalShortcut: DEFAULT_SETTINGS.globalShortcut });
  });

  it('falls back to the current values rather than the defaults', () => {
    const base = { ...DEFAULT_SETTINGS, overlayWidth: 400 };
    expect(sanitizeSettings({ overlayWidth: 'x' }, base).overlayWidth).toBe(400);
  });

  it('sanitizes the saved position', () => {
    expect(sanitizeSettings({ overlayPosition: { x: 10.4, y: 20.6 } }).overlayPosition).toEqual({ x: 10, y: 21 });
    expect(sanitizeSettings({ overlayPosition: { x: 'a', y: 2 } }).overlayPosition).toBeNull();
  });
});
