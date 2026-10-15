import { describe, it, expect } from 'vitest';
import { Folder } from '../src/app/core/models/folder.model';
import { findSampleNotesToGather } from '../src/app/core/utils/sample-notes.util';

const TITLES = ['Markdown Basics', 'Code Blocks'];
const INBOX: Folder = { id: 1, name: 'Inbox', parentId: null, createdAt: '', isDefault: true, sortOrder: null };
const GETTING_STARTED: Folder = { id: 2, name: 'Getting started', parentId: null, createdAt: '', isDefault: false, sortOrder: null };

describe('findSampleNotesToGather', () => {
  it('returns the sample notes sitting at the root', () => {
    const notes = [
      { id: 1, title: 'Markdown Basics', folderId: null, sortOrder: null },
      { id: 2, title: 'My note', folderId: null, sortOrder: null },
    ];

    expect(findSampleNotesToGather([INBOX], notes, TITLES).map(n => n.id)).toEqual([1]);
  });

  it('leaves a sample note the user already filed in a folder', () => {
    const notes = [{ id: 1, title: 'Code Blocks', folderId: 1, sortOrder: null }];

    expect(findSampleNotesToGather([INBOX], notes, TITLES)).toEqual([]);
  });

  it('does nothing once the Getting started folder exists', () => {
    const notes = [{ id: 1, title: 'Code Blocks', folderId: null, sortOrder: null }];

    expect(findSampleNotesToGather([INBOX, GETTING_STARTED], notes, TITLES)).toEqual([]);
  });
});
