import { describe, it, expect } from 'vitest';
import { EditorState, TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { deleteMarkupBackward, insertNewlineContinueMarkup, markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { ensureSyntaxTree } from '@codemirror/language';
import { deleteCharBackward, deleteCharForward, history, undo } from '@codemirror/commands';
import {
  deletedListStarts, indentListItems, orderedListRenumbering, outdentListItems, renumberListsFrom, shiftListItem,
} from '../src/app/shared/codemirror/list-indent';

/** Minimal stand-in for EditorView (no DOM in the test environment): only state and dispatch are used. */
function createView(doc: string, anchor: number, head = anchor): EditorView {
  const view = {
    state: EditorState.create({
      doc,
      selection: { anchor, head },
      extensions: [markdown({ base: markdownLanguage }), history(), orderedListRenumbering],
    }),
    dispatch(spec: TransactionSpec) { view.state = view.state.update(spec).state; },
  };
  ensureSyntaxTree(view.state, view.state.doc.length, 5000);
  return view as unknown as EditorView;
}

function shift(doc: string, index: number, direction: 1 | -1): string {
  const lines = doc.split('\n');
  shiftListItem(lines, index, direction);
  return lines.join('\n');
}

describe('shiftListItem', () => {
  it('nests the item under the previous one, numbered from 1, and renumbers the items it leaves', () => {
    expect(shift('1. a\n2. b\n3. c', 1, 1)).toBe('1. a\n   1. b\n2. c');
  });

  it('continues the numbering of a sub-list already there', () => {
    expect(shift('1. a\n   1. x\n2. b\n3. c', 2, 1)).toBe('1. a\n   1. x\n   2. b\n2. c');
  });

  it('brings the item back to its parent level, after the parent, and renumbers both levels', () => {
    expect(shift('1. a\n   1. b\n   2. c\n2. d', 1, -1)).toBe('1. a\n2. b\n   1. c\n3. d');
  });

  it('indents to the text column of a wider marker', () => {
    expect(shift('10. a\n11. b', 1, 1)).toBe('10. a\n    1. b');
  });

  it('keeps the delimiter and the text', () => {
    expect(shift('1) a\n2) **b**', 1, 1)).toBe('1) a\n   1) **b**');
  });

  it('nests bullets under the text of the previous item', () => {
    expect(shift('1. a\n- b', 1, 1)).toBe('1. a\n   - b');
  });

  it('does not move the first item of a list, nor a top-level item back', () => {
    expect(shiftListItem(['1. a', '2. b'], 0, 1)).toBe(false);
    expect(shiftListItem(['1. a', '2. b'], 1, -1)).toBe(false);
    expect(shiftListItem(['texte'], 0, 1)).toBe(false);
  });
});

describe('indentListItems / outdentListItems', () => {
  it('rewrites only the start of the line: the cursor stays at the same place in the text', () => {
    const view = createView('1. a\n2. bravo', 12);
    expect(indentListItems(view)).toBe(true);
    expect(view.state.doc.toString()).toBe('1. a\n   1. bravo');
    expect(view.state.selection.main.head).toBe(view.state.doc.length - 1);
  });

  it('moves every selected item, top to bottom', () => {
    const doc = '1. a\n2. b\n3. c\n4. d';
    const view = createView(doc, doc.indexOf('2.'), doc.indexOf('3. c') + 4);
    indentListItems(view);
    expect(view.state.doc.toString()).toBe('1. a\n   1. b\n   2. c\n2. d');
    outdentListItems(view);
    expect(view.state.doc.toString()).toBe(doc);
  });

  it('lets the default Tab run outside lists and in code blocks', () => {
    expect(indentListItems(createView('texte', 2))).toBe(false);
    expect(indentListItems(createView('```\n1. a\n2. b\n```', 10))).toBe(false);
    expect(indentListItems(createView('1. a\n2. b', 1))).toBe(false);
  });
});

function renumber(doc: string, index: number, starts?: Map<number, number>): string {
  const lines = doc.split('\n');
  renumberListsFrom(lines, index, starts);
  return lines.join('\n');
}

describe('renumberListsFrom', () => {
  it('closes the gap left by a deleted item', () => {
    expect(renumber('1. a\n2. b\n4. d\n5. e', 2)).toBe('1. a\n2. b\n3. d\n4. e');
  });

  it('keeps the number the list starts at and its delimiter', () => {
    expect(renumber('3) a\n5) c\n6) d', 1)).toBe('3) a\n4) c\n5) d');
  });

  it('gives the start of the list to the item that now begins it', () => {
    expect(renumber('2. b\n3. c', 0, new Map([[0, 1]]))).toBe('1. b\n2. c');
    expect(renumber('2. b\n3. c', 0)).toBe('2. b\n3. c');
  });

  it('renumbers the sub-list alone, the parent list being separate', () => {
    expect(renumber('1. a\n   1. x\n   3. z\n2. b', 2)).toBe('1. a\n   1. x\n   2. z\n2. b');
  });

  it('renumbers the enclosing list once out of the sub-items, without touching the other sub-lists', () => {
    expect(renumber('1. a\n   1. x\n   2. y\n3. c\n   5. p\n4. d', 2)).toBe('1. a\n   1. x\n   2. y\n2. c\n   5. p\n3. d');
  });

  it('follows the list below what remains of an item whose marker was deleted', () => {
    expect(renumber('1. a\n   b\n3. c', 1)).toBe('1. a\n   b\n2. c');
  });

  it('stops where the list ends', () => {
    expect(renumber('1. a\n3. b\n\ntexte\n\n5. x', 1)).toBe('1. a\n2. b\n\ntexte\n\n5. x');
    expect(renumber('1. a\n- b\n3. c', 1)).toBe('1. a\n- b\n3. c');
  });

  it('leaves lazy numbering as written', () => {
    expect(renumber('1. a\n1. c\n1. d', 1)).toBe('1. a\n1. c\n1. d');
  });
});

describe('deletedListStarts', () => {
  it('gives the number of a deleted first item, by indentation', () => {
    expect(deletedListStarts(['3. a', '4. b'], 0)).toEqual(new Map([[0, 3]]));
    expect(deletedListStarts(['intro', '1. a', '   2. x'], 5)).toEqual(new Map([[0, 1], [3, 2]]));
  });

  it('ignores an item that continues a list, or whose marker was not deleted', () => {
    expect(deletedListStarts(['1. aa', '2. b'], 4)).toEqual(new Map());
    expect(deletedListStarts(['2.'], 1)).toEqual(new Map());
    expect(deletedListStarts(['2'], 0)).toEqual(new Map());
  });

  it('takes the list begun after a paragraph or a bullet', () => {
    expect(deletedListStarts(['1. a', 'texte', '5. x'], 0)).toEqual(new Map([[0, 5]]));
    expect(deletedListStarts(['1. a', '- b', '5. x'], 0)).toEqual(new Map([[0, 5]]));
  });
});

describe('orderedListRenumbering', () => {
  const list = '1. a\n2. b\n3. c\n4. d';

  function apply(doc: string, from: number, to: number, userEvent = 'delete.selection'): EditorView {
    const view = createView(doc, from);
    view.dispatch({ changes: { from, to }, userEvent });
    return view;
  }

  it('renumbers after a line deletion', () => {
    // Like deleteLine: the line break before the line goes with it
    expect(apply(list, list.indexOf('\n2.'), list.indexOf('\n3.'), 'delete.line').state.doc.toString()).toBe('1. a\n2. c\n3. d');
  });

  it('keeps the start of the list when its first item is deleted or cut', () => {
    expect(apply('3. a\n4. b\n5. c', 0, 5, 'delete.cut').state.doc.toString()).toBe('3. b\n4. c');
  });

  it('renumbers after a multi-line selection is deleted', () => {
    expect(apply(list, list.indexOf('a') + 1, list.indexOf('c') + 1).state.doc.toString()).toBe('1. a\n2. d');
  });

  it('renumbers when Backspace / Delete join two items', () => {
    const view = createView(list, list.indexOf('2.'));
    deleteCharBackward(view);
    expect(view.state.doc.toString()).toBe('1. a2. b\n2. c\n3. d');

    const forward = createView(list, list.indexOf('\n3.'));
    deleteCharForward(forward);
    expect(forward.state.doc.toString()).toBe('1. a\n2. b3. c\n3. d');
  });

  it('renumbers when Backspace removes the marker of an item', () => {
    const view = createView(list, list.indexOf('b'));
    deleteMarkupBackward(view);
    expect(view.state.doc.toString()).toBe('1. a\n   b\n2. c\n3. d');
  });

  it('keeps the cursor in the text of the renumbered item', () => {
    const view = apply(list, list.indexOf('\n2.') + 1, list.indexOf('\n3.') + 1, 'delete.line');
    expect(view.state.doc.toString()).toBe('1. a\n2. c\n3. d');
    expect(view.state.selection.main.head).toBe(list.indexOf('\n2.') + 1);
  });

  it('undoes the deletion and the renumbering at once', () => {
    const view = apply(list, list.indexOf('\n2.'), list.indexOf('\n3.'), 'delete.line');
    undo(view);
    expect(view.state.doc.toString()).toBe(list);
  });

  it('leaves the text of the items, the numbers being edited and the code blocks alone', () => {
    expect(apply(list, list.indexOf('b'), list.indexOf('b') + 1, 'delete.backward').state.doc.toString()).toBe('1. a\n2. \n3. c\n4. d');
    expect(apply(list, list.indexOf('2.') + 1, list.indexOf('2.') + 2, 'delete.backward').state.doc.toString()).toBe('1. a\n2 b\n3. c\n4. d');
    const code = '```\n1. a\n2. b\n3. c\n```';
    expect(apply(code, code.indexOf('\n2.'), code.indexOf('\n3.'), 'delete.line').state.doc.toString()).toBe('```\n1. a\n3. c\n```');
  });

  it('ignores the changes that are not deletions by the user', () => {
    expect(apply(list, list.indexOf('\n2.'), list.indexOf('\n3.'), 'input.paste').state.doc.toString()).toBe('1. a\n3. c\n4. d');
  });

  it('lets Enter renumber the items below the inserted one', () => {
    const view = createView(list, list.indexOf('\n2.'));
    insertNewlineContinueMarkup(view);
    expect(view.state.doc.toString()).toBe('1. a\n2. \n3. b\n4. c\n5. d');
  });
});
