import { describe, it, expect } from 'vitest';
import { translate } from '../src/app/core/i18n/translate';
import { Folder } from '../src/app/core/models/folder.model';
import {
  buildFolderTree,
  canMoveFolder,
  clampSectionHeight,
  clampSidebarWidth,
  collectSubtreeIds,
  flattenVisibleTree,
  folderDisplayName,
  folderOptions,
  visibleFolderOptions,
  isGettingStartedFolderName,
  DROP_EDGE_RATIO,
  folderDepth,
  insertSibling,
  resolveFolderRowDrop,
  resolveNoteRowDrop,
  isFolderDescendant,
  SECTION_MIN_HEIGHT,
  SIDEBAR_DEFAULT_WIDTH,
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
} from '../src/app/core/utils/folder-tree.util';

function folder(
  id: number,
  name: string,
  parentId: number | null = null,
  isDefault = false,
  sortOrder: number | null = null,
): Folder {
  return { id, name, parentId, createdAt: '', isDefault, sortOrder };
}

function note(
  id: number,
  title: string,
  folderId: number | null,
  sortOrder: number | null = null,
): { id: number; title: string; folderId: number | null; sortOrder: number | null } {
  return { id, title, folderId, sortOrder };
}

// Inbox(1, default) | Work(2) > Projects(3) > Archive(4) | Perso(5)
const FOLDERS: Folder[] = [
  folder(1, 'Inbox', null, true),
  folder(2, 'Work'),
  folder(3, 'Projects', 2),
  folder(4, 'Archive', 3),
  folder(5, 'Perso'),
];

describe('buildFolderTree', () => {
  it('nests folders under their parent, with the depth of each node', () => {
    const tree = buildFolderTree(FOLDERS, []);
    const work = tree.find(n => n.folder.id === 2);

    expect(work?.depth).toBe(0);
    expect(work?.children[0]?.folder.id).toBe(3);
    expect(work?.children[0]?.depth).toBe(1);
    expect(work?.children[0]?.children[0]?.depth).toBe(2);
  });

  it('puts the default folder first, then the others by name', () => {
    const names = buildFolderTree(FOLDERS, []).map(n => n.folder.name);
    expect(names).toEqual(['Inbox', 'Perso', 'Work']);
  });

  it('attaches the notes to their folder, sorted by title', () => {
    const tree = buildFolderTree(FOLDERS, [note(1, 'b', 2), note(2, 'a', 2), note(3, 'c', 5)]);
    const work = tree.find(n => n.folder.id === 2);

    expect(work?.notes.map(n => n.id)).toEqual([2, 1]);
  });

  it('counts the notes of a folder and of all its sub-folders', () => {
    const tree = buildFolderTree(FOLDERS, [note(1, 'a', 2), note(2, 'b', 3), note(3, 'c', 4), note(4, 'd', 5)]);
    const work = tree.find(n => n.folder.id === 2);

    expect(work?.noteCount).toBe(3);
    expect(work?.children[0]?.noteCount).toBe(2);
    expect(tree.find(n => n.folder.id === 5)?.noteCount).toBe(1);
  });

  it('ignores the notes without folder (they live at the root, outside of the folders)', () => {
    const tree = buildFolderTree(FOLDERS, [note(1, 'a', null)]);
    expect(tree.every(n => n.noteCount === 0)).toBe(true);
  });

  it('ignores the notes pointing to an unknown folder', () => {
    const tree = buildFolderTree(FOLDERS, [note(1, 'a', 999)]);
    expect(tree.every(n => n.noteCount === 0)).toBe(true);
  });

  it('promotes to the root a folder whose parent does not exist', () => {
    const tree = buildFolderTree([folder(7, 'Orphan', 999)], []);
    expect(tree.map(n => n.folder.id)).toEqual([7]);
  });
});

