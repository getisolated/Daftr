import { ChangeSpec, EditorState, Extension, Line, Text } from '@codemirror/state';
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

/** Indentation width of any line, list item or not. */
function leadingWidth(text: string): number {
  return indentWidth(/^[ \t]*/.exec(text)![0]);
}

/** Previous item at the same indentation (the parent of the item once indented), skipping the deeper lines. */
function previousSibling(lines: string[], index: number, indent: number): ListItem | null {
  for (let i = index - 1; i >= 0; i--) {
    const text = lines[i]!;
    if (isBlank(text)) continue;
    const item = parseItem(text);
    const width = item?.indent ?? leadingWidth(text);
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
    if (!item && leadingWidth(text) < indent) return null;
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
    const width = item?.indent ?? leadingWidth(text);
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

/** Next item at the same indentation, skipping the deeper lines; null once the list ends at that level. */
function nextSibling(lines: string[], index: number, indent: number): ListItem | null {
  for (let i = index + 1; i < lines.length; i++) {
    const text = lines[i]!;
    if (isBlank(text)) {
      continue;
    }

    const item = parseItem(text);
    const width = item?.indent ?? leadingWidth(text);
    if (width > indent) {
      continue;
    }
    return item && width === indent ? item : null;
  }
  return null;
}

/** Renumbers the ordered list of the item at line `index` from that item on; `start` numbers it when it begins the list. */
function renumberListAt(lines: string[], index: number, item: ListItem, start: number): void {
  const previous = previousSibling(lines, index, item.indent);
  const next = nextSibling(lines, index, item.indent);
  // Lazy numbering (`1.` on every item) is valid markdown: it is left as written
  if (previous?.number === item.number || next?.number === item.number) {
    return;
  }

  let number = start;
  // A bullet or another delimiter before the item: it begins a new list
  if (previous?.number != null && previous.delimiter === item.delimiter) {
    number = previous.number + 1;
  }
  lines[index] = withNumber(lines[index]!, item, number);
  renumberFollowing(lines, index, item.indent, number + 1);
}

/**
 * After a deletion ending on line `index`, renumbers the ordered items from there on: the first item at or after
 * the line continues its previous sibling (or takes the number `starts` gives for its indentation when it now begins
 * the list), then each enclosing list met below, while the walk climbs out of the deeper items. Sub-lists below that
 * level are separate lists and are left as they are.
 */
export function renumberListsFrom(lines: string[], index: number, starts: ReadonlyMap<number, number> = new Map()): void {
  let level = Number.POSITIVE_INFINITY;
  for (let i = index; i < lines.length && level > 0; i++) {
    const text = lines[i]!;
    if (isBlank(text)) {
      continue;
    }

    const item = parseItem(text);
    const width = item?.indent ?? leadingWidth(text);
    // The edited line may be what remains of an item (its marker deleted): the items below still follow the list
    if (width >= level || (!item && i === index)) {
      continue;
    }

    // A less indented line ends the deeper lists: only the enclosing ones are still renumbered
    level = width;
    if (item?.number != null) {
      renumberListAt(lines, i, item, starts.get(width) ?? item.number);
    }
  }
}

/**
 * Numbers of the ordered lists whose first item a deletion removed, by indentation: they keep their start once their
 * next item moves up in its place. `lines` are the lines the deletion touched, the first one whole and the last one
 * cut where the deletion ends; `deletedFrom` is the column where it starts on the first line.
 */
export function deletedListStarts(lines: string[], deletedFrom: number): Map<number, number> {
  const starts = new Map<number, number>();
  // Levels where an ordered item was met: a later item there continues that list, it does not begin one
  const open = new Set<number>();
  lines.forEach((text, index) => {
    if (isBlank(text)) {
      return;
    }

    const item = parseItem(text);
    const width = item?.indent ?? leadingWidth(text);
    // A line ends the deeper lists; a paragraph or a bullet also ends the ordered list of its own level
    const ended = (level: number): boolean => level > width || (level === width && item?.number == null);
    [...open].filter(ended).forEach(level => open.delete(level));
    [...starts.keys()].filter(ended).forEach(level => starts.delete(level));
    if (item?.number == null) {
      return;
    }

    // On the first line, the marker is deleted only when the deletion starts before it
    const markerDeleted = index > 0 || deletedFrom <= text.length - text.trimStart().length;
    if (markerDeleted && !open.has(width)) {
      starts.set(width, item.number);
    }
    open.add(width);
  });
  return starts;
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

/** Only the start of a changed line differs: replacing just that part keeps the cursors in the text. */
function lineChange(line: Line, next: string): ChangeSpec {
  let suffix = 0;
  while (suffix < next.length && suffix < line.text.length && next[next.length - 1 - suffix] === line.text[line.length - 1 - suffix]) {
    suffix++;
  }
  return { from: line.from, to: line.to - suffix, insert: next.slice(0, next.length - suffix) };
}

function lineChanges(state: EditorState, lines: string[]): ChangeSpec[] {
  const changes: ChangeSpec[] = [];
  lines.forEach((next, index) => {
    const line = state.doc.line(index + 1);
    if (next === line.text) return;
    changes.push(lineChange(line, next));
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

/** A paragraph (or a fence, a heading…) at the margin: the list above or below it ends there. */
function endsList(text: string): boolean {
  return !isBlank(text) && !parseItem(text) && leadingWidth(text) === 0;
}

/**
 * Whether a deletion may change the numbering: it joins lines or reaches an ordered marker. Deleting inside the text
 * of an item, the most frequent case when typing, is left out at once.
 */
function touchesListMarker(doc: Text, from: number, to: number): boolean {
  const line = doc.lineAt(from);
  if (to > line.to) {
    return true;
  }

  const item = parseItem(line.text);
  return item?.number != null && from - line.from <= item.prefixLength;
}

/** Ordered lists of the start state whose first item the deletion `from`-`to` removed (see deletedListStarts). */
function deletedStarts(doc: Text, from: number, to: number): Map<number, number> {
  const first = doc.lineAt(from);
  const last = doc.lineAt(to);
  const lines: string[] = [];
  for (let n = first.number; n <= last.number; n++) {
    lines.push(doc.line(n).text);
  }
  lines[lines.length - 1] = lines[lines.length - 1]!.slice(0, to - last.from);
  return deletedListStarts(lines, from - first.from);
}

/**
 * Renumbers the list around line `lineNumber` of `doc`, reading and writing the lines already renumbered in `edits`
 * (line number → text). Only the lines of that list are read, never the whole document.
 */
function renumberListAround(doc: Text, lineNumber: number, starts: ReadonlyMap<number, number>, edits: Map<number, string>): void {
  let first = lineNumber;
  while (first > 1 && !endsList(doc.line(first - 1).text)) {
    first--;
  }

  let last = lineNumber;
  while (last < doc.lines && !endsList(doc.line(last + 1).text)) {
    last++;
  }

  const lines: string[] = [];
  for (let n = first; n <= last; n++) {
    lines.push(edits.get(n) ?? doc.line(n).text);
  }
  renumberListsFrom(lines, lineNumber - first, starts);
  lines.forEach((text, index) => {
    if (text !== doc.line(first + index).text) {
      edits.set(first + index, text);
    }
  });
}

/**
 * Renumbers the ordered list an item was deleted from (Backspace / Delete, line deletion, cut, selection deleted),
 * in the same transaction: a single undo brings back both the item and the former numbers. Inserting an item
 * (Enter) is already renumbered by the markdown keymap; Tab / Shift+Tab by the commands above.
 */
export const orderedListRenumbering: Extension = EditorState.transactionFilter.of(tr => {
  if (!tr.docChanged || !tr.isUserEvent('delete') || tr.isUserEvent('delete.dedent')) {
    return tr;
  }

  const { startState, newDoc } = tr;
  const edits = new Map<number, string>();
  tr.changes.iterChanges((fromA, toA, fromB, toB) => {
    if (fromA === toA || !touchesListMarker(startState.doc, fromA, toA) || inCodeBlock(startState, fromA)) {
      return;
    }

    // Text inserted in place of the deleted one (the marker of an item replaced by spaces) begins no list
    const starts = fromB === toB ? deletedStarts(startState.doc, fromA, toA) : new Map<number, number>();
    renumberListAround(newDoc, newDoc.lineAt(toB).number, starts, edits);
  });

  const inverted = tr.changes.invertedDesc;
  const changes: ChangeSpec[] = [];
  [...edits].sort(([a], [b]) => a - b).forEach(([lineNumber, text]) => {
    const line = newDoc.line(lineNumber);
    // A line that looks like an item but sits in a code block is code: its number stays
    if (!inCodeBlock(startState, inverted.mapPos(line.from))) {
      changes.push(lineChange(line, text));
    }
  });
  return changes.length === 0 ? tr : [tr, { changes, sequential: true }];
});
