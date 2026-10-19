import { describe, it, expect } from 'vitest';
import { fileBreadcrumbs, noteBreadcrumbs } from '../src/app/core/utils/breadcrumbs.util';

const folders = [
  { id: 1, name: 'Work', parentId: null },
  { id: 2, name: 'Clients', parentId: 1 },
  { id: 3, name: 'Loop', parentId: 4 },
  { id: 4, name: 'Back', parentId: 3 },
];

describe('noteBreadcrumbs', () => {
  it('starts at the notes root and goes down the folders to the note', () => {
    expect(noteBreadcrumbs('Notes', 2, folders, 'Meeting')).toEqual(['Notes', 'Work', 'Clients', 'Meeting']);
  });

  it('puts a note of the root right under it', () => {
    expect(noteBreadcrumbs('Notes', null, folders, 'Idea')).toEqual(['Notes', 'Idea']);
  });

  it('stops on a cycle of folders', () => {
    expect(noteBreadcrumbs('Notes', 3, folders, 'N')).toEqual(['Notes', 'Back', 'Loop', 'N']);
  });
});

describe('fileBreadcrumbs', () => {
  it('starts at the collection and goes down to the file', () => {
    expect(fileBreadcrumbs('docs.capvision-cloud.fr', 'doc/fr-FR/index.md')).toEqual(['docs.capvision-cloud.fr', 'doc', 'fr-FR', 'index.md']);
  });
});
