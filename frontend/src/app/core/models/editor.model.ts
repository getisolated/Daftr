import { Note } from './note.model';

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

/** A replacement in a text: `[from, to)` of the old text becomes `insert` (UTF-16 offsets, as CodeMirror counts). */
export interface TextChange {
  from: number;
  to: number;
  insert: string;
}

/** The open note was deleted on another device: shown read-only until restored. */
export interface RemoteDeletion {
  /** The note as it was last shown, to show it again when its tab comes back. */
  note: Note;
  /** Name of the device it was deleted on, `null` when unknown. */
  deviceName: string | null;
}
