/** Folder as far as paths are concerned (a `Folder` satisfies it). */
export interface PathFolder {
  id: number;
  name: string;
  parentId: number | null;
}

/** Note as far as paths are concerned (a `Note` satisfies it). */
export interface PathNote {
  id: number;
  /** Stable id shared by every device, written in the `note://` links; unset for what is no stored note. */
  uuid?: string;
  title: string;
  folderId: number | null;
}

/** Note as far as `note://` links are concerned (a `Note` satisfies it); one without uuid is reached by its local id only. */
export interface NoteUrlTarget {
  id: number;
  uuid?: string;
}

/** Note a `note://` link designates: its uuid (lowercase), or the local id an older version wrote. */
export type NoteUrl = { kind: 'uuid'; uuid: string } | { kind: 'id'; id: number };

/** Note designated by a `note://` link, `null` when there is none. */
export type NoteUrlResolver<T extends NoteUrlTarget> = (url: NoteUrl) => T | null;

/** Id of the note a markdown link target designates, `null` when it designates none. */
export type LinkResolver = (target: string) => number | null;

export interface PathCompletion {
  kind: 'folder' | 'note';
  /** What the list shows: the folder (with a trailing slash) or the note title. */
  label: string;
  /** Whole path of the entry, from the root; a folder ends with a slash. */
  path: string;
  /** Folder holding a note found out of the typed directory. */
  detail?: string;
  /** Uuid of a note: the link is then written `note://<uuid>`, which survives renames, moves and other devices. */
  uuid?: string;
}

const EXTERNAL_SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const UNSAFE_IN_SEGMENT = /[\\/<>]/g;
const NEEDS_ANGLE_BRACKETS = /[\s()]/;

/** What a link target points to; a local target (note path, `note://`) that points to nothing is `broken`. */
export type LinkStatus = 'note' | 'external' | 'anchor' | 'broken';

/**
 * Target of a `note://` link: a uuid, tried first so that one made of digits only is not cut to its first group (its hex
 * digits in any case), else the local id an older version wrote (`note://12`), still followed.
 */
const NOTE_URL_TARGET = String.raw`(?:(?<uuid>[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})|(?<id>\d+))`;
/** Source of a regex matching a `note://` link within a text, with the groups read by {@link noteUrlFromMatch}. */
export const NOTE_URL_SOURCE = `note://${NOTE_URL_TARGET}`;
const NOTE_URL = new RegExp(`^${NOTE_URL_SOURCE}$`);

/** Note designated by a match of {@link NOTE_URL_SOURCE}; the uuid is lowercased, the form it is indexed under. */
export function noteUrlFromMatch(match: RegExpMatchArray | null): NoteUrl | null {
  const uuid = match?.groups?.['uuid'];
  const id = match?.groups?.['id'];

  if (uuid !== undefined) {
    return { kind: 'uuid', uuid: uuid.toLowerCase() };
  }

  return id === undefined ? null : { kind: 'id', id: Number(id) };
}

/** Note designated by a whole link target (`note://<uuid>` or `note://<id>`), `null` for any other target. */
export function parseNoteUrl(target: string): NoteUrl | null {
  return noteUrlFromMatch(NOTE_URL.exec(target.trim()));
}

/** Link to write towards the note of `uuid`. */
export function formatNoteUrl(uuid: string): string {
  return `note://${uuid.toLowerCase()}`;
}

/** Finds the notes of `note://` links: by uuid (whatever its case) through a map, or by local id for the older links. */
export function createNoteUrlResolver<T extends NoteUrlTarget>(notes: readonly T[]): NoteUrlResolver<T> {
  const byUuid = new Map<string, T>();
  const byId = new Map<number, T>();

  for (const note of notes) {
    byId.set(note.id, note);

    if (note.uuid) {
      byUuid.set(note.uuid.toLowerCase(), note);
    }
  }

  return url => (url.kind === 'uuid' ? byUuid.get(url.uuid) : byId.get(url.id)) ?? null;
}

/** Status of a link target, `noteExists` telling whether the note of a `note://` link is still there. */
export function linkStatus(rawTarget: string, resolve: LinkResolver, noteExists: (url: NoteUrl) => boolean): LinkStatus {
  const target = rawTarget.trim();
  const noteUrl = parseNoteUrl(target);

  if (noteUrl !== null) {
    return noteExists(noteUrl) ? 'note' : 'broken';
  }

  if (EXTERNAL_SCHEME.test(target)) {
    return 'external';
  }

  if (target.startsWith('#')) {
    return 'anchor';
  }

  return resolve(target) === null ? 'broken' : 'note';
}

/**
 * Segment of a path: slashes and angle brackets would break the path or the link, so they become dashes.
 * The segment of an untitled note is written in the links: it stays `Untitled` whatever the language, or they would break.
 */
export function toPathSegment(name: string): string {
  return name.replace(UNSAFE_IN_SEGMENT, '-').trim() || 'Untitled';
}

