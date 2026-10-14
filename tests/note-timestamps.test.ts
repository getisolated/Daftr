import { describe, it, expect } from 'vitest';
import { EditorState } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import { joinBlocks, noteTimestamps, setTimestampBlocks } from '../src/app/shared/codemirror/note-timestamps';

const blocks = [
  { text: 'first', label: '09:00' },
  { text: 'second\nline two', label: '09:05' },
];

function decorate(): { classes: { from: number; cls: string }[]; widgets: number[] } {
  const base = EditorState.create({ doc: joinBlocks(blocks), extensions: [noteTimestamps] });
  const state = base.update({ effects: setTimestampBlocks.of(blocks) }).state;
  const set = state.field(noteTimestamps);
  const classes: { from: number; cls: string }[] = [];
  const widgets: number[] = [];
  set.between(0, state.doc.length, (from, _to, value) => {
    const spec = value.spec as { class?: string; widget?: unknown };
    if (spec.class) {
      classes.push({ from, cls: spec.class });
    } else if (spec.widget) {
      widgets.push(from);
    }
  });
  return { classes, widgets };
}

describe('joinBlocks', () => {
  it('separates notes with one blank line', () => {
    expect(joinBlocks(blocks)).toBe('first\n\nsecond\nline two');
  });
});

describe('noteTimestamps', () => {
  it('puts a timestamp widget at the start of each note', () => {
    expect(decorate().widgets).toEqual([0, 7]);
  });

  it('marks the first line of each note and the following lines', () => {
    expect(decorate().classes).toEqual([
      { from: 0, cls: 'cm-note-first' },
      { from: 7, cls: 'cm-note-first' },
      { from: 14, cls: 'cm-note-rest' },
    ]);
  });

  it('is empty without blocks', () => {
    const state = EditorState.create({ doc: 'x', extensions: [noteTimestamps] });
    expect(state.field(noteTimestamps)).toBe(Decoration.none);
  });
});
