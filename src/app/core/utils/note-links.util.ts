import { Translator } from '../i18n/translate';
import { noteDisplayTitle } from './note-title.util';
import { LinkResolver, needsAngleBrackets, normalizeLinkTarget, pathKey } from './note-paths.util';

/** Minimal shape of a note needed to analyse its links (a `Note` satisfies it). */
export interface LinkableNote {
  id: number;
  title: string;
  content: string;
  folderId: number | null;
}

export interface Backlink {
  noteId: number;
  title: string;
  /** Text surrounding the first link to the target note. */
  excerpt: string;
}

export interface GraphNode {
  id: number;
  title: string;
  /** Number of links (in or out) touching the node. */
  degree: number;
  /** Target of a link whose note does not exist (anymore). */
  missing: boolean;
}

export interface GraphLink {
  source: number;
  target: number;
}

export interface NoteGraph {
  nodes: GraphNode[];
  links: GraphLink[];
}

export interface GraphOptions {
  /** Keeps the notes of this folder only; `null` keeps everything. */
  folderId?: number | null;
  /** Note around which a large graph is cut. */
  focusId?: number | null;
  /** Notes without any link stay out of the graph. */
  hideOrphans?: boolean;
  /** Resolves `[label](path)` links to notes; without it only `note://id` links count. */
  resolve?: LinkResolver;
}

/** Above this many notes the graph is limited to what stays readable and fast. */
export const GRAPH_LARGE_THRESHOLD = 500;
/** Link distance kept around the focused note on a large graph. */
export const GRAPH_FOCUS_DEPTH = 2;

const EXCERPT_RADIUS = 60;
const NOTE_LINK = /note:\/\/(?<id>\d+)/g;
/** `[label](target)` and `[label](<target with spaces>)`, an optional title after the target; images are left out. */
const MARKDOWN_LINK = /(?<!!)\[[^\]\n]*\]\((?<target><[^>\n]*>|[^)\s\n]*)(?:\s+"[^"\n]*")?\)/g;

interface LinkOccurrence {
  noteId: number;
  /** Position of the link in the content. */
  index: number;
  /** Length of the text standing for the link. */
  length: number;
}

/** Every link to a note found in `content`, in order of appearance: `note://id`, and `[label](path)` when `resolve` knows the path. */
function findLinkOccurrences(content: string, resolve?: LinkResolver): LinkOccurrence[] {
  const occurrences: LinkOccurrence[] = [];

  for (const match of content.matchAll(NOTE_LINK)) {
    occurrences.push({ noteId: Number(match.groups?.['id']), index: match.index, length: match[0].length });
  }

  if (resolve) {
    for (const match of content.matchAll(MARKDOWN_LINK)) {
      const noteId = resolve(match.groups?.['target'] ?? '');

      if (noteId !== null) {
        occurrences.push({ noteId, index: match.index, length: match[0].length });
      }
    }
  }

  return occurrences.sort((a, b) => a.index - b.index);
}

/**
 * `content` with every `[label](path)` link to `fromPath` pointed at `toPath`; `null` when none was found.
 * The label, the `./` prefix and the `.md` suffix of each link are kept.
 */
export function retargetNoteLinks(content: string, fromPath: string, toPath: string): string | null {
  const fromKey = pathKey(fromPath);
  let changed = false;

  const rewritten = content.replace(MARKDOWN_LINK, (link: string, target: string) => {
    const normalized = normalizeLinkTarget(target);

    if (normalized === null || pathKey(normalized) !== fromKey) {
      return link;
    }

    changed = true;

    const prefix = /^<?\.\//.test(target) ? './' : '';
    const suffix = /\.md>?$/i.test(target) ? '.md' : '';
    const path = `${prefix}${toPath}${suffix}`;
    const newTarget = needsAngleBrackets(path) ? `<${path}>` : path;

    return link.replace(target, () => newTarget);
  });

  return changed ? rewritten : null;
}

/** Ids of the notes `content` links to, in order of appearance and without duplicates. */
export function extractNoteLinks(content: string, resolve?: LinkResolver): number[] {
  return [...new Set(findLinkOccurrences(content, resolve).map(o => o.noteId))];
}

