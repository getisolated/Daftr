/** Screenshot of the display behind the overlay, blurred by the overlay to fake a frosted-glass window. */
export interface OverlayBackdrop {
  dataUrl: string;
  /** Display origin and size, in the same coordinates as `window.screenX/screenY`. */
  x: number;
  y: number;
  width: number;
  height: number;
}
