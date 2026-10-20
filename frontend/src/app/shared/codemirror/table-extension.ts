import { EditorState, Range, StateField } from '@codemirror/state';
import { Decoration, DecorationSet, EditorView, WidgetType, keymap } from '@codemirror/view';
import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import {
  ParsedTable, TableAlign, TableModel, alignColumn, cellAt, insertColumn, insertRow, parseTable, removeColumn,
  removeRow, serializeTable, tsvToMarkdownTable,
} from './markdown-table';
import { isInCodeBlock } from './slash-commands';
import { EXTERNAL_URL, ExternalLinkWidget } from './markdown-widgets';

// ─── Table detection ─────────────────────────────────────────────────────────

interface TableRange {
  from: number;
  to: number;
  text: string;
}

export type TableAction =
  | 'addRow' | 'deleteRow' | 'addColumn' | 'deleteColumn'
  | 'alignLeft' | 'alignCenter' | 'alignRight';

export interface TableCursorInfo {
  /** 0 = header. */
  row: number;
  col: number;
  rowCount: number;
  columnCount: number;
}

const ACTION_ALIGN: Partial<Record<TableAction, TableAlign>> = {
  alignLeft: 'left',
  alignCenter: 'center',
  alignRight: 'right',
};

/** Top-level GFM tables only: tables nested in lists or quotes keep their raw markdown. */
function findTables(state: EditorState): TableRange[] {
  const tree = ensureSyntaxTree(state, state.doc.length, 50) ?? syntaxTree(state);
  const tables: TableRange[] = [];

  tree.iterate({
    enter(node) {
      if (node.name !== 'Table') {
        return node.name === 'Document';
      }

      if (node.node.parent?.name === 'Document') {
        const from = state.doc.lineAt(node.from).from;
        const to = state.doc.lineAt(node.to).to;
        const text = state.sliceDoc(from, to);

        if (parseTable(text)) {
          tables.push({ from, to, text });
        }
      }

      return false;
    },
  });

  return tables;
}

function tableAtPos(state: EditorState, pos: number): TableRange | null {
  return findTables(state).find(table => pos >= table.from && pos <= table.to) ?? null;
}

export function describeTableAtCursor(state: EditorState): TableCursorInfo | null {
  const table = tableAtPos(state, state.selection.main.head);
  const parsed = table ? parseTable(table.text) : null;

  if (!table || !parsed) {
    return null;
  }

  const { row, col } = cellAt(parsed, state.selection.main.head - table.from);

  return { row, col, rowCount: parsed.model.rows.length + 1, columnCount: parsed.model.header.length };
}

// ─── Editing ─────────────────────────────────────────────────────────────────

/** Rewrites the table with aligned pipes and selects the content of cell (`row`, `col`). */
function commitTable(view: EditorView, table: TableRange, model: TableModel, row: number, col: number, caretInCell?: number): void {
  const { text, cells } = serializeTable(model);
  const span = cells[row]![col]!;
  const anchor = table.from + (caretInCell === undefined ? span.from : span.from + Math.min(caretInCell, span.to - span.from));
  const head = caretInCell === undefined ? table.from + span.to : anchor;

  view.dispatch({
    changes: text === table.text ? undefined : { from: table.from, to: table.to, insert: text },
    selection: { anchor, head },
    scrollIntoView: true,
  });
}

/** Aligns the pipes of the table holding the cursor, keeping the caret at the same place in its cell. */
export function formatTableAtCursor(view: EditorView): boolean {
  const head = view.state.selection.main.head;
  const table = tableAtPos(view.state, head);
  const parsed = table ? parseTable(table.text) : null;

  if (!table || !parsed || view.state.readOnly) {
    return false;
  }

  const offset = head - table.from;
  const { row, col } = cellAt(parsed, offset);
  const lineIndex = row === 0 ? 0 : row + 1;
  const span = parsed.lines[lineIndex]![col];
  const onDelimiter = lineIndex === 1;
  const caretInCell = !span || onDelimiter ? 0 : Math.max(0, offset - parsed.lineStarts[lineIndex]! - span.from);

  commitTable(view, table, parsed.model, row, col, caretInCell);

  return true;
}

