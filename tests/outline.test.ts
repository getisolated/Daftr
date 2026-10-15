import { describe, it, expect } from 'vitest';
import { findActiveHeading } from '../src/app/shared/codemirror/outline';

describe('findActiveHeading', () => {
  const tops = [0, 300, 700];

  it('returns -1 without headings', () => {
    expect(findActiveHeading([], 0, false)).toBe(-1);
  });

  it('picks the last heading above the viewport top', () => {
    expect(findActiveHeading(tops, 0, false)).toBe(0);
    expect(findActiveHeading(tops, 280, false)).toBe(1);
    expect(findActiveHeading(tops, 690, false)).toBe(2);
  });

  it('picks the last heading at the bottom of the scroll', () => {
    expect(findActiveHeading(tops, 100, true)).toBe(2);
  });
});