describe('flattenVisibleTree', () => {
  const tree = buildFolderTree(FOLDERS, [note(1, 'a', 2), note(2, 'b', 3)]);

  it('lists only the root folders when nothing is expanded', () => {
    const rows = flattenVisibleTree(tree, new Set());
    expect(rows.map(r => r.kind)).toEqual(['folder', 'folder', 'folder']);
  });

  it('lists sub-folders then notes of an expanded folder, in tree order', () => {
    const rows = flattenVisibleTree(tree, new Set([2]));
    const ids = rows.map(r => (r.kind === 'folder' ? `f${r.node.folder.id}` : `n${r.note.id}`));

    expect(ids).toEqual(['f1', 'f5', 'f2', 'f3', 'n1']);
  });

  it('does not list the content of a collapsed folder even when its parent is expanded', () => {
    const rows = flattenVisibleTree(tree, new Set([2]));
    expect(rows.some(r => r.kind === 'note' && r.note.id === 2)).toBe(false);
  });

  it('gives each row its depth', () => {
    const rows = flattenVisibleTree(tree, new Set([2, 3]));
    const noteRow = rows.find(r => r.kind === 'note' && r.note.id === 2);

    expect(noteRow?.depth).toBe(2);
  });
});

describe('isFolderDescendant', () => {
  it('is true for the folder itself and for any depth below the ancestor', () => {
    expect(isFolderDescendant(FOLDERS, 2, 2)).toBe(true);
    expect(isFolderDescendant(FOLDERS, 4, 2)).toBe(true);
  });

  it('is false for a sibling branch or an ancestor', () => {
    expect(isFolderDescendant(FOLDERS, 5, 2)).toBe(false);
    expect(isFolderDescendant(FOLDERS, 2, 4)).toBe(false);
  });

  it('does not loop forever on corrupted data containing a cycle', () => {
    const cyclic = [folder(1, 'a', 2), folder(2, 'b', 1)];
    expect(isFolderDescendant(cyclic, 1, 99)).toBe(false);
  });
});

describe('canMoveFolder', () => {
  it('allows moving a folder to the root or to an unrelated folder', () => {
    expect(canMoveFolder(FOLDERS, 4, null)).toBe(true);
    expect(canMoveFolder(FOLDERS, 4, 5)).toBe(true);
  });

  it('refuses to move a folder into itself or into one of its descendants', () => {
    expect(canMoveFolder(FOLDERS, 2, 2)).toBe(false);
    expect(canMoveFolder(FOLDERS, 2, 4)).toBe(false);
  });

  it('refuses a move that would not change anything', () => {
    expect(canMoveFolder(FOLDERS, 3, 2)).toBe(false);
    expect(canMoveFolder(FOLDERS, 2, null)).toBe(false);
  });

  it('refuses to move the default folder, and unknown folders', () => {
    expect(canMoveFolder(FOLDERS, 1, 5)).toBe(false);
    expect(canMoveFolder(FOLDERS, 999, 5)).toBe(false);
  });
});

describe('folderDepth', () => {
  it('is 1 for a root folder and grows with each level', () => {
    expect(folderDepth(FOLDERS, 2)).toBe(1);
    expect(folderDepth(FOLDERS, 4)).toBe(3);
  });

  it('is 0 for an unknown folder', () => {
    expect(folderDepth(FOLDERS, 999)).toBe(0);
  });
});

describe('collectSubtreeIds', () => {
  it('returns the folder and all its descendants', () => {
    expect(collectSubtreeIds(FOLDERS, 2).sort()).toEqual([2, 3, 4]);
  });

  it('returns only the folder when it has no child', () => {
    expect(collectSubtreeIds(FOLDERS, 5)).toEqual([5]);
  });
});

