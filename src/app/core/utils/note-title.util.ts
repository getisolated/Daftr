const HTML_COMMENT = /<!--[\s\S]*?(?:-->|$)/g;
const HEADING_MARKS = /^#+\s*/;

/** Title of a note: its first line of real content, ignoring blank lines and HTML comments (draft markers, metadata). */
export function extractNoteTitle(content: string): string {
  const visible = content.replace(HTML_COMMENT, '');
  const firstLine = visible.split('\n').find(line => line.trim() !== '') ?? '';
  return firstLine.replace(HEADING_MARKS, '').trim() || 'Untitled';
}
