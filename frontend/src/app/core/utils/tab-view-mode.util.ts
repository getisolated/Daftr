import { EditorViewMode } from '../models/editor.model';

const VIEW_MODES: readonly EditorViewMode[] = ['source', 'preview', 'split'];

/** Whether a value read back from the stored session is a view mode. */
export function isEditorViewMode(value: unknown): value is EditorViewMode {
  return VIEW_MODES.includes(value as EditorViewMode);
}

/**
 * View mode of a note being shown: a tab already shown keeps its own mode, a newly opened one takes the mode
 * of the last note shown (even with the home page shown since), and the settings decide when no note was shown yet.
 */
export function viewModeOnOpen(tabMode: EditorViewMode | undefined, leftMode: EditorViewMode | null, openInPreview: boolean): EditorViewMode {
  return tabMode ?? leftMode ?? (openInPreview ? 'preview' : 'source');
}
