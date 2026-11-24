import { describe, it, expect } from 'vitest';
import { completedLinkTarget } from '../src/app/shared/codemirror/note-link-completion';

const UUID = '0B1C2D3E-4F50-4A6B-8C7D-9E0F1A2B3C4D';

describe('completedLinkTarget', () => {
  it('links a note by its uuid, in lowercase, whatever its path or the angle bracket typed', () => {
    const entry = { kind: 'note' as const, label: 'My note', path: 'Work/My note', uuid: UUID };

    expect(completedLinkTarget(entry, false)).toBe('note://0b1c2d3e-4f50-4a6b-8c7d-9e0f1a2b3c4d)');
    expect(completedLinkTarget(entry, true)).toBe('note://0b1c2d3e-4f50-4a6b-8c7d-9e0f1a2b3c4d)');
  });

  it('keeps the path of a note without uuid', () => {
    expect(completedLinkTarget({ kind: 'note', label: 'Todo', path: 'Todo' }, false)).toBe('Todo)');
    expect(completedLinkTarget({ kind: 'note', label: 'My note', path: 'My note' }, false)).toBe('<My note>)');
  });

  it('extends the path with a folder, without closing the link', () => {
    expect(completedLinkTarget({ kind: 'folder', label: 'Work/', path: 'Work/' }, false)).toBe('Work/');
    expect(completedLinkTarget({ kind: 'folder', label: 'My work/', path: 'My work/' }, false)).toBe('<My work/');
  });
});
