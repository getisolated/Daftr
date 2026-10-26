import { CollectionFileRef, CollectionNode } from '../models/collection.model';
import { resolveRelativeLink } from './collection.util';
import { MARKDOWN_LINK } from './note-links.util';
import { PathFolder, needsAngleBrackets, notePath } from './note-paths.util';
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

/** A copied file once its note is placed: `folderId` is the folder that receives it. */
export interface CopiedNote {
  ref: CollectionFileRef;
  title: string;
  folderId: number | null;
}

/** Path of the note of each copied file, by {@link copiedNoteKey}. */
export type CopiedNotePaths = ReadonlyMap<string, string>;

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

/** Title of a copied note: the file name without its extension. */
export function copiedNoteTitle(fileName: string): string {
  return fileName.replace(/\.[^.]+$/, '') || fileName;
}

/** Content of a copied note: the title as first line (unless the file already starts with it), then the file. */
export function copiedNoteContent(content: string, title: string): string {
  return extractNoteTitle(content) === title ? content : `# ${title}\n\n${content}`;
}

/** Key of a collection file in {@link CopiedNotePaths}: the disk is case-insensitive, so are the links. */
function copiedNoteKey(root: string, path: string): string {
  return `${root}\u0000${path.toLowerCase()}`;
}

/** Note path (as written in the links between notes) of each copied file. */
export function copiedNotePaths(notes: readonly CopiedNote[], folders: readonly PathFolder[]): CopiedNotePaths {
  return new Map(notes.map(note => [copiedNoteKey(note.ref.root, note.ref.path), notePath({ id: 0, ...note }, folders)]));
}

/** Note path a link of the copied file `from` leads to; `null` when its target was not copied (or is not a page). */
function copiedLinkTarget(from: CollectionFileRef, target: string, paths: CopiedNotePaths): string | null {
  const resolved = resolveRelativeLink(from.path, target);

  if (resolved === null) {
    return null;
  }
  if (resolved.kind === 'file') {
    return paths.get(copiedNoteKey(from.root, resolved.path)) ?? null;
  }

  // A link to a folder opens its index page, as in the collection (see folderPage)
  const prefix = resolved.path ? `${resolved.path}/` : '';
  return paths.get(copiedNoteKey(from.root, `${prefix}index.md`)) ?? paths.get(copiedNoteKey(from.root, `${prefix}readme.md`)) ?? null;
}

/**
 * `content` of the copied file `from` with its relative links to the other copied files pointed at their notes: links
 * between notes are paths from the root of the notes, so the relative ones of the disk would break. An anchor is dropped
 * (note links have none); a link to a file left out of the copy is kept as written.
 */
export function retargetCopiedLinks(content: string, from: CollectionFileRef, paths: CopiedNotePaths): string {
  return content.replace(MARKDOWN_LINK, (link: string, target: string) => {
    const path = copiedLinkTarget(from, target, paths);

    if (path === null) {
      return link;
    }

    const newTarget = needsAngleBrackets(path) ? `<${path}>` : path;
    // The target follows the label, which cannot hold a `]`
    const at = link.indexOf(target, link.indexOf('](') + 2);
    return `${link.slice(0, at)}${newTarget}${link.slice(at + target.length)}`;
  });
}
