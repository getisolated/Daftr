import { BrowserWindow, desktopCapturer, screen } from 'electron/main';

/** Screenshot of the display the overlay sits on, taken while the overlay is hidden. */
export interface OverlayBackdrop {
  dataUrl: string;
  /** Display origin and size, in the same (DIP) coordinates as the window position. */
  x: number;
  y: number;
  width: number;
  height: number;
}

// It only feeds a heavy blur: a half-resolution JPEG is plenty and keeps the IPC payload small
const CAPTURE_SCALE = 0.5;
const JPEG_QUALITY = 70;

/**
 * Electron cannot blur what is behind a window (CSS backdrop-filter only sees the window's own content, and
 * the OS acrylic is not reliable), so the renderer blurs a capture of the desktop instead.
 * Returns null when the capture is unavailable: the renderer then falls back to a plain translucent panel.
 */
export async function captureBackdrop(win: BrowserWindow): Promise<OverlayBackdrop | null> {
  try {
    const display = screen.getDisplayMatching(win.getBounds());
    const sources = await desktopCapturer.getSources({
      types: ['screen'],
      thumbnailSize: {
        width: Math.round(display.bounds.width * CAPTURE_SCALE),
        height: Math.round(display.bounds.height * CAPTURE_SCALE),
      },
    });
    const source = sources.find(candidate => candidate.display_id === String(display.id)) ?? sources[0];
    if (!source || source.thumbnail.isEmpty()) {
      return null;
    }
    const dataUrl = `data:image/jpeg;base64,${source.thumbnail.toJPEG(JPEG_QUALITY).toString('base64')}`;
    return { dataUrl, ...display.bounds };
  } catch {
    return null;
  }
}
