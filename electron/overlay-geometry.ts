import { OverlayPosition } from './settings-schema';

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Size {
  width: number;
  height: number;
}

export const OVERLAY_MARGIN = 16;
/** Minimum part of the overlay (px, per axis) that must stay on a screen to consider a saved position valid. */
const MIN_VISIBLE = 80;

function intersectionLength(startA: number, lengthA: number, startB: number, lengthB: number): number {
  const start = Math.max(startA, startB);
  const end = Math.min(startA + lengthA, startB + lengthB);
  return Math.max(0, end - start);
}

/** Default spot: bottom-right corner of the given work area. */
export function bottomRightPosition(workArea: Rect, size: Size): OverlayPosition {
  return {
    x: workArea.x + workArea.width - size.width - OVERLAY_MARGIN,
    y: workArea.y + workArea.height - size.height - OVERLAY_MARGIN,
  };
}

/**
 * Picks where to show the overlay. The saved position is reused only if enough of the window is still
 * visible on a connected screen (a monitor may have been unplugged since); otherwise falls back to the
 * bottom-right of the primary work area.
 */
export function resolveOverlayPosition(
  saved: OverlayPosition | null,
  size: Size,
  workAreas: Rect[],
  primaryWorkArea: Rect,
): OverlayPosition {
  if (saved) {
    const visible = workAreas.some(area =>
      intersectionLength(saved.x, size.width, area.x, area.width) >= Math.min(MIN_VISIBLE, size.width) &&
      intersectionLength(saved.y, size.height, area.y, area.height) >= Math.min(MIN_VISIBLE, size.height));
    if (visible) {
      return saved;
    }
  }
  return bottomRightPosition(primaryWorkArea, size);
}

/** New top-left after a resize, keeping the bottom-right corner fixed (the chat input lives at the bottom). */
export function anchorBottomRight(position: OverlayPosition, oldSize: Size, newSize: Size): OverlayPosition {
  return {
    x: position.x + oldSize.width - newSize.width,
    y: position.y + oldSize.height - newSize.height,
  };
}
