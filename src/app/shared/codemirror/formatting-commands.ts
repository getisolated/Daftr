import { EditorSelection, EditorState, Line } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import { FormatId } from '../../core/models/formatting-bar.model';
import { findDraftBlocks, toggleDraft } from './draft-blocks';
import { createEmptyTable } from './markdown-table';

// ─── Slash menu primitives: they replace the `/query` range typed by the user ──

/** Replaces the slash range (from `/` to end of query) with a line prefix */
export function insertLinePrefix(view: EditorView, range: { from: number; to: number }, prefix: string) {
  const line = view.state.doc.lineAt(range.from);
  const beforeSlash = view.state.sliceDoc(line.from, range.from);
  const afterQuery = view.state.sliceDoc(range.to, line.to);
  const newText = beforeSlash.replace(/^(\s*).*/, '$1') + prefix + afterQuery.trimStart();
  view.dispatch({
    changes: { from: line.from, to: line.to, insert: newText },
    selection: { anchor: line.from + newText.length },
  });
  view.focus();
}

/** Wraps selection or inserts placeholder with wrap markers */
export function insertWrap(view: EditorView, range: { from: number; to: number }, before: string, after: string, placeholder: string) {
  const insert = `${before}${placeholder}${after}`;
  view.dispatch({
    changes: { from: range.from, to: range.to, insert },
    selection: { anchor: range.from + before.length, head: range.from + before.length + placeholder.length },
  });
  view.focus();
}

/** Inserts a code block */
export function insertCodeBlock(view: EditorView, range: { from: number; to: number }) {
  const line = view.state.doc.lineAt(range.from);
  const insert = '```\n\n```';
  view.dispatch({
    changes: { from: line.from, to: line.to, insert },
    selection: { anchor: line.from + 4 },
  });
  view.focus();
}

/** Inserts a horizontal rule */
export function insertHorizontalRule(view: EditorView, range: { from: number; to: number }) {
  const line = view.state.doc.lineAt(range.from);
  const insert = '---\n';
  view.dispatch({
    changes: { from: line.from, to: line.to, insert },
    selection: { anchor: line.from + insert.length },
  });
  view.focus();
}

/** Replaces the slash line with an empty 2×2 table, the caret in its first header cell */
export function insertTable(view: EditorView, range: { from: number; to: number }): void {
  const line = view.state.doc.lineAt(range.from);
  // A table cannot interrupt a paragraph: keep a blank line before it
  const needsBlankLine = line.number > 1 && view.state.doc.line(line.number - 1).text.trim() !== '';
  const prefix = needsBlankLine ? '\n' : '';
  const { text, cells } = createEmptyTable();

  view.dispatch({
    changes: { from: line.from, to: line.to, insert: `${prefix}${text}` },
    selection: { anchor: line.from + prefix.length + cells[0]![0]!.from },
  });
  view.focus();
}

// SVG icon paths (24x24 viewBox)
export const ICONS: Record<FormatId, string> = {
  paragraph: '<path d="M13 4a4 4 0 0 1 0 8H7v8m6-16v16m4-16v16" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  h1: '<path d="M4 12h6M4 6v12M10 6v12" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><text x="15" y="18" font-size="12" font-weight="700" fill="currentColor">1</text>',
  h2: '<path d="M4 12h6M4 6v12M10 6v12" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><text x="15" y="18" font-size="12" font-weight="700" fill="currentColor">2</text>',
  h3: '<path d="M4 12h6M4 6v12M10 6v12" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><text x="15" y="18" font-size="12" font-weight="700" fill="currentColor">3</text>',
  h4: '<path d="M4 12h6M4 6v12M10 6v12" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><text x="15" y="18" font-size="12" font-weight="700" fill="currentColor">4</text>',
  bulletList: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  orderedList: '<path d="M10 6h11M10 12h11M10 18h11" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><text x="1" y="8" font-size="9" fill="currentColor">1.</text><text x="1" y="14" font-size="9" fill="currentColor">2.</text><text x="1" y="20" font-size="9" fill="currentColor">3.</text>',
  quote: '<path d="M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V21z" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 .001 0 1.003 1 1.003z" fill="none" stroke="currentColor" stroke-width="1.5"/>',
  codeBlock: '<polyline points="16 18 22 12 16 6" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/><polyline points="8 6 2 12 8 18" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  bold: '<path d="M6 4h8a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6zM6 12h9a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  italic: '<line x1="19" y1="4" x2="10" y2="4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><line x1="14" y1="20" x2="5" y2="20" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><line x1="15" y1="4" x2="9" y2="20" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
  strikethrough: '<path d="M16 4a4 4 0 0 0-4.37-.37 4.95 4.95 0 0 0-3.39 2.87M4 12h16M12 12c5 0 5 8-1 8a3.5 3.5 0 0 1-3.5-3.5" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  inlineCode: '<path d="M16 18l6-6-6-6M8 6l-6 6 6 6" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  table: '<rect x="3" y="4" width="18" height="16" rx="2" stroke="currentColor" stroke-width="1.8" fill="none"/><path d="M3 10h18M3 15h18M9 4v16" stroke="currentColor" stroke-width="1.8" fill="none"/>',
  draft: '<path d="M12 20h9" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
  hr: '<path d="M3 12h18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><circle cx="6" cy="12" r="1" fill="currentColor"/><circle cx="12" cy="12" r="1" fill="currentColor"/><circle cx="18" cy="12" r="1" fill="currentColor"/>',
  taskList: '<rect x="3" y="5" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.8" fill="none"/><path d="M3.5 14.5h5v5h-5z" stroke="currentColor" stroke-width="1.8" fill="none" rx="1"/><path d="M11 8h10M11 17h10" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/><path d="M4.5 8l1.5 1.5 3-3" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
};


