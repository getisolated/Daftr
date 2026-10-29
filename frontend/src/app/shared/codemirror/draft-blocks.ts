import { EditorState, Extension, Range, StateField, Text } from '@codemirror/state';
import { Decoration, DecorationSet, EditorView, WidgetType } from '@codemirror/view';
import { translatorOf } from './translator';

/** Markers persisted in the note's markdown: a draft block is the lines between them. */
export const DRAFT_START = '<!-- draft -->';
export const DRAFT_END = '<!-- /draft -->';

export interface DraftBlock {
  /** 1-based number of the start marker line. */
  startLine: number;
  /** 1-based number of the end marker line. */
  endLine: number;
}

/** Complete (start + end) draft blocks of the document. An unclosed start marker is ignored. */
export function findDraftBlocks(doc: Text): DraftBlock[] {
  const blocks: DraftBlock[] = [];
  let open: number | null = null;
  for (let number = 1; number <= doc.lines; number++) {
    const text = doc.line(number).text.trim();
    if (text === DRAFT_START) {
      open = number;
    } else if (text === DRAFT_END && open !== null) {
      blocks.push({ startLine: open, endLine: number });
      open = null;
    }
  }
  return blocks;
}

/** Removes the marker lines, keeping the content (the block is no longer a draft). */
export function confirmDraft(view: EditorView, block: DraftBlock): void {
  const { doc } = view.state;
  const start = doc.line(block.startLine);
  const end = doc.line(block.endLine);
  view.dispatch({
    changes: [
      { from: start.from, to: Math.min(start.to + 1, doc.length) },
      { from: Math.max(end.from - 1, start.to + 1), to: end.to },
    ],
  });
}

/** Deletes the whole block, markers included. */
export function discardDraft(view: EditorView, block: DraftBlock): void {
  const { doc } = view.state;
  const start = doc.line(block.startLine);
  const end = doc.line(block.endLine);
  // Take one line break with it so no blank line is left behind
  const from = start.from > 0 ? start.from - 1 : start.from;
  const to = start.from > 0 ? end.to : Math.min(end.to + 1, doc.length);
  view.dispatch({ changes: { from, to } });
}

/** Turns the selected lines into a draft block, or the draft block holding the cursor back into plain text. */
export function toggleDraft(view: EditorView): boolean {
  const { state } = view;
  const selection = state.selection.main;
  const first = state.doc.lineAt(selection.from);
  const last = state.doc.lineAt(selection.to);

  const enclosing = findDraftBlocks(state.doc).find(
    block => first.number >= block.startLine && last.number <= block.endLine,
  );
  if (enclosing) {
    confirmDraft(view, enclosing);
    return true;
  }

  view.dispatch({
    changes: [
      { from: first.from, insert: `${DRAFT_START}\n` },
      { from: last.to, insert: `\n${DRAFT_END}` },
    ],
  });
  return true;
}

class DraftActionsWidget extends WidgetType {
  constructor(private readonly endPosition: number) {
    super();
  }

  override eq(other: DraftActionsWidget): boolean {
    return other.endPosition === this.endPosition;
  }

  override toDOM(view: EditorView): HTMLElement {
    const container = document.createElement('div');
    const t = translatorOf(view.state);
    container.className = 'cm-draft-actions';
    container.append(
      this.button(view, 'cm-draft-confirm', '✓', t('editor.draft.confirm'), confirmDraft),
      this.button(view, 'cm-draft-discard', '✕', t('editor.draft.discard'), discardDraft),
    );
    return container;
  }

  override ignoreEvent(): boolean {
    return true;
  }

  private button(
    view: EditorView,
    className: string,
    icon: string,
    label: string,
    action: (view: EditorView, block: DraftBlock) => void,
  ): HTMLElement {
    const button = document.createElement('button');
    button.className = className;
    button.type = 'button';
    button.textContent = `${icon} ${label}`;
    // Keeps the editor focus and its selection
    button.addEventListener('mousedown', event => event.preventDefault());
    button.addEventListener('click', () => {
      // Positions shift while editing: the block is looked up again at click time
      const endLine = view.state.doc.lineAt(this.endPosition).number;
      const block = findDraftBlocks(view.state.doc).find(candidate => candidate.endLine === endLine);
      if (block) {
        action(view, block);
      }
    });
    return button;
  }
}

const markerLine = Decoration.line({ class: 'cm-draft-marker' });
const draftLine = Decoration.line({ class: 'cm-draft-line' });

function buildDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  for (const block of findDraftBlocks(state.doc)) {
    const start = state.doc.line(block.startLine);
    const end = state.doc.line(block.endLine);
    ranges.push(markerLine.range(start.from), markerLine.range(end.from));
    for (let number = block.startLine + 1; number < block.endLine; number++) {
      ranges.push(draftLine.range(state.doc.line(number).from));
    }
    ranges.push(Decoration.widget({ widget: new DraftActionsWidget(end.from), block: true, side: 1 }).range(end.to));
  }
  return Decoration.set(ranges, true);
}

// Block widgets can only come from a state field, not from a view plugin
const draftField = StateField.define<DecorationSet>({
  create: buildDecorations,
  update: (decorations, transaction) => (transaction.docChanged ? buildDecorations(transaction.state) : decorations),
  provide: field => EditorView.decorations.from(field),
});

const GIT_ADDED_GREEN = '#2ea043';
const DRAFT_BORDER = 'rgba(46, 160, 67, 0.45)';

export const draftBlocksTheme = EditorView.theme({
  // Marker lines stay in the document (they are the persisted state) but are not displayed
  '.cm-draft-marker': { height: '0', overflow: 'hidden', fontSize: '0', lineHeight: '0' },
  // More specific than the editor theme's `.cm-line { padding: 0 }`, which would otherwise cancel the gap
  '.cm-content .cm-line.cm-draft-line': {
    // Border flush with the text column; the gap sits between it and the text
    paddingLeft: '16px',
    borderLeft: `2px solid ${DRAFT_BORDER}`,
    background: 'rgba(46, 160, 67, 0.04)',
    filter: 'brightness(95%)',
  },
  '.cm-draft-actions': {
    display: 'flex',
    justifyContent: 'flex-end',
    gap: '6px',
    // Padding, not margin: CodeMirror measures block widgets without their margins (see the tables)
    padding: '4px 0 6px',
    fontFamily: 'inherit',
  },
  '.cm-draft-actions button': {
    font: 'inherit',
    fontSize: '12px',
    padding: '1px 8px',
    borderRadius: '3px',
    background: 'transparent',
    cursor: 'pointer',
    color: 'var(--daftr-text)',
    border: '1px solid var(--daftr-border)',
  },
  '.cm-draft-actions .cm-draft-confirm': { borderColor: DRAFT_BORDER, color: GIT_ADDED_GREEN },
  '.cm-draft-actions .cm-draft-confirm:hover': { borderColor: GIT_ADDED_GREEN, background: 'rgba(46, 160, 67, 0.12)' },
  '.cm-draft-actions .cm-draft-discard:hover': { borderColor: 'var(--daftr-danger)', color: 'var(--daftr-danger)' },
});

export const draftBlocks: Extension = [draftField, draftBlocksTheme];
