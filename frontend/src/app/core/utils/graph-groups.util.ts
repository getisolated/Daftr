/** Beyond this many groups, colours would repeat and tell nothing: a shallower level of folders is used. */
export const MAX_GRAPH_GROUPS = 10;
const MAX_GROUP_DEPTH = 4;

/** First `depth` segments of a folder path (`a/b/c`), `null` at the root. */
function prefix(folder: string, depth: number): string | null {
  const segments = folder.split('/').filter(Boolean);
  return segments.length === 0 ? null : segments.slice(0, depth).join('/');
}

/**
 * Colour group of each item of the graph, from the folder holding it (`a/b`, '' at the root): the deepest level of
 * folders that still gives at most {@link MAX_GRAPH_GROUPS} groups. A collection whose files nearly all sit in one
 * folder (`doc/…`) is thus split by the sub-folders. Items at the root have no group (`null`).
 */
export function graphGroups(folders: readonly string[]): (string | null)[] {
  let chosen = 1;

  for (let depth = 2; depth <= MAX_GROUP_DEPTH; depth++) {
    const groups = new Set(folders.map(folder => prefix(folder, depth)).filter(group => group !== null));
    if (groups.size > MAX_GRAPH_GROUPS) {
      break;
    }
    chosen = depth;
  }

  return folders.map(folder => prefix(folder, chosen));
}

/** Categorical colours of the groups, readable on the light and the dark themes. */
export const GRAPH_GROUP_COLORS: readonly string[] = [
  '#4e79a7', '#f28e2b', '#e15759', '#76b7b2', '#59a14f', '#edc948', '#b07aa1', '#ff9da7', '#9c755f', '#8cd17d',
];
