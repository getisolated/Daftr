import { describe, it, expect } from 'vitest';
import { diffLines, hasDifferences } from '../src/app/core/utils/line-diff.util';

function kinds(original: string, copy: string): string[] {
  return diffLines(original, copy).map(row => row.kind);
}

describe('diffLines', () => {
  it('has no row for two empty texts, and only same rows for identical ones', () => {
    expect(diffLines('', '')).toEqual([]);
    expect(kinds('a\nb\n', 'a\nb\n')).toEqual(['same', 'same']);
    expect(hasDifferences(diffLines('a\nb', 'a\nb'))).toBe(false);
  });

  it('pairs a modified line', () => {
    const rows = diffLines('a\nb\nc', 'a\nB\nc');

    expect(rows.map(row => row.kind)).toEqual(['same', 'changed', 'same']);
    expect(rows[1]).toMatchObject({ left: 'b', right: 'B', leftLine: 2, rightLine: 2 });
  });

  it('shows an added line on the right only, with its own line number', () => {
    const rows = diffLines('a\nc', 'a\nb\nc');

    expect(rows.map(row => row.kind)).toEqual(['same', 'added', 'same']);
    expect(rows[1]).toMatchObject({ left: null, right: 'b', leftLine: null, rightLine: 2 });
    expect(rows[2]).toMatchObject({ leftLine: 2, rightLine: 3 });
  });

  it('shows a removed line on the left only', () => {
    expect(kinds('a\nb\nc', 'a\nc')).toEqual(['same', 'removed', 'same']);
  });

  it('pairs what it can of a run, then removes or adds the rest', () => {
    expect(kinds('a\nx\ny\nz\nd', 'a\n1\nd')).toEqual(['same', 'changed', 'removed', 'removed', 'same']);
    expect(kinds('a\nd', 'a\n1\n2\n3\nd')).toEqual(['same', 'added', 'added', 'added', 'same']);
  });

  it('accepts Windows line breaks and ignores the final one', () => {
    expect(kinds('a\r\nb\r\n', 'a\nb')).toEqual(['same', 'same']);
  });

  it('handles an empty side', () => {
    expect(kinds('', 'a\nb')).toEqual(['added', 'added']);
    expect(kinds('a', '')).toEqual(['removed']);
  });

  it('replaces a very large difference as one block instead of building the table', () => {
    const left = Array.from({ length: 2500 }, (_, index) => `left ${index}`).join('\n');
    const right = Array.from({ length: 2500 }, (_, index) => `right ${index}`).join('\n');
    const rows = diffLines(left, right);

    expect(rows).toHaveLength(2500);
    expect(rows.every(row => row.kind === 'changed')).toBe(true);
  });
});
