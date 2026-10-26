import { CollectionFileRef, CollectionNode } from '../models/collection.model';
import { extractNoteTitle } from './note-title.util';

/** A file or folder of a collection picked for "Copy To…" (a whole folder collection is a folder of path `''`). */
export interface CollectionCopySource {
  root: string;
  node: CollectionNode;
}

/** Folder to create in the notes; `parent` is the index of its parent folder in the plan, `null` for the target. */
export interface CopyPlanFolder {
  name: string;
  parent: number | null;
}

/** Markdown file to copy as a note into a planned folder (`null`: the target folder itself). */
export interface CopyPlanFile {
  ref: CollectionFileRef;
  name: string;
  folder: number | null;
}

/** What "Copy To…" creates: the folders first (parents before children), then the notes. */
export interface CollectionCopyPlan {
  folders: CopyPlanFolder[];
  files: CopyPlanFile[];
}

function isInside(path: string, folder: string): boolean {
  return folder === '' ? path !== '' : path.startsWith(`${folder}/`);
}

/** The sources a copy really applies to: what sits inside another selected folder is copied with it, once. */
export function topLevelSources(sources: readonly CollectionCopySource[]): CollectionCopySource[] {
  const seen = new Set<string>();

  return sources.filter(source => {
    const key = `${source.root}\u0000${source.node.path}`;
    const covered = sources.some(other =>
      other !== source && other.root === source.root && other.node.kind === 'folder' && isInside(source.node.path, other.node.path));

    if (covered || seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

/** Folders and files to create so the copied notes keep the folder structure of the collection. */
export function planCollectionCopy(sources: readonly CollectionCopySource[]): CollectionCopyPlan {
  const plan: CollectionCopyPlan = { folders: [], files: [] };

  const visit = (root: string, node: CollectionNode, parent: number | null): void => {
    if (node.kind === 'file') {
      plan.files.push({ ref: { root, path: node.path }, name: node.name, folder: parent });
      return;
    }

    plan.folders.push({ name: node.name, parent });
    const index = plan.folders.length - 1;
    (node.children ?? []).forEach(child => visit(root, child, index));
  };

  topLevelSources(sources).forEach(source => visit(source.root, source.node, null));
  return plan;
}

/** Title of a copied note: its first line, like any note; the file name (without extension) when the file is empty. */
export function copiedNoteTitle(content: string, fileName: string): string {
  return extractNoteTitle(content) || fileName.replace(/\.[^.]+$/, '') || fileName;
}
