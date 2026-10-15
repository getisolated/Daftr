import { describe, it, expect } from 'vitest';
import { formatNoteTimestamp, parseNoteDate } from '../src/app/core/utils/note-timestamp.util';

describe('parseNoteDate', () => {
  it('reads a SQLite datetime as UTC', () => {
    expect(parseNoteDate('2026-10-14 22:05:09').toISOString()).toBe('2026-10-14T22:05:09.000Z');
  });

  it('keeps ISO dates untouched', () => {
    expect(parseNoteDate('2026-10-14T22:05:09.000Z').toISOString()).toBe('2026-10-14T22:05:09.000Z');
  });

  it('gives an invalid date for garbage', () => {
    expect(isNaN(parseNoteDate('nope').getTime())).toBe(true);
  });
});

describe('formatNoteTimestamp', () => {
  it('formats as dd/MM/yyyy HH:mm:ss in local time', () => {
    expect(formatNoteTimestamp(new Date(2026, 9, 2, 9, 7, 5).toISOString())).toBe('02/10/2026 09:07:05');
  });

  it('is empty for an unusable date', () => {
    expect(formatNoteTimestamp('')).toBe('');
  });
});
