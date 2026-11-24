import { describe, it, expect } from 'vitest';
import { ChangeSet, EditorSelection, EditorState, Transaction } from '@codemirror/state';
import { history, undo } from '@codemirror/commands';
import { diffText } from '../src/app/core/utils/text-diff.util';
import { remoteChanges, remoteTransaction } from '../src/app/shared/codemirror/remote-changes';

function apply(before: string, after: string): string {
  return ChangeSet.of(diffText(before, after), before.length).apply(EditorState.create({ doc: before }).doc).toString();
}

describe('diffText', () => {
  it('changes nothing between equal texts', () => {
    expect(diffText('same\ntext', 'same\ntext')).toEqual([]);
  });

  it('narrows a change to the characters that differ', () => {
    expect(diffText('# Title\nhello world\n', '# Title\nhello there world\n')).toEqual([{ from: 14, to: 14, insert: 'there ' }]);
  });

  it('keeps separate edits apart instead of one block covering the lines between them', () => {
    const before = 'one\ntwo\nthree\nfour\nfive\n';
    const after = 'ONE\ntwo\nthree\nfour\nfive!\n';
    const changes = diffText(before, after);

    expect(changes).toEqual([
      { from: 0, to: 3, insert: 'ONE' },
      { from: 23, to: 23, insert: '!' },
    ]);
    expect(apply(before, after)).toBe(after);
  });

  it('handles inserted and deleted lines, a missing final line break and empty texts', () => {
    const cases: [string, string][] = [
      ['a\nb\nc', 'a\nx\nb\nc'],
      ['a\nb\nc\n', 'a\nc\n'],
      ['a\nb', 'a\nb\n'],
      ['', 'new note'],
      ['gone', ''],
      ['x\ny\nz\n', 'z\ny\nx\n'],
    ];

    for (const [before, after] of cases) {
      expect(apply(before, after), JSON.stringify([before, after])).toBe(after);
    }
  });

  it('never cuts an emoji in two', () => {
    const changes = diffText('a😀b', 'a😁b');

    expect(changes).toEqual([{ from: 1, to: 3, insert: '😁' }]);
    expect(apply('😀😀', '😀😁😀')).toBe('😀😁😀');
  });

  it('converges on random edits (accents, emojis, CRLF)', () => {
    const pieces = ['é', '😀', '\r\n', '\n', 'a', 'bc ', '# T', '-'];
    let seed = 7;
    const random = (limit: number): number => {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      return seed % limit;
    };
    const text = (): string => Array.from({ length: random(40) }, () => pieces[random(pieces.length)]).join('');

    for (let i = 0; i < 200; i++) {
      const before = text();
      const after = text();
      expect(apply(before, after)).toBe(after);
    }
  });
});

describe('remoteChanges', () => {
  it('brings the remote text as is when the user typed nothing', () => {
    const changes = remoteChanges('base\n', 'base\n', 'base\nremote\n');

    expect(changes?.apply(EditorState.create({ doc: 'base\n' }).doc).toString()).toBe('base\nremote\n');
  });

  it('keeps the unsaved local edits next to the remote ones', () => {
    const base = 'first\nsecond\nthird\n';
    const local = 'first, typed here\nsecond\nthird\n';
    const remote = 'first\nsecond\nthird, from the laptop\n';
    const changes = remoteChanges(base, local, remote);

    expect(changes?.apply(EditorState.create({ doc: local }).doc).toString()).toBe('first, typed here\nsecond\nthird, from the laptop\n');
  });

  it('has nothing to apply when the remote text is the known one or already shown', () => {
    expect(remoteChanges('a', 'a b', 'a')).toBeNull();
    expect(remoteChanges('a', 'a b', 'a b')).toBeNull();
  });
});

describe('remoteTransaction', () => {
  it('maps the cursor and the selection instead of resetting them', () => {
    const doc = 'line one\nline two\nline three\n';
    const cursor = doc.indexOf('two');
    const state = EditorState.create({
      doc,
      selection: EditorSelection.create([EditorSelection.cursor(cursor), EditorSelection.range(0, 4)]),
      extensions: EditorState.allowMultipleSelections.of(true),
    });
    const spec = remoteTransaction(state, doc, `# Added on top\n${doc}line four\n`);
    const next = state.update(spec ?? {}).state;

    expect(next.doc.toString()).toBe('# Added on top\nline one\nline two\nline three\nline four\n');
    // The ranges are kept sorted: the selected "line" first, then the cursor
    expect(next.selection.ranges[0]).toMatchObject({ from: 15, to: 19 });
    expect(next.selection.ranges[1]?.head).toBe(cursor + '# Added on top\n'.length);
  });

  it('stays out of the undo history: Ctrl+Z undoes the user edit, not the remote one', () => {
    let state = EditorState.create({ doc: 'abc\n', extensions: history() });
    state = state.update({ changes: { from: 3, insert: ' typed' }, userEvent: 'input.type' }).state;
    const spec = remoteTransaction(state, 'abc\n', 'remote\nabc\n');
    const transaction = state.update(spec ?? {});

    expect(transaction.annotation(Transaction.remote)).toBe(true);
    state = transaction.state;
    expect(state.doc.toString()).toBe('remote\nabc typed\n');

    undo({ state, dispatch: tr => { state = tr.state; } });
    expect(state.doc.toString()).toBe('remote\nabc\n');
  });
});
