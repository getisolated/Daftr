import { PathFolder } from './note-paths.util';

/**
 * Path of a note above the editor: the notes root (its label), the folders from the top down, then the note.
 * A cycle in the folders cuts the walk short.
 */
export function noteBreadcrumbs(rootLabel: string, folderId: number | null, folders: readonly PathFolder[], title: string): string[] {
  const byId = new Map(folders.map(folder => [folder.id, folder]));
  const names: string[] = [];
  const seen = new Set<number>();

  let current = folderId === null ? undefined : byId.get(folderId);

  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    names.unshift(current.name);
    current = current.parentId === null ? undefined : byId.get(current.parentId);
  }

  return [rootLabel, ...names, title];
}

/** Path of a collection file above the editor: the collection (its label), the folders, then the file. */
export function fileBreadcrumbs(collectionLabel: string, path: string): string[] {
  return [collectionLabel, ...path.split('/').filter(Boolean)];
}
