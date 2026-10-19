import { Collection, CollectionFileRef, CollectionNode } from '../models/collection.model';
import { LinkStatus } from './note-paths.util';

/** Last segment of a path, whatever its separators. */
export function baseName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

/** Parent folder of a path, whatever its separators ('' at the top). */
export function parentPath(path: string): string {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return index === -1 ? '' : path.slice(0, index);
}

/**
 * Labels of the collections: their display name, plus a muted hint when several share it (two `docs` folders of two
 * repositories): the name of their parent folder, or their whole parent path when even that is shared.
 */
export function labelCollections(collections: readonly Collection[]): { label: string; hint: string | null }[] {
  const byName = new Map<string, Collection[]>();

  for (const collection of collections) {
    const key = collection.displayName.toLowerCase();
    byName.set(key, [...(byName.get(key) ?? []), collection]);
  }

  return collections.map(collection => {
    const homonyms = byName.get(collection.displayName.toLowerCase()) ?? [];

    if (homonyms.length < 2) {
      return { label: collection.displayName, hint: null };
    }

    const parent = baseName(parentPath(collection.path));
    const parentShared = homonyms.filter(other => baseName(parentPath(other.path)).toLowerCase() === parent.toLowerCase()).length > 1;
    return { label: collection.displayName, hint: parentShared ? parentPath(collection.path) : parent };
  });
}

/** Where a path ends up once `from` (a file or a folder) became `to`; `null` when it is not concerned. */
export function movedPath(path: string, from: string, to: string): string | null {
  if (path === from) {
    return to;
  }
  return path.startsWith(`${from}/`) ? `${to}${path.slice(from.length)}` : null;
}

/** Every file of a tree, depth first, as the palette lists them. */
export function flattenFiles(root: string, nodes: readonly CollectionNode[]): CollectionFileRef[] {
  const files: CollectionFileRef[] = [];
  const pending = [...nodes].reverse();

  while (pending.length > 0) {
    const node = pending.pop() as CollectionNode;

    if (node.kind === 'file') {
      files.push({ root, path: node.path });
    } else {
      pending.push(...[...(node.children ?? [])].reverse());
    }
  }

  return files;
}

/**
 * Target of a relative markdown link written in a collection file, as a path of the same collection; `null` for what
 * is not a local markdown file (web address, anchor) or leaves the collection.
 */
export function resolveRelativeLink(fromFile: string, target: string): CollectionTarget | null {
  const resolved = resolveRelativePath(fromFile, target);

  if (resolved === null) {
    return null;
  }
  // A link to a folder (`../`, `./guide/`): its index page, as a web server serves index.html (see folderPage)
  if (pointsToFolder(cleanTarget(target))) {
    return { kind: 'folder', path: resolved };
  }
  if (/\.md$/i.test(resolved)) {
    return { kind: 'file', path: resolved };
  }

  // A file of another extension is not a markdown page of the collection
  return /\.[^./]+$/.test(baseName(resolved)) ? null : { kind: 'file', path: `${resolved}.md` };
}

/** What a link of a collection file designates: a markdown file, or a folder (`''` for the root). */
export type CollectionTarget = { kind: 'file' | 'folder'; path: string };

/** The target without its angle brackets and anchor, percent-decoded when it is; `~` alone is the root folder. */
function cleanTarget(target: string): string {
  const cleaned = target.trim().replace(/^<|>$/g, '').split('#')[0] ?? '';
  let decoded = cleaned;

  try {
    decoded = decodeURIComponent(cleaned);
  } catch {
    // Not percent-encoded: kept as written
  }
  return decoded === '~' ? '~/' : decoded;
}

function pointsToFolder(cleaned: string): boolean {
  const last = cleaned.split('/').pop() ?? '';
  return cleaned.endsWith('/') || last === '.' || last === '..';
}

/**
 * Path inside the collection of a relative target (link, image) of `fromFile`, as written: `''` for the root of the
 * collection, `null` for web addresses, anchors alone and paths leaving the collection. A target starting with `/` or
 * `~/` is relative to the root of the collection (`~/` as in DocFX).
 */
export function resolveRelativePath(fromFile: string, target: string): string | null {
  const cleaned = cleanTarget(target);

  if (!cleaned || /^[a-z][a-z0-9+.-]*:/i.test(cleaned)) {
    return null;
  }

  const fromRoot = cleaned.startsWith('/') || cleaned.startsWith('~/');
  const segments = fromRoot ? [] : parentPath(fromFile).split('/').filter(Boolean);

  for (const segment of (cleaned.startsWith('~/') ? cleaned.slice(2) : cleaned).split('/')) {
    if (segment === '..') {
      if (segments.length === 0) {
        return null;
      }
      segments.pop();
    } else if (segment !== '.' && segment !== '') {
      segments.push(segment);
    }
  }

  return segments.join('/');
}

/** Files and folders of a loaded collection, by lower-case path (the disk is case-insensitive), to their real path. */
export interface CollectionIndex {
  files: ReadonlyMap<string, string>;
  folders: ReadonlyMap<string, string>;
}

/** Index of a collection tree, for {@link collectionLinkStatus} and {@link folderPage}; the root folder is `''`. */
export function indexCollection(nodes: readonly CollectionNode[]): CollectionIndex {
  const files = new Map<string, string>();
  const folders = new Map<string, string>([['', '']]);
  const pending = [...nodes];

  for (let node = pending.pop(); node; node = pending.pop()) {
    (node.kind === 'folder' ? folders : files).set(node.path.toLowerCase(), node.path);
    pending.push(...(node.children ?? []));
  }
  return { files, folders };
}

/** The page a link to `folder` opens: its index.md, else its README.md; `null` when it has neither. */
export function folderPage(index: CollectionIndex, folder: string): string | null {
  const inFolder = (name: string): string => (folder ? `${folder}/${name}` : name).toLowerCase();
  return index.files.get(inFolder('index.md')) ?? index.files.get(inFolder('readme.md')) ?? null;
}

/**
 * Status of a link written in a collection file: a relative (or extension-less) target is resolved from that file,
 * inside its collection. Only a markdown page or a folder missing from the loaded tree is `broken`; files of another
 * extension (images, sources) are not listed in the tree and cannot be checked, nor can anything before it is loaded.
 */
export function collectionLinkStatus(fromFile: string, rawTarget: string, index: CollectionIndex | null): LinkStatus {
  const target = rawTarget.trim();

  if (/^[a-z][a-z0-9+.-]*:/i.test(target)) {
    return 'external';
  }
  if (target.startsWith('#')) {
    return 'anchor';
  }

  const resolved = resolveRelativeLink(fromFile, target);

  if (resolved === null || index === null) {
    return 'note';
  }
  // A folder is a valid target as soon as it exists, with or without an index page
  const known = resolved.kind === 'folder' ? index.folders : index.files;
  return known.has(resolved.path.toLowerCase()) ? 'note' : 'broken';
}
