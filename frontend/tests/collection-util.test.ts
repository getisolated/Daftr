import { describe, it, expect } from 'vitest';
import { baseName, collectionLinkStatus, flattenFiles, folderPage, indexCollection, labelCollections, movedPath, parentPath, resolveRelativeLink, resolveRelativePath } from '../src/app/core/utils/collection.util';

const collection = (path: string, displayName = baseName(path)) => ({ path, kind: 'folder' as const, displayName, sortOrder: 0 });

describe('baseName / parentPath', () => {
  it('handles both separators', () => {
    expect(baseName('C:\\repos\\a\\docs')).toBe('docs');
    expect(parentPath('docs/guide/intro.md')).toBe('docs/guide');
    expect(parentPath('intro.md')).toBe('');
  });
});

describe('labelCollections', () => {
  it('adds the parent folder to homonyms only', () => {
    expect(labelCollections([collection('C:\\a\\docs'), collection('C:\\b\\docs'), collection('C:\\c\\notes')])).toEqual([
      { label: 'docs', hint: 'a' },
      { label: 'docs', hint: 'b' },
      { label: 'notes', hint: null },
    ]);
  });

  it('falls back to the whole parent path when the parents share their name too', () => {
    const [first] = labelCollections([collection('C:\\w\\repo\\docs'), collection('D:\\v\\repo\\docs')]);
    expect(first?.hint).toBe('C:\\w\\repo');
  });

  it('compares the display names, which the user may change', () => {
    expect(labelCollections([collection('C:\\a\\docs', 'Front'), collection('C:\\b\\docs', 'Back')]).map(l => l.hint)).toEqual([null, null]);
  });
});

describe('movedPath', () => {
  it('follows a renamed file or folder', () => {
    expect(movedPath('a/b.md', 'a/b.md', 'a/c.md')).toBe('a/c.md');
    expect(movedPath('a/x/y.md', 'a/x', 'z')).toBe('z/y.md');
    expect(movedPath('a/xy.md', 'a/x', 'z')).toBeNull();
  });
});

describe('flattenFiles', () => {
  it('lists the files depth first', () => {
    const tree = [
      { name: 'docs', path: 'docs', kind: 'folder' as const, children: [{ name: 'a.md', path: 'docs/a.md', kind: 'file' as const }] },
      { name: 'README.md', path: 'README.md', kind: 'file' as const },
    ];
    expect(flattenFiles('R', tree).map(file => file.path)).toEqual(['docs/a.md', 'README.md']);
  });
});

describe('resolveRelativeLink', () => {
  it('resolves relative paths against the folder of the current file', () => {
    expect(resolveRelativeLink('docs/guide/intro.md', './setup.md')).toEqual({ kind: 'file', path: 'docs/guide/setup.md' });
    expect(resolveRelativeLink('docs/guide/intro.md', '../api/index.md#auth')).toEqual({ kind: 'file', path: 'docs/api/index.md' });
    expect(resolveRelativeLink('docs/intro.md', '/README.md')).toEqual({ kind: 'file', path: 'README.md' });
    expect(resolveRelativeLink('docs/intro.md', 'Other%20page')).toEqual({ kind: 'file', path: 'docs/Other page.md' });
  });

  it('refuses web addresses, bare anchors and paths leaving the collection', () => {
    expect(resolveRelativeLink('a.md', 'https://x.dev')).toBeNull();
    expect(resolveRelativeLink('a.md', '#top')).toBeNull();
    expect(resolveRelativeLink('a.md', '../outside.md')).toBeNull();
  });

  it('refuses files of another extension', () => {
    expect(resolveRelativeLink('a.md', 'src/WMSAlign.Enum.al')).toBeNull();
  });
});

