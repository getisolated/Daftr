import { ChangeSpec, EditorState } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import { Command, EditorView } from '@codemirror/view';
import { SyntaxNode } from '@lezer/common';

/** Indentation, marker (`-`, `*`, `+`, `1.` or `1)`) and the spaces before the item's text. */
const LIST_ITEM = /^([ \t]*)(?:(\d{1,9})([.)])|[-*+])([ \t]*)(?=\S|$)/;

interface ListItem {
  /** Width of the indentation, a tab counting for 4 columns. */
  indent: number;
  /** Column where the item's text starts: where a nested item must be indented to (CommonMark). */
  content: number;
  /** Null for a bullet. */
  number: number | null;
  /** Length of the indentation + number (what a renumbering rewrites); 0 for a bullet. */
  prefixLength: number;
  delimiter: string;
}

function indentWidth(indent: string): number {
  return [...indent].reduce((width, char) => width + (char === '\t' ? 4 : 1), 0);
}

function parseItem(text: string): ListItem | null {
  const match = LIST_ITEM.exec(text);
  if (!match) {
    return null;
  }

  const [whole, indent, digits, delimiter, spaces] = match as unknown as [string, string, string | undefined, string | undefined, string];
  const width = indentWidth(indent);
  const markerLength = whole.length - indent.length - spaces.length;
  // More than 4 spaces after the marker: the text is an indented code block, the item's content starts after 1 space
  const gap = spaces.length >= 1 && spaces.length <= 4 ? spaces.length : 1;

  return {
    indent: width,
    content: width + markerLength + gap,
    number: digits === undefined ? null : Number(digits),
    prefixLength: digits === undefined ? 0 : indent.length + digits.length,
    delimiter: delimiter ?? '',
  };
}

function isBlank(text: string): boolean {
  return text.trim() === '';
}

/** Previous item at the same indentation (the parent of the item once indented), skipping the deeper lines. */
function previousSibling(lines: string[], index: number, indent: number): ListItem | null {
  for (let i = index - 1; i >= 0; i--) {
    const text = lines[i]!;
    if (isBlank(text)) continue;
    const item = parseItem(text);
    const width = item?.indent ?? indentWidth(/^[ \t]*/.exec(text)![0]);
    if (width > indent) continue;
    return item && width === indent ? item : null;
  }
  return null;
}

/** Closest item above that is less indented: the parent the item goes back to when outdented. */
function parentItem(lines: string[], index: number, indent: number): ListItem | null {
  for (let i = index - 1; i >= 0; i--) {
    const text = lines[i]!;
    if (isBlank(text)) continue;
    const item = parseItem(text);
    if (item && item.indent < indent) return item;
    // A paragraph less indented than the item: the list ended above it
    if (!item && indentWidth(/^[ \t]*/.exec(text)![0]) < indent) return null;
  }
  return null;
}

function withNumber(text: string, item: ListItem, number: number): string {
  const indent = /^[ \t]*/.exec(text)![0];
  return `${indent}${number}${text.slice(item.prefixLength)}`;
}

/** Numbers the ordered items following `index` at `indent` from `start`, until the list ends at that level. */
function renumberFollowing(lines: string[], index: number, indent: number, start: number): void {
  let number = start;
  for (let i = index + 1; i < lines.length; i++) {
    const text = lines[i]!;
    if (isBlank(text)) continue;
    const item = parseItem(text);
    const width = item?.indent ?? indentWidth(/^[ \t]*/.exec(text)![0]);
    if (width > indent) continue;
    if (!item || width < indent || item.number === null) return;
    lines[i] = withNumber(text, item, number++);
  }
}

/**
 * Moves the list item of line `index` one level deeper (`direction` 1, under the previous item) or back to its
 * parent's level (-1), and renumbers the ordered items of both levels: the item gets the number following its new
 * previous sibling (1 when it starts a sub-list), the items it leaves continue from its old number.
 * Returns false when the line is not a list item or cannot move (first item of a list, top level).
 */
export function shiftListItem(lines: string[], index: number, direction: 1 | -1): boolean {
  const text = lines[index]!;
  const item = parseItem(text);
  if (!item) {
    return false;
  }

  const anchor = direction === 1 ? previousSibling(lines, index, item.indent) : parentItem(lines, index, item.indent);
  if (!anchor) {
    return false;
  }

  const target = direction === 1 ? anchor.content : anchor.indent;
  const newSibling = previousSibling(lines, index, target);
  let moved = ' '.repeat(target) + text.slice(/^[ \t]*/.exec(text)![0].length);
  let movedItem = parseItem(moved)!;

  if (item.number !== null) {
    const number = newSibling?.number != null && newSibling.delimiter === item.delimiter ? newSibling.number + 1 : 1;
    moved = withNumber(moved, movedItem, number);
    movedItem = parseItem(moved)!;
  }
  lines[index] = moved;

  if (item.number !== null) {
    renumberFollowing(lines, index, item.indent, item.number);
    renumberFollowing(lines, index, target, movedItem.number! + 1);
  }
  return true;
}

/** Lines touched by the selection; a selection ending at the very start of a line leaves that line out. */
function selectedLineIndexes(state: EditorState): number[] {
  const indexes = new Set<number>();
  for (const range of state.selection.ranges) {
    const first = state.doc.lineAt(range.from).number;
    let last = state.doc.lineAt(range.to).number;
    if (!range.empty && last > first && state.doc.line(last).from === range.to) {
      last--;
    }
    for (let n = first; n <= last; n++) {
      indexes.add(n - 1);
    }
  }
  return [...indexes].sort((a, b) => a - b);
}

function inCodeBlock(state: EditorState, pos: number): boolean {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); node; node = node.parent) {
    if (node.name === 'FencedCode' || node.name === 'CodeBlock') return true;
  }
  return false;
}

/** Only the start of each changed line differs: replacing just that part keeps the cursors in the text. */
function lineChanges(state: EditorState, lines: string[]): ChangeSpec[] {
  const changes: ChangeSpec[] = [];
  lines.forEach((next, index) => {
    const line = state.doc.line(index + 1);
    if (next === line.text) return;
    let suffix = 0;
    while (suffix < next.length && suffix < line.text.length && next[next.length - 1 - suffix] === line.text[line.length - 1 - suffix]) {
      suffix++;
    }
    changes.push({ from: line.from, to: line.to - suffix, insert: next.slice(0, next.length - suffix) });
  });
  return changes;
}

function shiftSelectedItems(view: EditorView, direction: 1 | -1): boolean {
  const { state } = view;
  // Blank lines of a multi-line selection are left as they are
  const indexes = selectedLineIndexes(state).filter(index => !isBlank(state.doc.line(index + 1).text));
  if (indexes.length === 0 || indexes.some(index => !parseItem(state.doc.line(index + 1).text) || inCodeBlock(state, state.doc.line(index + 1).from))) {
    return false;
  }

  const lines = state.doc.toString().split('\n');
  // Top to bottom: each item is placed relative to the ones above it, already moved
  const shifted = indexes.map(index => shiftListItem(lines, index, direction)).some(Boolean);
  if (!shifted) {
    return false;
  }

  view.dispatch({ changes: lineChanges(state, lines), userEvent: direction === 1 ? 'input.indent' : 'delete.dedent', scrollIntoView: true });
  return true;
}

/** Tab on list items: nests them under the previous item and renumbers the ordered lists. */
export const indentListItems: Command = view => shiftSelectedItems(view, 1);

/** Shift+Tab on list items: brings them back to their parent's level and renumbers the ordered lists. */
export const outdentListItems: Command = view => shiftSelectedItems(view, -1);
