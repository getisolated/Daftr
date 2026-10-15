import { BrowserWindow, screen } from 'electron/main';
import * as path from 'path';
import { isAcrylicActive, refreshAcrylicState } from './acrylic-support';
import { captureBackdrop } from './overlay-backdrop';
import { anchorBottomRight, resolveOverlayPosition, Size } from './overlay-geometry';
import { loadRenderer } from './renderer-loader';
import { AppSettings } from './settings-schema';
import { getSettings, updateSettings } from './settings';

const POSITION_SAVE_DELAY_MS = 300;

let overlayWindow: BrowserWindow | null = null;
// Material the current window was created with: it cannot change afterwards (acrylic needs a non-transparent window)
let overlayAcrylic = false;
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

function createOverlayWindow(acrylic: boolean): BrowserWindow {
  const settings = getSettings();
  const win = new BrowserWindow({
    ...overlaySize(settings),
    show: false,
    frame: false,
    // Acrylic is drawn by the OS behind a non-transparent window; otherwise the renderer blurs a desktop capture
    transparent: !acrylic,
    backgroundColor: '#00000000',
    ...(acrylic ? { backgroundMaterial: 'acrylic' as const } : {}),
    hasShadow: acrylic,
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
  if (acrylic) {
    // The renderer then paints no background of its own and lets the acrylic show through
    win.webContents.on('dom-ready', () => {
      void win.webContents.executeJavaScript("document.documentElement.classList.add('overlay-acrylic')");
    });
  }

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
  overlayAcrylic = acrylic;
  return win;
}

/** Creates the hidden overlay window once at startup, so that showing it later is instantaneous. */
export async function prepareOverlay(): Promise<void> {
  createOverlayWindow(await isAcrylicActive());
}

/** Returns the overlay window, recreated if Battery Saver / transparency settings changed the usable material. */
async function ensureOverlayWindow(): Promise<BrowserWindow> {
  const acrylic = await isAcrylicActive();
  if (overlayWindow && !overlayWindow.isDestroyed() && overlayAcrylic !== acrylic) {
    overlayWindow.destroy();
    overlayWindow = null;
  }
  return overlayWindow ?? createOverlayWindow(acrylic);
}

export async function showOverlay(): Promise<void> {
  const win = await ensureOverlayWindow();
  placeOverlay(win);
  // Captured before show(), otherwise the overlay itself would be in the screenshot; acrylic needs none
  const backdrop = overlayAcrylic ? null : await captureBackdrop(win);
  if (win.isDestroyed()) {
    return;
  }
  win.webContents.send('overlay:backdrop', backdrop);
  win.show();
  win.focus();
  win.webContents.send('overlay:shown');
  // A Battery Saver change is taken into account from the next show
  refreshAcrylicState();
}

export function hideOverlay(): void {
  overlayWindow?.hide();
}

export function toggleOverlay(): void {
  if (overlayWindow?.isVisible()) {
    hideOverlay();
  } else {
    void showOverlay();
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
