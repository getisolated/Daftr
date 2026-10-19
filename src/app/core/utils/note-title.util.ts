import { Translator } from '../i18n/translate';

/** Stored as the title of the notes without one, before it became empty. */
const LEGACY_UNTITLED = 'Untitled';
const HTML_COMMENT = /<!--[\s\S]*?(?:-->|$)/g;
const HEADING_MARKS = /^#+\s*/;

/**
 * Title of a note: its first line of real content, ignoring blank lines and HTML comments (draft markers, metadata).
 * Empty when there is none: it is stored as is, and shown as `noteDisplayTitle` in the current language.
 */
export function extractNoteTitle(content: string): string {
  const visible = content.replace(HTML_COMMENT, '');
  const firstLine = visible.split('\n').find(line => line.trim() !== '') ?? '';
  return firstLine.replace(HEADING_MARKS, '').trim();
}

/** Title to show for a note: the placeholder of the current language when it has none (or kept the legacy English one). */
export function noteDisplayTitle(title: string | null | undefined, t: Translator): string {
  return title && title !== LEGACY_UNTITLED ? title : t('common.untitled');
}
