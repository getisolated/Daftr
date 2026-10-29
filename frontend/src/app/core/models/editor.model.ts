/** How a note is shown: its markdown source, its rendering (read-only), or both side by side. */
export type EditorViewMode = 'source' | 'preview' | 'split';

/** Asks the preview to bring a source line (1-based, may be fractional) to its top; a new object for each request. */
export interface PreviewScrollRequest {
  line: number;
}

/** Where a tab was left, restored when it is shown again: the editor is rebuilt for each note. */
export interface NoteViewState {
  anchor: number;
  head: number;
  /** Line at the top of the editor and how far it was scrolled past (px); null when the editor was hidden (preview only). */
  editorTop: { pos: number; offset: number } | null;
  /** Scroll of the preview shown alone; null otherwise (side by side, it follows the editor). */
  previewTop: number | null;
}