/** A markdown link target holding spaces or parentheses must be written between angle brackets. */
export function needsAngleBrackets(path: string): boolean {
  return NEEDS_ANGLE_BRACKETS.test(path);
}

function indexFolders(folders: readonly PathFolder[]): Map<number, PathFolder> {
  return new Map(folders.map(f => [f.id, f]));
}

/** Names of the folders from the root down to `folderId` (a cycle in the data cuts the walk short). */
function folderSegments(folderId: number | null, byId: Map<number, PathFolder>): string[] {
  const segments: string[] = [];
  const seen = new Set<number>();
  let current = folderId === null ? undefined : byId.get(folderId);

  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    segments.unshift(toPathSegment(current.name));
    current = current.parentId === null ? undefined : byId.get(current.parentId);
  }

  return segments;
}

/** Path of a note: its folders then its title, joined by slashes. */
export function notePath(note: PathNote, folders: readonly PathFolder[]): string {
  return [...folderSegments(note.folderId, indexFolders(folders)), toPathSegment(note.title)].join('/');
}

/** Form of a path under which two paths designate the same note (case and surrounding spaces ignored). */
export function pathKey(path: string): string {
  return path.split('/').map(s => s.trim().toLowerCase()).filter(Boolean).join('/');
}

/**
 * Relative target of a markdown link, ready to be compared with note paths.
 * `null` for what is not a path to a note: external links, anchors, `note://` (handled on its own).
 */
export function normalizeLinkTarget(raw: string): string | null {
  let target = raw.trim();

  if (target.startsWith('<') && target.endsWith('>')) {
    target = target.slice(1, -1);
  }

  if (target === '' || target.startsWith('#') || EXTERNAL_SCHEME.test(target)) {
    return null;
  }

  try {
    target = decodeURIComponent(target);
  } catch {
    // Not percent-encoded after all (a title holding a lone %): keep it as typed
  }

  return target.replace(/^\.?\//, '').replace(/\.md$/i, '');
}

/** Resolves link targets against the paths of the notes, ignoring case; the first note wins a duplicated path. */
export function createLinkResolver(notes: readonly PathNote[], folders: readonly PathFolder[]): LinkResolver {
  const byId = indexFolders(folders);
  const idsByPath = new Map<string, number>();

  for (const note of notes) {
    const key = pathKey([...folderSegments(note.folderId, byId), toPathSegment(note.title)].join('/'));

    if (!idsByPath.has(key)) {
      idsByPath.set(key, note.id);
    }
  }

  return target => {
    const normalized = normalizeLinkTarget(target);
    return normalized === null ? null : idsByPath.get(pathKey(normalized)) ?? null;
  };
}

/**
 * What the autocompletion offers for the path typed so far, like a file explorer: the folders and notes inside the
 * typed directory that start with the last segment. Out of any directory, notes elsewhere whose title contains it are offered too.
 */
export function listPathCompletions(typed: string, notes: readonly PathNote[], folders: readonly PathFolder[]): PathCompletion[] {
  const normalized = typed.replace(/^\.?\//, '');
  const lastSlash = normalized.lastIndexOf('/');
  const directory = normalized.slice(0, lastSlash + 1).split('/').filter(Boolean);
  const filter = normalized.slice(lastSlash + 1).trim().toLowerCase();
  const byId = indexFolders(folders);
  const canonical: string[] = [];
  let folderId: number | null = null;

  for (const segment of directory) {
    const found: PathFolder | undefined = folders.find(
      f => f.parentId === folderId && toPathSegment(f.name).toLowerCase() === segment.trim().toLowerCase(),
    );

    if (!found) {
      return [];
    }

    canonical.push(toPathSegment(found.name));
    folderId = found.id;
  }

  const base = canonical.map(s => `${s}/`).join('');
  const completions: PathCompletion[] = [];

  const subFolders = folders
    .filter(f => f.parentId === folderId && toPathSegment(f.name).toLowerCase().startsWith(filter))
    .sort((a, b) => a.name.localeCompare(b.name));

  for (const folder of subFolders) {
    const segment = toPathSegment(folder.name);
    completions.push({ kind: 'folder', label: `${segment}/`, path: `${base}${segment}/` });
  }

  const inDirectory = notes
    .filter(n => n.folderId === folderId && toPathSegment(n.title).toLowerCase().startsWith(filter))
    .sort((a, b) => a.title.localeCompare(b.title));

  for (const note of inDirectory) {
    const segment = toPathSegment(note.title);
    completions.push({ kind: 'note', label: segment, path: `${base}${segment}`, uuid: note.uuid });
  }

  if (directory.length === 0 && filter !== '') {
    const elsewhere = notes
      .filter(n => n.folderId !== null && toPathSegment(n.title).toLowerCase().includes(filter))
      .sort((a, b) => a.title.localeCompare(b.title));

    for (const note of elsewhere) {
      completions.push({
        kind: 'note',
        label: toPathSegment(note.title),
        path: notePath(note, folders),
        detail: folderSegments(note.folderId, byId).join('/'),
        uuid: note.uuid,
      });
    }
  }

  return completions;
}
