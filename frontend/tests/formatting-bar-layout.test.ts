import { describe, it, expect } from 'vitest';
import { FORMATTING_BAR_GROUPS } from '../src/app/shared/codemirror/formatting-commands';
import { shouldCondense } from '../src/app/core/utils/formatting-bar.util';

describe('shouldCondense', () => {
  it('condenses only when the full bar does not fit', () => {
    expect(shouldCondense(845, 844)).toBe(true);
    expect(shouldCondense(845, 845)).toBe(false);
    expect(shouldCondense(845, 1200)).toBe(false);
  });

  it('keeps the full bar until it has been measured', () => {
    expect(shouldCondense(null, 0)).toBe(false);
  });
});

describe('FORMATTING_BAR_GROUPS', () => {
  it('lists every item once, the heading drop-down in its own group', () => {
    const items = FORMATTING_BAR_GROUPS.flatMap(group => group.items);
    expect(new Set(items).size).toBe(items.length);
    expect(FORMATTING_BAR_GROUPS.find(group => group.id === 'heading')?.items).toEqual(['heading']);
  });
});
