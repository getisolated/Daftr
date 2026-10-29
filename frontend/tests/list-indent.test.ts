import { describe, it, expect } from 'vitest';
import { EditorState, TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { ensureSyntaxTree } from '@codemirror/language';
import { indentListItems, outdentListItems, shiftListItem } from '../src/app/shared/codemirror/list-indent';

/** Minimal stand-in for EditorView (no DOM in the test environment): only state and dispatch are used. */
function createView(doc: string, anchor: number, head = anchor): EditorView {
  const view = {
    state: EditorState.create({ doc, selection: { anchor, head }, extensions: [markdown({ base: markdownLanguage })] }),
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
