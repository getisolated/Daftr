import { describe, it, expect } from 'vitest';
import { anchorBottomRight, bottomRightPosition, resolveOverlayPosition } from '../../electron/overlay-geometry';

const screen1 = { x: 0, y: 0, width: 1920, height: 1040 };
const screen2 = { x: 1920, y: 0, width: 1280, height: 1024 };
const size = { width: 250, height: 500 };

describe('bottomRightPosition', () => {
  it('places the overlay in the bottom-right corner with a margin', () => {
    expect(bottomRightPosition(screen1, size)).toEqual({ x: 1920 - 250 - 16, y: 1040 - 500 - 16 });
  });

  it('respects the work area origin (secondary monitor)', () => {
    expect(bottomRightPosition(screen2, size)).toEqual({ x: 1920 + 1280 - 250 - 16, y: 1024 - 500 - 16 });
  });
});

describe('resolveOverlayPosition', () => {
  it('defaults to bottom-right when nothing was saved', () => {
    expect(resolveOverlayPosition(null, size, [screen1], screen1)).toEqual(bottomRightPosition(screen1, size));
  });

  it('reuses the saved position when it is visible', () => {
    expect(resolveOverlayPosition({ x: 300, y: 200 }, size, [screen1], screen1)).toEqual({ x: 300, y: 200 });
  });

  it('reuses a saved position located on a secondary monitor', () => {
    expect(resolveOverlayPosition({ x: 2000, y: 100 }, size, [screen1, screen2], screen1)).toEqual({ x: 2000, y: 100 });
  });

  it('falls back to default when the saved monitor is gone', () => {
    expect(resolveOverlayPosition({ x: 2000, y: 100 }, size, [screen1], screen1)).toEqual(bottomRightPosition(screen1, size));
  });

  it('falls back to default when the window is almost entirely off-screen', () => {
    expect(resolveOverlayPosition({ x: 1900, y: 100 }, size, [screen1], screen1)).toEqual(bottomRightPosition(screen1, size));
  });
});

describe('anchorBottomRight', () => {
  it('keeps the bottom-right corner fixed when growing', () => {
    expect(anchorBottomRight({ x: 100, y: 100 }, { width: 250, height: 500 }, { width: 300, height: 600 }))
      .toEqual({ x: 50, y: 0 });
  });

  it('keeps the bottom-right corner fixed when shrinking', () => {
    expect(anchorBottomRight({ x: 100, y: 100 }, { width: 250, height: 500 }, { width: 200, height: 400 }))
      .toEqual({ x: 150, y: 200 });
  });
});
