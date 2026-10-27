import { describe, it, expect } from 'vitest';
import { CollectionNode } from '../src/app/core/models/collection.model';
import {
  copiedNoteContent,
  copiedNotePaths,
  copiedNoteTitle,
  planCollectionCopy,
  retargetCopiedLinks,
  topLevelSources,
} from '../src/app/core/utils/collection-copy.util';

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
  it('is the file name without its extension', () => {
    expect(copiedNoteTitle('meeting notes.md')).toBe('meeting notes');
    expect(copiedNoteTitle('todo.markdown')).toBe('todo');
    expect(copiedNoteTitle('v1.2.md')).toBe('v1.2');
  });

  it('keeps a name made of an extension only', () => {
    expect(copiedNoteTitle('.md')).toBe('.md');
  });
});

describe('copiedNoteContent', () => {
  it('puts the title as first line', () => {
    expect(copiedNoteContent('# Getting started\n\nText', 'intro')).toBe('# intro\n\n# Getting started\n\nText');
    expect(copiedNoteContent('', 'intro')).toBe('# intro\n\n');
  });

  it('keeps a file already starting with its title', () => {
    expect(copiedNoteContent('<!-- draft -->\n# intro\nText', 'intro')).toBe('<!-- draft -->\n# intro\nText');
  });
});

describe('retargetCopiedLinks', () => {
  const folders = [{ id: 1, name: 'Docs', parentId: null }, { id: 2, name: 'guide', parentId: 1 }, { id: 3, name: 'api', parentId: 2 }];
  const paths = copiedNotePaths([
    { ref: { root: ROOT, path: 'guide/intro.md' }, title: 'intro', folderId: 2 },
    { ref: { root: ROOT, path: 'guide/api/Auth Flow.md' }, title: 'Auth Flow', folderId: 3 },
    { ref: { root: ROOT, path: 'guide/api/index.md' }, title: 'index', folderId: 3 },
    { ref: { root: ROOT, path: 'README.md' }, title: 'README', folderId: 1 },
  ], folders);
  const intro = { root: ROOT, path: 'guide/intro.md' };

  it('points the relative links to the copied files at their notes', () => {
    const content = 'See [auth](api/Auth%20Flow.md), [home](../readme.md) and [again](<./api/Auth Flow>).';

    expect(retargetCopiedLinks(content, intro, paths))
      .toBe('See [auth](<Docs/guide/api/Auth Flow>), [home](Docs/README) and [again](<Docs/guide/api/Auth Flow>).');
  });

  it('resolves links from the root of the collection, and drops their anchor', () => {
    expect(retargetCopiedLinks('[a](/guide/intro.md#setup "Intro")', { root: ROOT, path: 'README.md' }, paths))
      .toBe('[a](Docs/guide/intro "Intro")');
  });

  it('leads a link to a folder to its index page', () => {
    expect(retargetCopiedLinks('[api](api/)', intro, paths)).toBe('[api](Docs/guide/api/index)');
  });

  it('keeps the label even when it reads like the target', () => {
    expect(retargetCopiedLinks('[../README.md](../README.md)', intro, paths)).toBe('[../README.md](Docs/README)');
  });

  it('keeps what does not lead to a copied file', () => {
    const content = '[web](https://example.com) [top](#top) [gone](missing.md) ![img](api/index.md) [other](../../out.md)';

    expect(retargetCopiedLinks(content, intro, paths)).toBe(content);
  });

  it('never resolves a link into another collection', () => {
    expect(retargetCopiedLinks('[x](intro.md)', { root: 'D:\\other', path: 'guide/a.md' }, paths)).toBe('[x](intro.md)');
  });
});
