import { RangeSetBuilder, StateEffect, StateField } from '@codemirror/state';
import { Decoration, DecorationSet, EditorView, WidgetType } from '@codemirror/view';

export interface TimestampedBlock {
  /** Text of the note, as it appears in the document. */
  text: string;
  /** Already formatted timestamp. */
  label: string;
}

/** Separator between two notes in the concatenated document: one blank line. */
export const NOTE_SEPARATOR = '\n\n';

/** Builds the document text for the blocks (their offsets are what `setTimestampBlocks` decorates). */
export function joinBlocks(blocks: TimestampedBlock[]): string {
  return blocks.map(block => block.text).join(NOTE_SEPARATOR);
}

class TimestampWidget extends WidgetType {
  constructor(private readonly label: string) {
    super();
  }

  override eq(other: TimestampWidget): boolean {
    return other.label === this.label;
  }

  override toDOM(): HTMLElement {
    const element = document.createElement('span');
    element.className = 'cm-note-time';
    element.textContent = this.label;
    return element;
  }
}

const firstLine = Decoration.line({ class: 'cm-note-first' });
const otherLine = Decoration.line({ class: 'cm-note-rest' });

function buildDecorations(view: EditorView['state'], blocks: TimestampedBlock[]): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  let offset = 0;
  for (const block of blocks) {
    const start = view.doc.lineAt(offset);
    builder.add(start.from, start.from, firstLine);
    builder.add(start.from, start.from, Decoration.widget({ widget: new TimestampWidget(block.label), side: -1 }));
    const end = offset + block.text.length;
    for (let line = start.number + 1; line <= view.doc.lineAt(end).number; line++) {
      const from = view.doc.line(line).from;
      builder.add(from, from, otherLine);
    }
    offset = end + NOTE_SEPARATOR.length;
  }
  return builder.finish();
}

/** Dispatch together with the document change: the blocks describe the NEW document. */
export const setTimestampBlocks = StateEffect.define<TimestampedBlock[]>();

/**
 * Shows each note's timestamp, muted, to the left of its first line; wrapped and following lines keep the
 * same left indent so the notes read as an aligned column.
 */
export const noteTimestamps = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setTimestampBlocks)) {
        return buildDecorations(transaction.state, effect.value);
      }
    }
    return decorations.map(transaction.changes);
  },
  provide: field => EditorView.decorations.from(field),
});

export const noteTimestampsTheme = EditorView.baseTheme({
  '.cm-line.cm-note-first': { paddingLeft: '150px', textIndent: '-150px' },
  '.cm-line.cm-note-rest': { paddingLeft: '150px' },
  '.cm-note-time': {
    display: 'inline-block',
    width: '150px',
    textIndent: '0',
    color: 'var(--evn-syntax-timestamp)',
    // Not the text I-beam: the timestamp is not part of the (read-only) text
    cursor: 'default',
    userSelect: 'none',
    fontSize: '11px',
    verticalAlign: 'baseline',
  },
});