describe('clampSidebarWidth', () => {
  it('keeps a width within the bounds', () => {
    expect(clampSidebarWidth(300)).toBe(300);
  });

  it('clamps too small and too large widths', () => {
    expect(clampSidebarWidth(10)).toBe(SIDEBAR_MIN_WIDTH);
    expect(clampSidebarWidth(5000)).toBe(SIDEBAR_MAX_WIDTH);
  });

  it('falls back to the default width for non numeric values', () => {
    expect(clampSidebarWidth(Number.NaN)).toBe(SIDEBAR_DEFAULT_WIDTH);
    expect(clampSidebarWidth('abc')).toBe(SIDEBAR_DEFAULT_WIDTH);
  });
});

describe('manual order', () => {
  it('lists the positioned folders first, in their order, then the others alphabetically', () => {
    const folders = [folder(1, 'Inbox', null, true), folder(2, 'B', null, false, 2), folder(3, 'A', null, false, 1), folder(4, 'Z')];
    expect(buildFolderTree(folders, []).map(n => n.folder.name)).toEqual(['Inbox', 'A', 'B', 'Z']);
  });

  it('orders the notes of a folder the same way', () => {
    const tree = buildFolderTree(FOLDERS, [note(1, 'a', 5), note(2, 'b', 5, 2), note(3, 'c', 5, 1)]);
    expect(tree.find(n => n.folder.id === 5)?.notes.map(n => n.id)).toEqual([3, 2, 1]);
  });
});

describe('resolveFolderRowDrop', () => {
  const work = FOLDERS[1] as Folder;
  const inbox = FOLDERS[0] as Folder;

  it('puts a folder before / after the row on its top / bottom edge', () => {
    expect(resolveFolderRowDrop('folder', work, false, 0.1))
      .toEqual({ kind: 'between', parentId: null, anchorId: 2, side: 'before' });
    expect(resolveFolderRowDrop('folder', work, false, 0.9))
      .toEqual({ kind: 'between', parentId: null, anchorId: 2, side: 'after' });
  });

  it('drops into the folder in the middle of the row', () => {
    expect(resolveFolderRowDrop('folder', work, false, 0.5)).toEqual({ kind: 'into', folderId: 2 });
  });

  it('never drops a note beside a folder: it goes inside', () => {
    expect(resolveFolderRowDrop('note', work, false, 0.05)).toEqual({ kind: 'into', folderId: 2 });
  });

  it('does not place after an open folder (its content comes next), nor before the default folder', () => {
    expect(resolveFolderRowDrop('folder', work, true, 0.9)).toEqual({ kind: 'into', folderId: 2 });
    expect(resolveFolderRowDrop('folder', inbox, false, 0.1)).toEqual({ kind: 'into', folderId: 1 });
  });

  it('uses edge zones smaller than half a row', () => {
    expect(DROP_EDGE_RATIO).toBeGreaterThan(0);
    expect(DROP_EDGE_RATIO).toBeLessThan(0.5);
  });
});

describe('resolveNoteRowDrop', () => {
  const target = note(7, 't', 2);

  it('places a note before / after depending on the half of the row', () => {
    expect(resolveNoteRowDrop('note', target, 2, 0.2)).toEqual({ kind: 'between', parentId: 2, anchorId: 7, side: 'before' });
    expect(resolveNoteRowDrop('note', target, 2, 0.8)).toEqual({ kind: 'between', parentId: 2, anchorId: 7, side: 'after' });
  });

  it('sends a dragged folder into the folder holding the note', () => {
    expect(resolveNoteRowDrop('folder', target, 2, 0.2)).toEqual({ kind: 'into', folderId: 2 });
    expect(resolveNoteRowDrop('folder', target, null, 0.2)).toEqual({ kind: 'into', folderId: null });
  });
});

describe('insertSibling', () => {
  it('inserts before or after the anchor', () => {
    expect(insertSibling([1, 2, 3], 9, 2, 'before')).toEqual([1, 9, 2, 3]);
    expect(insertSibling([1, 2, 3], 9, 2, 'after')).toEqual([1, 2, 9, 3]);
  });

  it('moves an item already among the siblings', () => {
    expect(insertSibling([1, 2, 3], 1, 3, 'after')).toEqual([2, 3, 1]);
  });

  it('keeps a pinned first sibling in front', () => {
    expect(insertSibling([1, 2, 3], 3, 1, 'before', 1)).toEqual([1, 3, 2]);
  });

  it('appends when the anchor is unknown', () => {
    expect(insertSibling([1, 2], 9, 99, 'before')).toEqual([1, 2, 9]);
  });
});

