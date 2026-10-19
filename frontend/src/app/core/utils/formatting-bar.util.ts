import { FormattingBarPosition } from '../models/formatting-bar.model';

/** A vertical bar (left / right) is condensed below this window height: its buttons would not fit. */
export const VERTICAL_CONDENSED_BELOW_PX = 870;

/** A horizontal bar (top / bottom) is condensed when the editor (the window without the open sidebar) is narrower. */
export const HORIZONTAL_CONDENSED_BELOW_PX = 1200;

export interface WindowSize {
  width: number;
  height: number;
}

/**
 * Whether the bar shows one drop-down per group instead of every button, to save room along its own axis.
 * `sidebarWidth` is the width of the sidebar, 0 when it is closed.
 */
export function isFormattingBarCondensed(position: FormattingBarPosition, window: WindowSize, sidebarWidth: number): boolean {
  if (position === 'left' || position === 'right') {
    return window.height < VERTICAL_CONDENSED_BELOW_PX;
  }
  return window.width - sidebarWidth < HORIZONTAL_CONDENSED_BELOW_PX;
}