describe('collectionLinkStatus', () => {
  const files = indexCollection([
    { name: 'README.md', path: 'README.md', kind: 'file' },
    {
      name: 'docs', path: 'docs', kind: 'folder', children: [
        { name: 'events.md', path: 'docs/events.md', kind: 'file' },
        { name: 'Navigation.md', path: 'docs/Navigation.md', kind: 'file' },
      ],
    },
  ]);

  it('resolves a relative or extension-less target from the open file', () => {
    expect(collectionLinkStatus('docs/events.md', 'navigation.md', files)).toBe('note');
    expect(collectionLinkStatus('docs/events.md', './navigation', files)).toBe('note');
    expect(collectionLinkStatus('docs/events.md', '../README.md#top', files)).toBe('note');
  });

  it('flags a markdown page missing from the collection', () => {
    expect(collectionLinkStatus('docs/events.md', 'missing.md', files)).toBe('broken');
    expect(collectionLinkStatus('docs/events.md', 'README.md', files)).toBe('broken');
  });

  it('never flags what it cannot check', () => {
    expect(collectionLinkStatus('docs/events.md', 'missing.md', null)).toBe('note');
    expect(collectionLinkStatus('docs/events.md', 'diagram.png', files)).toBe('note');
    expect(collectionLinkStatus('docs/events.md', 'https://example.com', files)).toBe('external');
    expect(collectionLinkStatus('docs/events.md', '#section', files)).toBe('anchor');
  });
});

describe('resolveRelativePath', () => {
  it('resolves an image next to the open file, or from the root of the collection', () => {
    expect(resolveRelativePath('doc/v28/index.md', 'Images/plan.gif')).toBe('doc/v28/Images/plan.gif');
    expect(resolveRelativePath('doc/v28/index.md', '../shared/logo%20bleu.png')).toBe('doc/shared/logo bleu.png');
    expect(resolveRelativePath('doc/v28/index.md', '/assets/a.png')).toBe('assets/a.png');
    expect(resolveRelativePath('doc/v28/index.md', '~/assets/a.png')).toBe('assets/a.png');
  });

  it('leaves web addresses and paths leaving the collection', () => {
    expect(resolveRelativePath('a.md', 'https://x.dev/a.png')).toBeNull();
    expect(resolveRelativePath('a.md', 'data:image/png;base64,AAA')).toBeNull();
    expect(resolveRelativePath('a.md', '../out.png')).toBeNull();
  });
});

describe('resolveRelativeLink to a folder', () => {
  it('designates the folder itself', () => {
    expect(resolveRelativeLink('doc/guide/a.md', '../')).toEqual({ kind: 'folder', path: 'doc' });
    expect(resolveRelativeLink('doc/a.md', './api/')).toEqual({ kind: 'folder', path: 'doc/api' });
    expect(resolveRelativeLink('doc/a.md', '..')).toEqual({ kind: 'folder', path: '' });
    expect(resolveRelativeLink('a.md', '../')).toBeNull();
  });

  it('resolves ~/ from the root of the collection', () => {
    expect(resolveRelativeLink('doc/v28/a.md', '~/doc/v27/index.md')).toEqual({ kind: 'file', path: 'doc/v27/index.md' });
    expect(resolveRelativeLink('doc/v28/a.md', '~/doc/guide')).toEqual({ kind: 'file', path: 'doc/guide.md' });
    expect(resolveRelativeLink('doc/v28/a.md', '~/')).toEqual({ kind: 'folder', path: '' });
    expect(resolveRelativeLink('doc/v28/a.md', '~')).toEqual({ kind: 'folder', path: '' });
  });
});

describe('folder links', () => {
  const index = indexCollection([
    { name: 'cap-caisse', path: 'cap-caisse', kind: 'folder', children: [
      { name: 'v28', path: 'cap-caisse/v28', kind: 'folder', children: [{ name: 'index.md', path: 'cap-caisse/v28/index.md', kind: 'file' }] },
      { name: 'v26', path: 'cap-caisse/v26', kind: 'folder', children: [{ name: 'client.md', path: 'cap-caisse/v26/client.md', kind: 'file' }] },
    ] },
    { name: 'guide', path: 'guide', kind: 'folder', children: [{ name: 'README.md', path: 'guide/README.md', kind: 'file' }] },
  ]);

  it('accepts a folder that exists, with or without an index page', () => {
    expect(collectionLinkStatus('cap-caisse/v26/client.md', '../', index)).toBe('note');
    expect(collectionLinkStatus('cap-caisse/v26/client.md', '../v28/', index)).toBe('note');
    expect(collectionLinkStatus('cap-caisse/v26/client.md', '../v99/', index)).toBe('broken');
  });

  it('opens the index.md, else the README.md, else nothing', () => {
    expect(folderPage(index, 'cap-caisse/v28')).toBe('cap-caisse/v28/index.md');
    expect(folderPage(index, 'Guide')).toBe('guide/README.md');
    expect(folderPage(index, 'cap-caisse')).toBeNull();
  });
});
