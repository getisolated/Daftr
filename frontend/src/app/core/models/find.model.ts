/** What the find panel searches for, and replaces with. */
export interface FindOptions {
  search: string;
  replace: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  regexp: boolean;
}

/** What the preview shows of the search: the query, and the current match to bring into view; a new object for each request. */
export interface PreviewFind {
  options: FindOptions;
  /** Source line (1-based) of each match of the editor, in order. */
  lines: readonly number[];
  /** Index of the current match in `lines`, -1 when none. */
  current: number;
  /** Whether the preview scrolls to the current match (it is alone on screen: the editor cannot show it). */
  reveal: boolean;
}
