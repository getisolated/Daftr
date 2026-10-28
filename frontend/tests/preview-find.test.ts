import { describe, it, expect } from 'vitest';
import { findInText, findRegExp, rankInBlock } from '../src/app/shared/markdown/preview-find';
import { FindOptions } from '../src/app/core/models/find.model';

const options = (search: string, patch: Partial<FindOptions> = {}): FindOptions => ({
  search, replace: '', caseSensitive: false, wholeWord: false, regexp: false, ...patch,
});

describe('findInText', () => {
  it('lists every occurrence, ignoring case by default', () => {
    expect(findInText('Chat chat', options('chat'))).toEqual([{ from: 0, to: 4 }, { from: 5, to: 9 }]);
  });

  it('follows the case, whole word and regexp options', () => {
    const text = 'Chat chat chats';
    expect(findInText(text, options('chat', { caseSensitive: true }))).toHaveLength(2);
    expect(findInText(text, options('chat', { wholeWord: true }))).toHaveLength(2);
    expect(findInText(text, options('chats?$', { regexp: true }))).toEqual([{ from: 10, to: 15 }]);
  });

  it('reads a plain search literally, except the escaped line breaks', () => {
    expect(findInText('a.b axb', options('a.b'))).toEqual([{ from: 0, to: 3 }]);
    expect(findInText('un\ndeux', options('un\\ndeux'))).toEqual([{ from: 0, to: 7 }]);
  });

  it('treats accented letters as word characters', () => {
    expect(findInText('été étés', options('été', { wholeWord: true }))).toEqual([{ from: 0, to: 3 }]);
  });

  it('matches nothing for an empty search, an invalid regexp or empty matches', () => {
    expect(findInText('abc', options(''))).toEqual([]);
    expect(findInText('abc', options('(', { regexp: true }))).toEqual([]);
    expect(findInText('abc', options('^', { regexp: true }))).toEqual([]);
    expect(findRegExp(options('(', { regexp: true }))).toBeNull();
  });

  it('stops at the limit', () => {
    expect(findInText('a a a a', options('a'), 3)).toHaveLength(3);
  });
});

describe('rankInBlock', () => {
  it('counts the matches before the current one in the same block', () => {
    const lines = [1, 3, 3, 4, 8];
    expect(rankInBlock(lines, 0, 1, 3)).toBe(0);
    expect(rankInBlock(lines, 2, 3, 8)).toBe(1);
    expect(rankInBlock(lines, 3, 3, 8)).toBe(2);
    expect(rankInBlock(lines, 4, 8, Infinity)).toBe(0);
  });
});
