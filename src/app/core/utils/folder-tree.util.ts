import { DropPlacement, Folder, FolderTreeNode, FolderTreeRow, TreeNote } from '../models/folder.model';

export const SIDEBAR_MIN_WIDTH = 180;
export const SIDEBAR_MAX_WIDTH = 600;
export const SIDEBAR_DEFAULT_WIDTH = 260;
/** A resized sidebar section keeps at least its header and a row. */
export const SECTION_MIN_HEIGHT = 48;

/** Items the user positioned come first in that order; the others follow (`null`), alphabetically. */
function compareSortOrder(a: number | null, b: number | null): number {
  if (a === b) {
    return 0;
  }

  if (a === null) {
    return 1;
  }

  return b === null ? -1 : a - b;
}

/** Default folder first (it is the landing place of new notes), then manual order, then alphabetical. */
function compareFolders(a: Folder, b: Folder): number {
  if (a.isDefault !== b.isDefault) {
    return a.isDefault ? -1 : 1;
  }

  return compareSortOrder(a.sortOrder, b.sortOrder) || a.name.localeCompare(b.name);
}

export function compareNotes(a: TreeNote, b: TreeNote): number {
  return compareSortOrder(a.sortOrder, b.sortOrder) || a.title.localeCompare(b.title);
}

/**
 * Builds the folder forest in O(n): one pass to index the children by parent, one recursion to assemble.
 * Notes without folder (or pointing to a missing one) are not part of the tree: they live at the root.
 */
export function buildFolderTree(folders: Folder[], notes: TreeNote[]): FolderTreeNode[] {
  const knownIds = new Set(folders.map(f => f.id));
  const childrenByParent = new Map<number | null, Folder[]>();

  for (const folder of folders) {
    // A folder whose parent vanished would be unreachable: it is shown at the root instead
    const parentKey = folder.parentId !== null && knownIds.has(folder.parentId) ? folder.parentId : null;
    const siblings = childrenByParent.get(parentKey) ?? [];

    siblings.push(folder);
    childrenByParent.set(parentKey, siblings);
  }

  const notesByFolder = new Map<number, TreeNote[]>();

  for (const note of notes) {
    if (note.folderId === null || !knownIds.has(note.folderId)) {
      continue;
    }

    const inFolder = notesByFolder.get(note.folderId) ?? [];

    inFolder.push(note);
    notesByFolder.set(note.folderId, inFolder);
  }

  const assemble = (folder: Folder, depth: number, visited: Set<number>): FolderTreeNode => {
    // `visited` protects against a corrupted parent cycle (it would recurse forever)
    visited.add(folder.id);

    const children = (childrenByParent.get(folder.id) ?? [])
      .filter(child => !visited.has(child.id))
      .sort(compareFolders)
      .map(child => assemble(child, depth + 1, visited));
    const ownNotes = (notesByFolder.get(folder.id) ?? []).sort(compareNotes);
    const noteCount = children.reduce((total, child) => total + child.noteCount, ownNotes.length);

    return { folder, depth, children, notes: ownNotes, noteCount };
  };

  const visited = new Set<number>();

  return (childrenByParent.get(null) ?? []).sort(compareFolders).map(root => assemble(root, 0, visited));
}

/** The rows currently visible in the sidebar, in display order: for each folder, its sub-folders then its notes. */
export function flattenVisibleTree(nodes: FolderTreeNode[], expandedIds: ReadonlySet<number>): FolderTreeRow[] {
  const rows: FolderTreeRow[] = [];

  const walk = (siblings: FolderTreeNode[]): void => {
    for (const node of siblings) {
      rows.push({ kind: 'folder', depth: node.depth, node });

      if (!expandedIds.has(node.folder.id)) {
        continue;
      }

      walk(node.children);

      for (const note of node.notes) {
        rows.push({ kind: 'note', depth: node.depth + 1, note });
      }
    }
  };

  walk(nodes);

  return rows;
}

function indexById(folders: Folder[]): Map<number, Folder> {
  return new Map(folders.map(f => [f.id, f]));
}

/** `true` when `folderId` is `ancestorId` itself or lies anywhere below it. */
export function isFolderDescendant(folders: Folder[], folderId: number, ancestorId: number): boolean {
  const byId = indexById(folders);
  const seen = new Set<number>();
  let currentId: number | null = folderId;

  while (currentId !== null && !seen.has(currentId)) {
    if (currentId === ancestorId) {
      return true;
    }

    seen.add(currentId);
    currentId = byId.get(currentId)?.parentId ?? null;
  }

  return false;
}

