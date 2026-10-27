/** Strip along the left edge of the window that slides the floating sidebar in (zen mode). */
export const ZEN_REVEAL_EDGE_PX = 12;

/** What brought the floating sidebar in: the pointer (it leaves with it) or the shortcut (it stays). `null`: tucked away. */
export type ZenSidebarReveal = 'pointer' | 'keyboard' | null;

export interface ZenPointerState {
  /** Horizontal position of the pointer in the window. */
  x: number;
  /** The pointer is over the sidebar or over something it opened (menu, dialog). */
  overPanel: boolean;
  revealed: ZenSidebarReveal;
  /** A field of the sidebar is being typed in (renaming): it must not slide away under the caret. */
  editing: boolean;
}

/** What a pointer move does to the floating sidebar of the zen mode. */
export function zenSidebarPointerAction(state: ZenPointerState): 'open' | 'stay' | 'close' {
  if (state.overPanel || state.x <= ZEN_REVEAL_EDGE_PX) {
    return 'open';
  }

  // Opened from the keyboard: closed by the shortcut again or a click elsewhere, not by a pointer passing by
  if (state.revealed !== 'pointer' || state.editing) {
    return 'stay';
  }

  return 'close';
}
