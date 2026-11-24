import { describe, it, expect } from 'vitest';
import { NoteUrl, linkStatus } from '../src/app/core/utils/note-paths.util';

const UUID = '0b1c2d3e-4f50-4a6b-8c7d-9e0f1a2b3c4d';
const resolve = (target: string): number | null => (target === 'Folder/Here' ? 1 : null);
const exists = (url: NoteUrl): boolean => (url.kind === 'id' ? url.id === 1 : url.uuid === UUID);

describe('linkStatus', () => {
  it('recognises existing notes by path or by note://id', () => {
    expect(linkStatus('Folder/Here', resolve, exists)).toBe('note');
    expect(linkStatus('note://1', resolve, exists)).toBe('note');
    expect(linkStatus(`note://${UUID}`, resolve, exists)).toBe('note');
    expect(linkStatus(`note://${UUID.toUpperCase()}`, resolve, exists)).toBe('note');
  });

  it('flags local links pointing to nothing as broken', () => {
    expect(linkStatus('Folder/Gone', resolve, exists)).toBe('broken');
    expect(linkStatus('note://42', resolve, exists)).toBe('broken');
    expect(linkStatus('note://ffffffff-ffff-4fff-8fff-ffffffffffff', resolve, exists)).toBe('broken');
  });

  it('never flags external addresses nor anchors', () => {
    expect(linkStatus('https://example.com', resolve, exists)).toBe('external');
    expect(linkStatus('mailto:a@b.c', resolve, exists)).toBe('external');
    expect(linkStatus('#section', resolve, exists)).toBe('anchor');
  });
});
