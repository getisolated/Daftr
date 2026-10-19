import { describe, it, expect } from 'vitest';
import { FORMATTING_BAR_GROUPS } from '../src/app/shared/codemirror/formatting-commands';
import { isFormattingBarCondensed } from '../src/app/core/utils/formatting-bar.util';

describe('isFormattingBarCondensed', () => {
  const size = (width: number, height: number) => ({ width, height });

  it('condenses a vertical bar below 870px of height, whatever the width and the sidebar', () => {
    expect(isFormattingBarCondensed('left', size(1280, 664), 0)).toBe(true);
    expect(isFormattingBarCondensed('right', size(1280, 869), 400)).toBe(true);
    expect(isFormattingBarCondensed('right', size(700, 870), 0)).toBe(false);
  });

  it('condenses a horizontal bar below 1200px of width minus the open sidebar width', () => {
    expect(isFormattingBarCondensed('top', size(1199, 2000), 0)).toBe(true);
    expect(isFormattingBarCondensed('bottom', size(1200, 500), 0)).toBe(false);
    expect(isFormattingBarCondensed('top', size(950, 800), 250)).toBe(false);
    expect(isFormattingBarCondensed('top', size(949, 800), 250)).toBe(true);
  });
});

describe('FORMATTING_BAR_GROUPS', () => {
  it('lists every item once, the heading drop-down in its own group', () => {
    const items = FORMATTING_BAR_GROUPS.flatMap(group => group.items);
    expect(new Set(items).size).toBe(items.length);
    expect(FORMATTING_BAR_GROUPS.find(group => group.id === 'heading')?.items).toEqual(['heading']);
  });
});