// ─── Toolbar commands: they toggle the formatting under the selection ────────

const HEADING_PREFIX = /^#{1,6}[ \t]+/;
const LIST_PREFIX = /^([-*+]|\d+[.)])[ \t]+(\[[ xX]\][ \t]+)?/;
const QUOTE_PREFIX = /^>[ \t]?/;
const FENCE_LINE = /^\s*(`{3,}|~{3,})\s*$/;

type LineKind = 'paragraph' | 'h1' | 'h2' | 'h3' | 'h4' | 'bulletList' | 'orderedList' | 'taskList' | 'quote';

/** Lines touched by the selection; a selection ending at the very start of a line leaves that line out. */
function selectedLines(state: EditorState): Line[] {
  const lines = new Map<number, Line>();
  for (const range of state.selection.ranges) {
    const first = state.doc.lineAt(range.from);
    let last = state.doc.lineAt(range.to);
    if (!range.empty && last.number > first.number && last.from === range.to) {
      last = state.doc.line(last.number - 1);
    }
    for (let n = first.number; n <= last.number; n++) {
      lines.set(n, state.doc.line(n));
    }
  }
  return [...lines.values()];
}

function lineHasKind(kind: LineKind, body: string): boolean {
  const list = LIST_PREFIX.exec(body);
  switch (kind) {
    case 'h1': case 'h2': case 'h3': case 'h4': {
      const heading = HEADING_PREFIX.exec(body);
      return heading !== null && heading[0].trim().length === Number(kind[1]);
    }
    case 'bulletList': return list !== null && !list[2] && /^[-*+]/.test(list[1]!);
    case 'orderedList': return list !== null && /^\d/.test(list[1]!);
    case 'taskList': return list !== null && !!list[2];
    case 'quote': return QUOTE_PREFIX.test(body);
    default: return false;
  }
}

function rewriteLine(kind: LineKind, body: string, index: number, remove: boolean): string {
  switch (kind) {
    case 'paragraph': return body.replace(QUOTE_PREFIX, '').replace(LIST_PREFIX, '').replace(HEADING_PREFIX, '');
    case 'h1': case 'h2': case 'h3': case 'h4': {
      const bare = body.replace(HEADING_PREFIX, '');
      return remove ? bare : `${'#'.repeat(Number(kind[1]))} ${bare}`;
    }
    case 'quote': return remove ? body.replace(QUOTE_PREFIX, '') : `> ${body}`;
    default: {
      const bare = body.replace(LIST_PREFIX, '');
      if (remove) return bare;
      const marker = kind === 'orderedList' ? `${index + 1}. ` : kind === 'taskList' ? '- [ ] ' : '- ';
      return marker + bare;
    }
  }
}

/** Applies a block style to the selected lines; applying it again where it already is removes it. */
export function toggleLineKind(view: EditorView, kind: LineKind): void {
  const selected = selectedLines(view.state);
  // Blank lines of a multi-line selection stay blank instead of getting an empty bullet
  const lines = selected
    .filter(line => selected.length === 1 || line.text.trim() !== '')
    .map(line => ({ line, indent: /^\s*/.exec(line.text)![0] }));
  const remove = kind !== 'paragraph' && lines.every(({ line, indent }) => lineHasKind(kind, line.text.slice(indent.length)));

  const changes = lines.flatMap(({ line, indent }, index) => {
    const next = indent + rewriteLine(kind, line.text.slice(indent.length), index, remove);
    return next === line.text ? [] : [{ from: line.from, to: line.to, insert: next }];
  });

  if (changes.length > 0) {
    view.dispatch({ changes });
  }
  view.focus();
}

/** Innermost-first search of an ancestor node, which must also cover up to `end`. */
function findAncestor(state: EditorState, pos: number, name: string, end = pos): SyntaxNode | null {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, -1); node; node = node.parent) {
    if (node.name === name && node.to >= end) return node;
  }
  return null;
}

/** Wraps the selection with a markdown mark, or unwraps it when the caret already sits inside one. */
export function toggleInlineMark(view: EditorView, marker: string, nodeName: string): void {
  const { state } = view;
  const main = state.selection.main;
  const node = findAncestor(state, main.from, nodeName, main.to);
  const open = node?.firstChild;
  const close = node?.lastChild;

  if (open && close && open !== close) {
    view.dispatch({ changes: [{ from: open.from, to: open.to }, { from: close.from, to: close.to }] });
  } else {
    view.dispatch(state.changeByRange(range => ({
      changes: { from: range.from, to: range.to, insert: `${marker}${state.sliceDoc(range.from, range.to)}${marker}` },
      // Empty: caret between the marks. Otherwise the text stays selected, inside them
      range: range.empty
        ? EditorSelection.cursor(range.from + marker.length)
        : EditorSelection.range(range.from + marker.length, range.to + marker.length),
    })));
  }
  view.focus();
}

/** Range covering a whole line and one of its line breaks, so deleting it leaves no empty line behind. */
function lineRangeWithBreak(state: EditorState, line: Line): { from: number; to: number } {
  if (line.to < state.doc.length) return { from: line.from, to: line.to + 1 };
  return { from: Math.max(0, line.from - 1), to: line.to };
}

/** Fences the selected lines, or removes the fences when the caret is inside a code block. */
export function toggleCodeBlock(view: EditorView): void {
  const { state } = view;
  const fenced = findAncestor(state, state.selection.main.head, 'FencedCode');

  if (fenced) {
    const openLine = state.doc.lineAt(fenced.from);
    const closeLine = state.doc.lineAt(fenced.to);
    const ranges = [lineRangeWithBreak(state, openLine)];
    if (closeLine.number > openLine.number && FENCE_LINE.test(closeLine.text)) {
      const closing = lineRangeWithBreak(state, closeLine);
      const opening = ranges[0]!;
      // Adjacent fences share a line break: a single deletion
      if (closing.from <= opening.to) {
        opening.to = Math.max(opening.to, closing.to);
        // Nothing follows: take the line break before the block instead
        if (opening.to === state.doc.length && opening.from > 0) opening.from--;
      }
      else ranges.push(closing);
    }
    view.dispatch({ changes: ranges });
  } else {
    const lines = selectedLines(state);
    const first = lines[0]!;
    const last = lines[lines.length - 1]!;

    if (lines.length === 1 && first.text.trim() === '') {
      view.dispatch({ changes: { from: first.from, to: first.to, insert: '```\n\n```' }, selection: { anchor: first.from + 4 } });
    } else {
      view.dispatch({ changes: [{ from: first.from, insert: '```\n' }, { from: last.to, insert: '\n```' }] });
    }
  }
  view.focus();
}

/** Inserts a rule below the current line; the blank line before it keeps it from turning the paragraph into a heading. */
export function insertRuleBelow(view: EditorView): void {
  const line = view.state.doc.lineAt(view.state.selection.main.head);

  if (line.text.trim() === '') {
    insertHorizontalRule(view, { from: line.from, to: line.to });
    return;
  }

  const insert = '\n\n---\n';
  view.dispatch({ changes: { from: line.to, insert }, selection: { anchor: line.to + insert.length } });
  view.focus();
}

/** Inserts an empty table: in place on a blank line, otherwise below the current line. Nothing happens inside a table. */
export function insertTableBelow(view: EditorView): void {
  const { state } = view;
  const line = state.doc.lineAt(state.selection.main.head);

  const names = new Set<string>();

  collectAncestorNames(state, line.from + /^\s*/.exec(line.text)![0].length, 1, names);

  if (names.has('Table')) {
    view.focus();
    return;
  }

  if (line.text.trim() !== '') {
    // A table needs a blank line before it, and the following text stays out of it
    const { text, cells } = createEmptyTable();
    const insert = `\n\n${text}\n`;
    view.dispatch({ changes: { from: line.to, insert }, selection: { anchor: line.to + 2 + cells[0]![0]!.from } });
    view.focus();
    return;
  }

  insertTable(view, { from: line.from, to: line.to });
}

/** Turns the selected lines into a draft block, or the draft under the caret back into text. */
function toggleDraftAndFocus(view: EditorView): void {
  toggleDraft(view);
  view.focus();
}

// ─── Active formatting under the caret ───────────────────────────────────────

function collectAncestorNames(state: EditorState, pos: number, side: -1 | 1, into: Set<string>): void {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, side); node; node = node.parent) {
    into.add(node.name);
  }
}

/** Formats applying at the caret, read from the markdown syntax tree. */
export function detectActiveFormats(state: EditorState): Set<FormatId> {
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  const inline = new Set<string>();
  const block = new Set<string>();

  collectAncestorNames(state, head, -1, inline);
  // At the very start of a line the caret still belongs to the previous node: the line content is looked at too
  collectAncestorNames(state, line.from + /^\s*/.exec(line.text)![0].length, 1, block);
  inline.forEach(name => block.add(name));

  const active = new Set<FormatId>();
  for (let level = 1; level <= 4; level++) {
    if (block.has(`ATXHeading${level}`) || block.has(`SetextHeading${level}`)) active.add(`h${level}` as FormatId);
  }
  if (inline.has('StrongEmphasis')) active.add('bold');
  if (inline.has('Emphasis')) active.add('italic');
  if (inline.has('Strikethrough')) active.add('strikethrough');
  if (inline.has('InlineCode')) active.add('inlineCode');
  if (block.has('FencedCode')) active.add('codeBlock');
  if (block.has('Blockquote')) active.add('quote');
  if (block.has('HorizontalRule')) active.add('hr');
  if (block.has('Table')) active.add('table');
  const caretLine = line.number;
  if (findDraftBlocks(state.doc).some(draft => caretLine >= draft.startLine && caretLine <= draft.endLine)) active.add('draft');
  if (block.has('Task')) active.add('taskList');
  else if (block.has('BulletList')) active.add('bulletList');
  if (block.has('OrderedList')) active.add('orderedList');
  if (active.size === 0) active.add('paragraph');

  return active;
}

// ─── Actions shared by the slash menu (labels, icons) and the toolbar (toggles) ──

export interface FormatAction {
  id: FormatId;
  label: string;
  description: string;
  /** SVG path(s) for a 24x24 viewBox. */
  icon: string;
  toggle: (view: EditorView) => void;
}

function action(id: FormatId, label: string, description: string, toggle: FormatAction['toggle']): [FormatId, FormatAction] {
  return [id, { id, label, description, icon: ICONS[id], toggle }];
}

export const FORMAT_ACTIONS = Object.fromEntries([
  action('paragraph', 'Paragraphe', 'Texte normal', view => toggleLineKind(view, 'paragraph')),
  action('h1', 'Titre 1', 'Titre principal', view => toggleLineKind(view, 'h1')),
  action('h2', 'Titre 2', 'Sous-titre', view => toggleLineKind(view, 'h2')),
  action('h3', 'Titre 3', 'Sous-section', view => toggleLineKind(view, 'h3')),
  action('h4', 'Titre 4', 'Sous-sous-section', view => toggleLineKind(view, 'h4')),
  action('bulletList', 'Liste à puces', 'Liste non ordonnée', view => toggleLineKind(view, 'bulletList')),
  action('orderedList', 'Liste numérotée', 'Liste ordonnée', view => toggleLineKind(view, 'orderedList')),
  action('taskList', 'Liste de tâches', 'Case à cocher', view => toggleLineKind(view, 'taskList')),
  action('quote', 'Citation', 'Bloc de citation', view => toggleLineKind(view, 'quote')),
  action('codeBlock', 'Bloc de code', 'Bloc de code multi-ligne', toggleCodeBlock),
  action('bold', 'Gras', 'Texte en gras', view => toggleInlineMark(view, '**', 'StrongEmphasis')),
  action('italic', 'Italique', 'Texte en italique', view => toggleInlineMark(view, '*', 'Emphasis')),
  action('strikethrough', 'Barré', 'Texte barré', view => toggleInlineMark(view, '~~', 'Strikethrough')),
  action('inlineCode', 'Code', 'Code en ligne', view => toggleInlineMark(view, '`', 'InlineCode')),
  action('hr', 'Séparateur', 'Ligne horizontale', insertRuleBelow),
  action('table', 'Tableau', 'Tableau 2×2', insertTableBelow),
  action('draft', 'Brouillon', 'Bloc de brouillon', toggleDraftAndFocus),
]) as Record<FormatId, FormatAction>;

/** Toolbar layout: one array per visual group. */
export const FORMATTING_BAR_GROUPS: FormatId[][] = [
  ['paragraph', 'h1', 'h2', 'h3', 'h4'],
  ['bold', 'italic', 'strikethrough', 'inlineCode'],
  ['codeBlock', 'quote', 'hr', 'table', 'draft'],
  ['bulletList', 'orderedList', 'taskList'],
];
