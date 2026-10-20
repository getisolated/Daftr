import { describe, it, expect } from 'vitest';
import { SCROLLBAR_HOVER_ZONE_PX, isOverScrollbarZone } from '../src/app/shared/codemirror/hidden-scrollbar';

const box = { right: 500, bottom: 400 };

describe('isOverScrollbarZone', () => {
  it('is true along the right edge (vertical scrollbar)', () => {
    expect(isOverScrollbarZone(500, 100, box)).toBe(true);
    expect(isOverScrollbarZone(500 - SCROLLBAR_HOVER_ZONE_PX, 100, box)).toBe(true);
  });

  it('is true along the bottom edge (horizontal scrollbar)', () => {
    expect(isOverScrollbarZone(100, 395, box)).toBe(true);
  });

  it('is false over the text and outside the editor', () => {
    expect(isOverScrollbarZone(500 - SCROLLBAR_HOVER_ZONE_PX - 1, 100, box)).toBe(false);
    expect(isOverScrollbarZone(501, 100, box)).toBe(false);
    expect(isOverScrollbarZone(100, 100, box)).toBe(false);
  });
});
