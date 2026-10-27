import { describe, it, expect } from 'vitest';
import { ZEN_REVEAL_EDGE_PX, ZenPointerState, zenSidebarPointerAction } from '../src/app/core/utils/zen-sidebar.util';

const at = (state: Partial<ZenPointerState>): ZenPointerState => ({ x: 400, overPanel: false, revealed: null, editing: false, ...state });

describe('zenSidebarPointerAction', () => {
  it('opens when the pointer reaches the left edge of the window', () => {
    expect(zenSidebarPointerAction(at({ x: 0 }))).toBe('open');
    expect(zenSidebarPointerAction(at({ x: ZEN_REVEAL_EDGE_PX }))).toBe('open');
  });

  it('keeps it open while the pointer is over it, wherever that is', () => {
    expect(zenSidebarPointerAction(at({ x: 200, overPanel: true, revealed: 'pointer' }))).toBe('open');
  });

  it('closes when the pointer leaves the sidebar it opened', () => {
    expect(zenSidebarPointerAction(at({ x: ZEN_REVEAL_EDGE_PX + 1, revealed: 'pointer' }))).toBe('close');
  });

  it('does nothing away from the edge while tucked away', () => {
    expect(zenSidebarPointerAction(at({ x: 300 }))).toBe('stay');
  });

  it('stays open when opened from the keyboard, or while a name is being typed in it', () => {
    expect(zenSidebarPointerAction(at({ revealed: 'keyboard' }))).toBe('stay');
    expect(zenSidebarPointerAction(at({ revealed: 'pointer', editing: true }))).toBe('stay');
  });
});
