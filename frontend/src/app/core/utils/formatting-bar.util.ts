import { FormattingBarPosition } from '../models/formatting-bar.model';

/** A vertical bar (left / right) is condensed below this window width. */
export const VERTICAL_CONDENSED_BELOW_PX = 870;

/** A horizontal bar (top / bottom) is condensed below this window width, minus the width of the open sidebar. */
export const HORIZONTAL_CONDENSED_BELOW_PX = 1200;

/**
 * Whether the bar shows one drop-down per group instead of every button, to save room.
 * `sidebarWidth` is the width of the sidebar, 0 when it is closed.
 */
export function isFormattingBarCondensed(position: FormattingBarPosition, windowWidth: number, sidebarWidth: number): boolean {
  if (position === 'left' || position === 'right') {
    return windowWidth < VERTICAL_CONDENSED_BELOW_PX;
  }
  return windowWidth < HORIZONTAL_CONDENSED_BELOW_PX - sidebarWidth;
}
