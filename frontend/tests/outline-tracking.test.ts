import { describe, it, expect } from 'vitest';
import { findActiveHeading, headingBlockIndexes, revealScrollTop } from '../src/app/core/utils/outline-tracking.util';

describe('findActiveHeading', () => {
  const tops = [0, 300, 700];

  it('returns -1 without headings', () => {
    expect(findActiveHeading([], 0, false)).toBe(-1);
  });

  it('picks the last heading above the viewport top', () => {
    expect(findActiveHeading(tops, 0, false)).toBe(0);
    expect(findActiveHeading(tops, 259, false)).toBe(0);
    expect(findActiveHeading(tops, 260, false)).toBe(1);
    expect(findActiveHeading(tops, 690, false)).toBe(2);
  });

  it('keeps the first heading while above it', () => {
    expect(findActiveHeading([200, 500], 0, false)).toBe(0);
  });

  it('picks the last heading at the bottom of the scroll', () => {
    expect(findActiveHeading(tops, 100, true)).toBe(2);
  });

  it('takes a custom offset', () => {
    expect(findActiveHeading(tops, 290, false, 0)).toBe(0);
    expect(findActiveHeading(tops, 300, false, 0)).toBe(1);
  });
});

describe('headingBlockIndexes', () => {
  it('finds the block starting on each heading line', () => {
    expect(headingBlockIndexes([1, 3, 5, 9, 12], [1, 5, 12])).toEqual([0, 2, 4]);
  });

  it('falls back on the block holding a nested heading', () => {
    // A heading inside a quote (line 6) belongs to the block starting line 5
    expect(headingBlockIndexes([1, 5, 9], [6, 10])).toEqual([1, 2]);
  });

  it('gives the same block to headings of one block', () => {
    expect(headingBlockIndexes([1, 5], [6, 7])).toEqual([1, 1]);
  });

  it('returns -1 for a heading above the first block, and nothing without headings', () => {
    expect(headingBlockIndexes([3, 5], [1, 4])).toEqual([-1, 0]);
    expect(headingBlockIndexes([], [2])).toEqual([-1]);
    expect(headingBlockIndexes([1, 2], [])).toEqual([]);
  });
});

describe('revealScrollTop', () => {
  // Scrolled by 100, the visible span is 100 → 400
  const view = { top: 100, bottom: 400 };

  it('keeps the scroll of an item already in view', () => {
    expect(revealScrollTop(100, view, { top: 100, bottom: 130 })).toBe(100);
    expect(revealScrollTop(100, view, { top: 370, bottom: 400 })).toBe(100);
  });

  it('scrolls up just enough to show an item above', () => {
    expect(revealScrollTop(100, view, { top: 60, bottom: 90 })).toBe(60);
  });

  it('scrolls down just enough to show an item below', () => {
    expect(revealScrollTop(100, view, { top: 420, bottom: 450 })).toBe(150);
  });

  it('shows the start of an item taller than the view', () => {
    expect(revealScrollTop(100, view, { top: 300, bottom: 800 })).toBe(300);
    expect(revealScrollTop(100, view, { top: 50, bottom: 800 })).toBe(50);
  });

  it('never goes above the top of the content', () => {
    expect(revealScrollTop(10, { top: 50, bottom: 300 }, { top: 0, bottom: 20 })).toBe(0);
  });
});
