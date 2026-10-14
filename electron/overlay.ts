import { BrowserWindow, screen } from 'electron';
import * as path from 'path';
import { anchorBottomRight, resolveOverlayPosition, Size } from './overlay-geometry';
import { loadRenderer } from './renderer-loader';
import { AppSettings } from './settings-schema';
import { getSettings, updateSettings } from './settings';

const POSITION_SAVE_DELAY_MS = 300;

let overlayWindow: BrowserWindow | null = null;
let savePositionTimer: ReturnType<typeof setTimeout> | null = null;

function overlaySize(settings: AppSettings): Size {
  return { width: settings.overlayWidth, height: settings.overlayHeight };
}

function currentPosition(win: BrowserWindow): { x: number; y: number } {
  const [x, y] = win.getPosition();
  return { x, y };
}

function placeOverlay(win: BrowserWindow): void {
  const settings = getSettings();
  const size = overlaySize(settings);
  const position = resolveOverlayPosition(
    settings.overlayPosition,
    size,
    screen.getAllDisplays().map(display => display.workArea),
    screen.getPrimaryDisplay().workArea,
  );
  win.setBounds({ ...position, ...size });
}

/** Creates the hidden overlay window once, so that showing it later is instantaneous. */
export function createOverlayWindow(): BrowserWindow {
  const settings = getSettings();
  const win = new BrowserWindow({
    ...overlaySize(settings),
    show: false,
    frame: false,
    transparent: true,
    backgroundColor: '#00000000',
    hasShadow: false,
    resizable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  // 'screen-saver' level keeps the overlay above fullscreen apps too
  win.setAlwaysOnTop(true, 'screen-saver');
  loadRenderer(win, 'overlay');

  // Clicking anywhere else on the screen blurs the window: hide it, the renderer keeps the draft alive
  win.on('blur', () => win.hide());

  // Debounced: 'move' fires continuously while dragging
  win.on('move', () => {
    if (savePositionTimer) {
      clearTimeout(savePositionTimer);
    }
    savePositionTimer = setTimeout(() => {
      if (!win.isDestroyed() && win.isVisible()) {
        updateSettings({ overlayPosition: currentPosition(win) });
      }
    }, POSITION_SAVE_DELAY_MS);
  });

  win.on('closed', () => { overlayWindow = null; });
  overlayWindow = win;
  return win;
}

export function showOverlay(): void {
  const win = overlayWindow ?? createOverlayWindow();
  placeOverlay(win);
  win.show();
  win.focus();
  win.webContents.send('overlay:shown');
}

export function hideOverlay(): void {
  overlayWindow?.hide();
}

export function toggleOverlay(): void {
  if (overlayWindow?.isVisible()) {
    hideOverlay();
  } else {
    showOverlay();
  }
}

/** Applies a new overlay size, keeping the bottom-right corner in place and remembering the result. */
export function applyOverlaySize(previous: AppSettings, next: AppSettings): void {
  if (!overlayWindow || (previous.overlayWidth === next.overlayWidth && previous.overlayHeight === next.overlayHeight)) {
    return;
  }
  const newSize = overlaySize(next);
  const anchored = anchorBottomRight(currentPosition(overlayWindow), overlaySize(previous), newSize);
  overlayWindow.setBounds({ ...anchored, ...newSize });
  updateSettings({ overlayPosition: anchored });
}

export function broadcastToWindows(channel: string, payload?: unknown, except?: Electron.WebContents): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (win.webContents !== except) {
      win.webContents.send(channel, payload);
    }
  }
}
