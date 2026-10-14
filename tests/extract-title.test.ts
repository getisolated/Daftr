import { describe, it, expect } from 'vitest';
import { extractNoteTitle as extractTitle } from '../src/app/core/utils/note-title.util';


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

  it('returns "Untitled" for empty content', () => {
    expect(extractTitle('')).toBe('Untitled');
    expect(extractTitle('\n\n')).toBe('Untitled');
  });

  it('returns "Untitled" for heading with no text', () => {
    expect(extractTitle('# ')).toBe('Untitled');
    expect(extractTitle('##')).toBe('Untitled');
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

  it('returns "Untitled" when only comments remain', () => {
    expect(extractTitle('<!-- draft -->\n<!-- /draft -->')).toBe('Untitled');
    expect(extractTitle('<!-- unclosed')).toBe('Untitled');
  });
});