describe('clampSectionHeight', () => {
  it('keeps a height within the bounds, rounded', () => {
    expect(clampSectionHeight(120.6, 400)).toBe(121);
  });

  it('clamps to the minimum and to the given maximum', () => {
    expect(clampSectionHeight(10, 400)).toBe(SECTION_MIN_HEIGHT);
    expect(clampSectionHeight(900, 400)).toBe(400);
  });

  it('prefers the minimum when the maximum is smaller (tiny window)', () => {
    expect(clampSectionHeight(100, 20)).toBe(SECTION_MIN_HEIGHT);
  });
});

describe('folderDisplayName', () => {
  const en = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>): string => translate('en', key, params);
  const fr = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>): string => translate('fr', key, params);

  it('translates the default folder whatever its stored name', () => {
    expect(folderDisplayName(folder(1, 'Inbox', null, true), en)).toBe('Inbox');
    expect(folderDisplayName(folder(1, 'Inbox', null, true), fr)).toBe('Boîte de réception');
  });

  it('translates the sample notes folder created in either language', () => {
    expect(folderDisplayName(folder(2, 'Getting started'), fr)).toBe('Premiers pas');
    expect(folderDisplayName(folder(2, 'Premiers pas'), en)).toBe('Getting started');
  });

  it('keeps the name of any other folder', () => {
    expect(folderDisplayName(folder(3, 'Work'), fr)).toBe('Work');
  });

  it('recognises the sample notes folder name in both languages only', () => {
    expect(isGettingStartedFolderName('Getting started')).toBe(true);
    expect(isGettingStartedFolderName('Premiers pas')).toBe(true);
    expect(isGettingStartedFolderName('Inbox')).toBe(false);
  });
});

describe('folderOptions', () => {
  const t = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) => translate('en', key, params);

  it('lists the root first, then every folder in tree order with its depth', () => {
    const options = folderOptions(FOLDERS, 'Notes', new Set([3, 4]), t);

    expect(options[0]).toEqual({ id: null, label: 'Notes', depth: 0, disabled: false });
    expect(options.map(option => option.id).slice(1).sort()).toEqual([1, 2, 3, 4, 5]);
    const work = options.findIndex(option => option.id === 2);
    expect(options.slice(work, work + 3).map(option => [option.id, option.depth, option.disabled])).toEqual([
      [2, 0, false], [3, 1, true], [4, 2, true],
    ]);
  });
});

describe('visibleFolderOptions', () => {
  const options = [
    { id: null, label: 'Notes', depth: 0, disabled: false },
    { id: 2, label: 'Work', depth: 0, disabled: false },
    { id: 3, label: 'Projects', depth: 1, disabled: false },
    { id: 4, label: 'Archive', depth: 2, disabled: false },
    { id: 5, label: 'Perso', depth: 0, disabled: false },
  ];

  it('shows everything when nothing is collapsed, marking the folders with sub-folders', () => {
    expect(visibleFolderOptions(options, new Set()).map(row => [row.option.id, row.expandable])).toEqual([
      [null, false], [2, true], [3, true], [4, false], [5, false],
    ]);
  });

  it('hides every sub-folder of a collapsed folder, not its next siblings', () => {
    expect(visibleFolderOptions(options, new Set([2])).map(row => row.option.id)).toEqual([null, 2, 5]);
    expect(visibleFolderOptions(options, new Set([3])).map(row => row.option.id)).toEqual([null, 2, 3, 5]);
  });

  it('ignores a collapsed folder without sub-folders', () => {
    expect(visibleFolderOptions(options, new Set([4, 5]))).toHaveLength(5);
  });
});
