import { describe, it, expect } from 'vitest';
import { extractNoteTitle as extractTitle, noteDisplayTitle } from '../src/app/core/utils/note-title.util';


describe('extractTitle', () => {
  it('extracts plain text from first line', () => {
    expect(extractTitle('My Note\nSome content')).toBe('My Note');
  });

  it('strips markdown heading syntax', () => {
    expect(extractTitle('# My Heading')).toBe('My Heading');
    expect(extractTitle('## Sub Heading')).toBe('Sub Heading');
    expect(extractTitle('### Deep Heading')).toBe('Deep Heading');
  });

  it('strips multiple hash marks', () => {
    expect(extractTitle('###### H6 Title')).toBe('H6 Title');
  });

  it('returns an empty title for empty content', () => {
    expect(extractTitle('')).toBe('');
    expect(extractTitle('\n\n')).toBe('');
  });

  it('returns an empty title for heading with no text', () => {
    expect(extractTitle('# ')).toBe('');
    expect(extractTitle('##')).toBe('');
  });

  it('trims whitespace around title', () => {
    expect(extractTitle('#  Spaced  ')).toBe('Spaced');
  });

  it('treats indented hash as plain text (not a heading)', () => {
    expect(extractTitle('  # Not a heading')).toBe('# Not a heading');
  });

  it('uses only the first line', () => {
    expect(extractTitle('Line 1\n# Line 2')).toBe('Line 1');
  });

  it('skips blank lines and draft markers', () => {
    expect(extractTitle('\n<!-- draft -->\n# Real title\n<!-- /draft -->')).toBe('Real title');
  });

  it('skips single and multi-line HTML comments', () => {
    expect(extractTitle('<!-- meta -->Title')).toBe('Title');
    expect(extractTitle('<!--\nmeta: 1\n-->\nTitle')).toBe('Title');
  });

  it('returns an empty title when only comments remain', () => {
    expect(extractTitle('<!-- draft -->\n<!-- /draft -->')).toBe('');
    expect(extractTitle('<!-- unclosed')).toBe('');
  });
});

describe('noteDisplayTitle', () => {
  const t = (key: string): string => `[${key}]`;

  it('keeps a real title', () => {
    expect(noteDisplayTitle('My Note', t)).toBe('My Note');
  });

  it('falls back to the translated placeholder for an empty or legacy title', () => {
    expect(noteDisplayTitle('', t)).toBe('[common.untitled]');
    expect(noteDisplayTitle(null, t)).toBe('[common.untitled]');
    expect(noteDisplayTitle('Untitled', t)).toBe('[common.untitled]');
  });
});