export function moveToCell(view: EditorView, delta: 1 | -1): boolean {
  const head = view.state.selection.main.head;
  const table = tableAtPos(view.state, head);
  const parsed = table ? parseTable(table.text) : null;

  if (!table || !parsed || view.state.readOnly) {
    return false;
  }

  const columnCount = parsed.model.header.length;
  const current = cellAt(parsed, head - table.from);
  let model = parsed.model;
  const index = Math.max(0, current.row * columnCount + current.col + delta);

  // Tab on the last cell grows the table
  if (index >= (model.rows.length + 1) * columnCount) {
    model = insertRow(model, model.rows.length);
  }

  commitTable(view, table, model, Math.floor(index / columnCount), index % columnCount);

  return true;
}

export function applyTableAction(view: EditorView, action: TableAction): boolean {
  const head = view.state.selection.main.head;
  const table = tableAtPos(view.state, head);
  const parsed = table ? parseTable(table.text) : null;

  if (!table || !parsed || view.state.readOnly) {
    return false;
  }

  let model = parsed.model;
  let { row, col } = cellAt(parsed, head - table.from);
  const align = ACTION_ALIGN[action];

  if (align) {
    model = alignColumn(model, col, align);
  } else if (action === 'addRow') {
    model = insertRow(model, row);
    row++;
  } else if (action === 'deleteRow') {
    if (row === 0) {
      return false;
    }

    model = removeRow(model, row - 1);
    row = Math.min(row, model.rows.length);
  } else if (action === 'addColumn') {
    model = insertColumn(model, col + 1);
    col++;
  } else {
    if (model.header.length <= 1) {
      return false;
    }

    model = removeColumn(model, col);
    col = Math.min(col, model.header.length - 1);
  }

  commitTable(view, table, model, row, col);
  view.focus();

  return true;
}

// ─── Inline rendering of cells ───────────────────────────────────────────────

interface InlineRule {
  pattern: RegExp;
  /** Underscore flavours must not start inside a word (snake_case). */
  wordBoundary?: boolean;
  render: (match: RegExpExecArray, parent: Node) => void;
}

function appendInline(parent: Node, tag: string, className: string | null, content: string): void {
  const element = document.createElement(tag);

  if (className) {
    element.className = className;
  }

  renderInline(element, content);
  parent.appendChild(element);
}

const INLINE_RULES: InlineRule[] = [
  {
    pattern: /\\([\\`*_~|[\]()<>#+\-.!])/y,
    render: (match, parent) => parent.appendChild(document.createTextNode(match[1]!)),
  },
  {
    pattern: /(`+)(.+?)\1/y,
    render: (match, parent) => {
      const code = document.createElement('code');

      code.className = 'cm-md-inline-code';
      code.textContent = match[2]!;
      parent.appendChild(code);
    },
  },
  { pattern: /\*\*(?=\S)(.+?)(?<=\S)\*\*/y, render: (match, parent) => appendInline(parent, 'strong', 'cm-md-bold', match[1]!) },
  { pattern: /__(?=\S)(.+?)(?<=\S)__/y, wordBoundary: true, render: (match, parent) => appendInline(parent, 'strong', 'cm-md-bold', match[1]!) },
  { pattern: /~~(?=\S)(.+?)(?<=\S)~~/y, render: (match, parent) => appendInline(parent, 'del', 'cm-md-strike', match[1]!) },
  { pattern: /\*(?=\S)(.+?)(?<=\S)\*/y, render: (match, parent) => appendInline(parent, 'em', 'cm-md-italic', match[1]!) },
  { pattern: /_(?=\S)(.+?)(?<=\S)_/y, wordBoundary: true, render: (match, parent) => appendInline(parent, 'em', 'cm-md-italic', match[1]!) },
  // Not an <a>: a click inside the table must edit the cell, never navigate
  {
    pattern: /\[([^\]]+)\]\((?<url>[^)]*)\)/y,
    render: (match, parent) => {
      appendInline(parent, 'span', 'cm-md-link', match[1]!);

      if (EXTERNAL_URL.test(match.groups?.['url'] ?? '')) {
        parent.appendChild(new ExternalLinkWidget().toDOM());
      }
    },
  },
  { pattern: /<br\s*\/?>/iy, render: (_, parent) => parent.appendChild(document.createElement('br')) },
];

