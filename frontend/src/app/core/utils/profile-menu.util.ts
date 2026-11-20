/** Gap kept between the menu and the edges of the window. */
const VIEWPORT_MARGIN = 8;

export interface MenuAnchor {
  right: number;
  bottom: number;
}

export interface MenuPosition {
  left: number;
  /** Distance from the bottom of the window: the menu grows upwards from there. */
  bottom: number;
}

/**
 * Position of the profile menu: on the right of its item, bottom edges aligned (the item sits at the bottom of the
 * sidebar), kept inside the window. Overlapping the item by `overlap` pixels leaves no gap for the pointer to fall
 * into on its way to the menu.
 */
export function profileMenuPosition(
  anchor: MenuAnchor,
  viewport: { width: number; height: number },
  menuWidth: number,
  overlap = 4,
): MenuPosition {
  const maxLeft = viewport.width - menuWidth - VIEWPORT_MARGIN;
  const left = Math.min(anchor.right - overlap, maxLeft);
  const bottom = viewport.height - anchor.bottom;

  return { left: Math.max(VIEWPORT_MARGIN, left), bottom: Math.max(VIEWPORT_MARGIN, bottom) };
}

/**
 * Opens a menu on hover and keeps it open while the pointer travels from its trigger to it: leaving one of them only
 * closes after `closeDelayMs`, cancelled by entering the other.
 */
export class HoverIntent {
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly setOpen: (open: boolean) => void,
    private readonly openDelayMs = 80,
    private readonly closeDelayMs = 250,
  ) {}

  /** The pointer entered the trigger or the menu. */
  enter(): void {
    this.schedule(true, this.openDelayMs);
  }

  /** The pointer left the trigger or the menu. */
  leave(): void {
    this.schedule(false, this.closeDelayMs);
  }

  /** Forgets what was scheduled (the menu was opened or closed another way). */
  cancel(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private schedule(open: boolean, delay: number): void {
    this.cancel();
    this.timer = setTimeout(() => {
      this.timer = null;
      this.setOpen(open);
    }, delay);
  }
}
