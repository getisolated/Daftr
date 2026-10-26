import { describe, it, expect } from 'vitest';
import { CollectionNode } from '../src/app/core/models/collection.model';
import { copiedNoteTitle, planCollectionCopy, topLevelSources } from '../src/app/core/utils/collection-copy.util';

const ROOT = 'C:\\docs';

function file(path: string): CollectionNode {
  return { name: path.split('/').pop() ?? path, path, kind: 'file' };
}

function folder(path: string, children: CollectionNode[]): CollectionNode {
  return { name: path.split('/').pop() ?? path, path, kind: 'folder', children };
}

describe('topLevelSources', () => {
  it('drops what a selected folder of the same collection already holds', () => {
    const guide = folder('guide', [file('guide/a.md'), folder('guide/deep', [file('guide/deep/b.md')])]);
    const sources = [
      { root: ROOT, node: guide },
      { root: ROOT, node: file('guide/a.md') },
      { root: ROOT, node: guide.children![1]! },
      { root: ROOT, node: file('readme.md') },
    ];

    expect(topLevelSources(sources).map(source => source.node.path)).toEqual(['guide', 'readme.md']);
  });

  it('keeps a same path from another collection, and a folder whose name only starts like another', () => {
    const sources = [
      { root: ROOT, node: folder('guide', []) },
      { root: 'D:\\other', node: file('guide/a.md') },
      { root: ROOT, node: file('guides/a.md') },
    ];

    expect(topLevelSources(sources)).toHaveLength(3);
  });

  it('lets a whole collection (path "") cover its files', () => {
    const sources = [
      { root: ROOT, node: folder('', [file('a.md')]) },
      { root: ROOT, node: file('a.md') },
    ];

    expect(topLevelSources(sources).map(source => source.node.path)).toEqual(['']);
  });

  it('keeps a source picked twice once', () => {
    expect(topLevelSources([{ root: ROOT, node: file('a.md') }, { root: ROOT, node: file('a.md') }])).toHaveLength(1);
  });
});

describe('planCollectionCopy', () => {
  it('recreates the folder structure, parents before children', () => {
    const guide = folder('guide', [
      file('guide/intro.md'),
      folder('guide/api', [file('guide/api/auth.md'), folder('guide/api/empty', [])]),
    ]);

    const plan = planCollectionCopy([{ root: ROOT, node: guide }, { root: ROOT, node: file('readme.md') }]);

    expect(plan.folders).toEqual([
      { name: 'guide', parent: null },
      { name: 'api', parent: 0 },
      { name: 'empty', parent: 1 },
    ]);
    expect(plan.files).toEqual([
      { ref: { root: ROOT, path: 'guide/intro.md' }, name: 'intro.md', folder: 0 },
      { ref: { root: ROOT, path: 'guide/api/auth.md' }, name: 'auth.md', folder: 1 },
      { ref: { root: ROOT, path: 'readme.md' }, name: 'readme.md', folder: null },
    ]);
  });

  it('copies a lone file straight into the target', () => {
    expect(planCollectionCopy([{ root: ROOT, node: file('notes/todo.md') }])).toEqual({
      folders: [],
      files: [{ ref: { root: ROOT, path: 'notes/todo.md' }, name: 'todo.md', folder: null }],
    });
  });
});

describe('copiedNoteTitle', () => {
  it('takes the first line, like any note', () => {
    expect(copiedNoteTitle('\n# Getting started\n\nText', 'intro.md')).toBe('Getting started');
  });

  it('falls back to the file name without its extension', () => {
    expect(copiedNoteTitle('', 'meeting notes.md')).toBe('meeting notes');
    expect(copiedNoteTitle('<!-- draft -->\n', 'todo.markdown')).toBe('todo');
  });
});
