import { describe, it, expect } from 'vitest';
import { FORMATTING_BAR_GROUPS } from '../src/app/shared/codemirror/formatting-commands';
import { isFormattingBarCondensed } from '../src/app/core/utils/formatting-bar.util';

describe('isFormattingBarCondensed', () => {
  it('condenses a vertical bar below 870px, whatever the sidebar', () => {
    expect(isFormattingBarCondensed('left', 869, 0)).toBe(true);
    expect(isFormattingBarCondensed('right', 870, 0)).toBe(false);
    expect(isFormattingBarCondensed('left', 900, 300)).toBe(false);
  });

  it('condenses a horizontal bar below 1200px minus the open sidebar width', () => {
    expect(isFormattingBarCondensed('top', 1199, 0)).toBe(true);
    expect(isFormattingBarCondensed('bottom', 1200, 0)).toBe(false);
    expect(isFormattingBarCondensed('top', 950, 250)).toBe(false);
    expect(isFormattingBarCondensed('top', 949, 250)).toBe(true);
  });
});

describe('FORMATTING_BAR_GROUPS', () => {
  it('lists every item once, the heading drop-down in its own group', () => {
    const items = FORMATTING_BAR_GROUPS.flatMap(group => group.items);
    expect(new Set(items).size).toBe(items.length);
    expect(FORMATTING_BAR_GROUPS.find(group => group.id === 'heading')?.items).toEqual(['heading']);
  });
});