/** Whether dropping the folder under `targetParentId` (`null` = root) is a valid, effective move. */
export function canMoveFolder(folders: Folder[], folderId: number, targetParentId: number | null): boolean {
  const folder = folders.find(f => f.id === folderId);

  if (!folder || folder.isDefault || folder.parentId === targetParentId) {
    return false;
  }

  // Moving a folder into its own subtree would detach it from the tree
  return targetParentId === null || !isFolderDescendant(folders, targetParentId, folderId);
}

/** Level of the folder in the tree: 1 for a root folder, 0 when the folder does not exist. */
export function folderDepth(folders: Folder[], folderId: number): number {
  const byId = indexById(folders);
  const seen = new Set<number>();
  let depth = 0;
  let currentId: number | null = folderId;

  while (currentId !== null && byId.has(currentId) && !seen.has(currentId)) {
    seen.add(currentId);
    depth++;
    currentId = byId.get(currentId)?.parentId ?? null;
  }

  return depth;
}

/** The folder and every folder below it. */
export function collectSubtreeIds(folders: Folder[], folderId: number): number[] {
  const childrenByParent = new Map<number, number[]>();

  for (const folder of folders) {
    if (folder.parentId !== null) {
      const siblings = childrenByParent.get(folder.parentId) ?? [];

      siblings.push(folder.id);
      childrenByParent.set(folder.parentId, siblings);
    }
  }

  const ids = new Set<number>();
  const pending = [folderId];

  while (pending.length > 0) {
    const id = pending.pop() as number;

    if (!ids.has(id)) {
      ids.add(id);
      pending.push(...(childrenByParent.get(id) ?? []));
    }
  }

  return [...ids];
}

export function clampSidebarWidth(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return SIDEBAR_DEFAULT_WIDTH;
  }

  return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, value));
}

/** Height of a resized sidebar section, between its minimum and `max` (what leaves room for the notes tree). */
export function clampSectionHeight(value: number, max: number): number {
  return Math.round(Math.max(SECTION_MIN_HEIGHT, Math.min(max, value)));
}

/** Share of a row's height, at its top and bottom, where a drop means "next to this row" rather than "into it". */
export const DROP_EDGE_RATIO = 0.25;

/**
 * Drop on a folder row. A folder dragged on the edge of a row is placed beside it, in the middle it goes inside.
 * The bottom edge of an open folder is not "after" it: the next row visually is its first child.
 * `ratio` is the pointer position in the row, 0 (top) to 1 (bottom).
 */
export function resolveFolderRowDrop(
  dragKind: 'note' | 'folder',
  folder: Folder,
  hasVisibleContent: boolean,
  ratio: number,
): DropPlacement {
  if (dragKind === 'folder') {
    // Nothing can be placed above the default folder: it always stays first
    if (ratio < DROP_EDGE_RATIO && !folder.isDefault) {
      return { kind: 'between', parentId: folder.parentId, anchorId: folder.id, side: 'before' };
    }

    if (ratio > 1 - DROP_EDGE_RATIO && !hasVisibleContent) {
      return { kind: 'between', parentId: folder.parentId, anchorId: folder.id, side: 'after' };
    }
  }

  return { kind: 'into', folderId: folder.id };
}

/**
 * Drop on a note row. A note goes before or after the hovered note (the upper or lower half), a folder goes
 * into the folder holding that note (folders are listed above the notes, so there is no position among them).
 */
export function resolveNoteRowDrop(
  dragKind: 'note' | 'folder',
  note: TreeNote,
  holdingFolderId: number | null,
  ratio: number,
): DropPlacement {
  if (dragKind === 'folder') {
    return { kind: 'into', folderId: holdingFolderId };
  }

  return { kind: 'between', parentId: holdingFolderId, anchorId: note.id, side: ratio < 0.5 ? 'before' : 'after' };
}

/**
 * Sibling ids once `movedId` is inserted before/after `anchorId`. `minIndex` keeps a pinned first sibling
 * (the default folder) in front. An unknown anchor appends at the end.
 */
export function insertSibling(
  siblingIds: number[],
  movedId: number,
  anchorId: number,
  side: 'before' | 'after',
  minIndex = 0,
): number[] {
  const rest = siblingIds.filter(id => id !== movedId);
  const anchorIndex = rest.indexOf(anchorId);
  const wanted = anchorIndex === -1 ? rest.length : anchorIndex + (side === 'after' ? 1 : 0);
  const index = Math.max(wanted, Math.min(minIndex, rest.length));

  rest.splice(index, 0, movedId);

  return rest;
}