/** Minimal inline markdown (bold, italic, strike, code, link, `<br>`), built with DOM nodes so cell content is never parsed as HTML. */
function renderInline(parent: Node, text: string): void {
  let plain = '';
  let i = 0;

  const flush = (): void => {
    if (plain) {
      parent.appendChild(document.createTextNode(plain));
      plain = '';
    }
  };

  while (i < text.length) {
    let matched = false;

    for (const rule of INLINE_RULES) {
      if (rule.wordBoundary && i > 0 && /\w/.test(text[i - 1]!)) {
        continue;
      }

      rule.pattern.lastIndex = i;
      const match = rule.pattern.exec(text);

      if (match) {
        flush();
        rule.render(match, parent);
        i += match[0].length;
        matched = true;
        break;
      }
    }

    if (!matched) {
      plain += text[i];
      i++;
    }
  }

  flush();
}

// ─── Widget ──────────────────────────────────────────────────────────────────

class TableWidget extends WidgetType {
  constructor(private readonly text: string) {
    super();
  }

  override eq(other: TableWidget): boolean {
    return other.text === this.text;
  }

  override ignoreEvent(): boolean {
    return true;
  }

  override toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement('div');

    wrap.className = 'cm-md-table-wrap';

    const parsed = parseTable(this.text);

    if (!parsed) {
      return wrap;
    }

    const table = document.createElement('table');
    const head = table.createTHead().insertRow();
    const body = table.createTBody();

    table.className = 'cm-md-table';
    parsed.model.header.forEach((cell, col) => head.appendChild(this.buildCell('th', cell, 0, col, parsed.model.align[col]!)));
    parsed.model.rows.forEach((row, index) => {
      const tr = body.insertRow();

      row.forEach((cell, col) => tr.appendChild(this.buildCell('td', cell, index + 1, col, parsed.model.align[col]!)));
    });
    wrap.appendChild(table);

    // Clicking a cell reveals the raw markdown with the caret in that cell
    wrap.addEventListener('mousedown', (event) => {
      if (event.button !== 0 || this.selectCell(view, wrap, parsed, event)) {
        event.preventDefault();
      }
    });
    wrap.addEventListener('contextmenu', (event) => {
      event.preventDefault();
      event.stopPropagation();
      this.selectCell(view, wrap, parsed, event);
      // The widget is gone after the selection change: replay the event on the editor so its context menu handler sees the cursor inside the table
      view.contentDOM.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true, cancelable: true, clientX: event.clientX, clientY: event.clientY,
      }));
    });

    return wrap;
  }

  private buildCell(tag: 'th' | 'td', content: string, row: number, col: number, align: TableAlign): HTMLElement {
    const cell = document.createElement(tag);

    cell.dataset['row'] = String(row);
    cell.dataset['col'] = String(col);

    if (align !== 'none') {
      cell.style.textAlign = align;
    }

    renderInline(cell, content);

    return cell;
  }

  private selectCell(view: EditorView, wrap: HTMLElement, parsed: ParsedTable, event: MouseEvent): boolean {
    const cell = (event.target as HTMLElement).closest<HTMLElement>('th, td');

    if (!cell) {
      return false;
    }

    const row = Number(cell.dataset['row']);
    const col = Number(cell.dataset['col']);
    const lineIndex = row === 0 ? 0 : row + 1;
    const span = parsed.lines[lineIndex]?.[col];
    const position = view.posAtDOM(wrap) + parsed.lineStarts[lineIndex]! + (span?.to ?? 0);

    view.dispatch({ selection: { anchor: position } });
    view.focus();

    return true;
  }
}

