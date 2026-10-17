import { describe, expect, it } from 'vitest';
import { dialSlots, documentHue, monogram } from '../src/app/core/utils/docs-navigator.util';

describe('monogram', () => {
  it('takes the initials of the first two words', () => {
    expect(monogram('road trip notes')).toBe('RT');
  });

  it('takes the first two letters of a single word', () => {
    expect(monogram('  soleil ')).toBe('SO');
    expect(monogram('x')).toBe('X');
  });

  it('falls back to a dot for an empty title', () => {
    expect(monogram('   ')).toBe('·');
  });
});

describe('documentHue', () => {
  it('is stable and within the color wheel', () => {
    expect(documentHue(42)).toBe(documentHue(42));
    for (const id of [-7, 0, 1, 99, 123456]) {
      expect(documentHue(id)).toBeGreaterThanOrEqual(0);
      expect(documentHue(id)).toBeLessThan(360);
    }
  });

  it('sets consecutive documents apart', () => {
    expect(Math.abs(documentHue(1) - documentHue(2))).toBeGreaterThan(60);
  });
});

describe('dialSlots', () => {
  it('returns no slot without documents', () => {
    expect(dialSlots(0)).toEqual([]);
  });

  it('puts a single document straight below the center', () => {
    expect(dialSlots(1)).toEqual([{ x: 0, y: 120 }]);
  });

  it('fills the inner ring from the bottom outward, alternating sides', () => {
    const [first, second, third] = dialSlots(3);
    expect(first).toEqual({ x: 0, y: 120 });
    expect(second.x).toBeGreaterThan(0);
    expect(third.x).toBe(-second.x);
    expect(second.y).toBe(third.y);
  });

  it('keeps every slot below the title bar', () => {
    expect(dialSlots(40).every(slot => slot.y > 0)).toBe(true);
  });

  it('moves to a larger ring once the inner one is full', () => {
    const slots = dialSlots(8);
    const radius = (slot: { x: number; y: number }): number => Math.round(Math.hypot(slot.x, slot.y));
    expect(slots.slice(0, 7).every(slot => Math.abs(radius(slot) - 120) <= 1)).toBe(true);
    expect(radius(slots[7])).toBeGreaterThan(190);
  });
});