/** Text around the first link to `targetId` in `content`, on its line, with an ellipsis where it was cut. */
export function extractLinkExcerpt(content: string, targetId: number, resolve?: LinkResolver): string {
  const occurrence = findLinkOccurrences(content, resolve).find(o => o.noteId === targetId);

  if (!occurrence) {
    return '';
  }

  const { index, length } = occurrence;
  const lineStart = content.lastIndexOf('\n', index) + 1;
  const lineEndRaw = content.indexOf('\n', index);
  const lineEnd = lineEndRaw === -1 ? content.length : lineEndRaw;
  const from = Math.max(lineStart, index - EXCERPT_RADIUS);
  const to = Math.min(lineEnd, index + length + EXCERPT_RADIUS);
  const prefix = from > lineStart ? '…' : '';
  const suffix = to < lineEnd ? '…' : '';

  return `${prefix}${content.slice(from, to).trim()}${suffix}`;
}

/** Notes linking to `targetId`, sorted by title. A note never backlinks itself. */
export function findBacklinks(notes: readonly LinkableNote[], targetId: number, t: Translator, resolve?: LinkResolver): Backlink[] {
  const backlinks: Backlink[] = [];

  for (const note of notes) {
    if (note.id === targetId || !extractNoteLinks(note.content, resolve).includes(targetId)) {
      continue;
    }

    backlinks.push({ noteId: note.id, title: noteDisplayTitle(note.title, t), excerpt: extractLinkExcerpt(note.content, targetId, resolve) });
  }

  return backlinks.sort((a, b) => a.title.localeCompare(b.title));
}

/** Notes reachable from `focusId` in at most `depth` links, whatever the direction of the links. */
function collectNeighbourhood(links: readonly GraphLink[], focusId: number, depth: number): Set<number> {
  const adjacency = new Map<number, number[]>();

  for (const { source, target } of links) {
    adjacency.set(source, [...(adjacency.get(source) ?? []), target]);
    adjacency.set(target, [...(adjacency.get(target) ?? []), source]);
  }

  const reached = new Set<number>([focusId]);
  let frontier = [focusId];

  for (let level = 0; level < depth && frontier.length > 0; level++) {
    const next: number[] = [];

    for (const id of frontier) {
      for (const neighbour of adjacency.get(id) ?? []) {
        if (!reached.has(neighbour)) {
          reached.add(neighbour);
          next.push(neighbour);
        }
      }
    }

    frontier = next;
  }

  return reached;
}

/**
 * Graph of the links between notes. A link to a note that does not exist gives a `missing` node.
 * Two notes linking each other keep one link per direction (A→B and B→A), so a cycle stays visible.
 */
export function buildNoteGraph(notes: readonly LinkableNote[], t: Translator, options: GraphOptions = {}): NoteGraph {
  const { folderId = null, focusId = null, hideOrphans = false, resolve } = options;
  const scoped = folderId === null ? notes : notes.filter(n => n.folderId === folderId);
  const known = new Map(scoped.map(n => [n.id, n]));
  const existing = new Set(notes.map(n => n.id));
  const links: GraphLink[] = [];
  const missing = new Set<number>();

  for (const note of scoped) {
    for (const targetId of extractNoteLinks(note.content, resolve)) {
      if (targetId === note.id) {
        continue;
      }

      if (known.has(targetId)) {
        links.push({ source: note.id, target: targetId });
      } else if (!existing.has(targetId)) {
        missing.add(targetId);
        links.push({ source: note.id, target: targetId });
      }
    }
  }

  const degrees = new Map<number, number>();

  for (const { source, target } of links) {
    degrees.set(source, (degrees.get(source) ?? 0) + 1);
    degrees.set(target, (degrees.get(target) ?? 0) + 1);
  }

  const isLarge = scoped.length > GRAPH_LARGE_THRESHOLD;
  const neighbourhood = isLarge && focusId !== null ? collectNeighbourhood(links, focusId, GRAPH_FOCUS_DEPTH) : null;
  // Without a focus, a large graph keeps what is connected: hundreds of floating orphans tell nothing
  const dropOrphans = hideOrphans || (isLarge && neighbourhood === null);
  const nodes: GraphNode[] = [];

  for (const [id, note] of known) {
    const degree = degrees.get(id) ?? 0;

    if ((dropOrphans && degree === 0) || (neighbourhood && !neighbourhood.has(id))) {
      continue;
    }

    nodes.push({ id, title: noteDisplayTitle(note.title, t), degree, missing: false });
  }

  for (const id of missing) {
    if (!neighbourhood || neighbourhood.has(id)) {
      nodes.push({ id, title: t('editor.graph.missingNote', { id }), degree: degrees.get(id) ?? 0, missing: true });
    }
  }

  const kept = new Set(nodes.map(n => n.id));

  return { nodes, links: links.filter(l => kept.has(l.source) && kept.has(l.target)) };
}