// ─── Decorations ─────────────────────────────────────────────────────────────

function selectionTouches(state: EditorState, table: TableRange): boolean {
  return state.selection.ranges.some(range => range.from <= table.to && range.to >= table.from);
}

function buildTableDecorations(state: EditorState): DecorationSet {
  const decorations: Range<Decoration>[] = [];
  const lineDecoration = Decoration.line({ class: 'cm-md-table-line' });

  for (const table of findTables(state)) {
    if (!selectionTouches(state, table)) {
      decorations.push(Decoration.replace({ widget: new TableWidget(table.text), block: true }).range(table.from, table.to));
      continue;
    }

    // Raw markdown: monospace keeps the aligned pipes aligned
    const lastLine = state.doc.lineAt(table.to).number;

    for (let number = state.doc.lineAt(table.from).number; number <= lastLine; number++) {
      decorations.push(lineDecoration.range(state.doc.line(number).from));
    }
  }

  return Decoration.set(decorations, true);
}

// Block decorations cannot come from a ViewPlugin, hence a StateField
const tableDecorations = StateField.define<DecorationSet>({
  create: buildTableDecorations,
  update(decorations, tr) {
    if (tr.docChanged || tr.selection || syntaxTree(tr.state) !== syntaxTree(tr.startState)) {
      return buildTableDecorations(tr.state);
    }

    return decorations;
  },
  provide: field => EditorView.decorations.from(field),
});

// ─── Keymap, paste, blur ─────────────────────────────────────────────────────

const tableKeymap = keymap.of([
  { key: 'Tab', run: view => moveToCell(view, 1) },
  { key: 'Shift-Tab', run: view => moveToCell(view, -1) },
]);

const tableDomHandlers = EditorView.domEventHandlers({
  blur(_event, view) {
    formatTableAtCursor(view);

    return false;
  },
  paste(event, view) {
    const text = event.clipboardData?.getData('text/plain') ?? '';
    const head = view.state.selection.main.head;

    if (view.state.readOnly || tableAtPos(view.state, head) || isInCodeBlock(view.state, head)) {
      return false;
    }

    const table = tsvToMarkdownTable(text);

    if (!table) {
      return false;
    }

    event.preventDefault();

    const { from, to } = view.state.selection.main;
    const line = view.state.doc.lineAt(from);
    const needsLeadingBreak = view.state.sliceDoc(line.from, from).trim() !== '';
    const insert = `${needsLeadingBreak ? '\n' : ''}${table}\n`;

    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + insert.length },
      userEvent: 'input.paste',
    });

    return true;
  },
});

const tableTheme = EditorView.baseTheme({
  '.cm-md-table-wrap': {
    overflowX: 'auto',
    margin: '4px 0',
    cursor: 'text',
  },
  '.cm-md-table': {
    borderCollapse: 'collapse',
    fontSize: '0.95em',
  },
  '.cm-md-table th, .cm-md-table td': {
    border: '1px solid var(--daftr-border)',
    padding: '4px 12px',
    verticalAlign: 'top',
    minWidth: '3ch',
    height: '1.4em',
  },
  '.cm-md-table th': {
    background: 'var(--daftr-bg-chip)',
    fontWeight: '600',
    color: 'var(--daftr-text-strong)',
  },
  '.cm-md-table-line': {
    fontFamily: 'Consolas, "Courier New", monospace',
    fontSize: '0.92em',
  },
});

export const tableExtensions = [
  tableDecorations,
  tableKeymap,
  tableDomHandlers,
  tableTheme,
];
