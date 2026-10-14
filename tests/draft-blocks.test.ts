import { describe, it, expect } from 'vitest';
import { EditorState, TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { confirmDraft, discardDraft, findDraftBlocks, toggleDraft } from '../src/app/shared/codemirror/draft-blocks';

/** Minimal stand-in for EditorView (no DOM in the test environment): only state and dispatch are used. */
function createView(doc: string, anchor: number, head = anchor): EditorView {
  const view = { state: EditorState.create({ doc, selection: { anchor, head } }) };
  return Object.assign(view, {
    dispatch: (spec: TransactionSpec) => {
      view.state = view.state.update(spec).state;
    },
  }) as unknown as EditorView;
}

describe('draft blocks', () => {
  it('finds complete blocks only', () => {
    const doc = EditorState.create({ doc: 'a\n<!-- draft -->\nb\n<!-- /draft -->\n<!-- draft -->\nc' }).doc;
    expect(findDraftBlocks(doc)).toEqual([{ startLine: 2, endLine: 4 }]);
  });

  it('wraps the selected lines, then unwraps when the cursor is inside', () => {
    const view = createView('a\nb\nc', 0, 3);
    toggleDraft(view);
    expect(view.state.doc.toString()).toBe('<!-- draft -->\na\nb\n<!-- /draft -->\nc');
    view.dispatch({ selection: { anchor: 16 } });
    toggleDraft(view);
    expect(view.state.doc.toString()).toBe('a\nb\nc');
  });

  it('confirm keeps the content', () => {
    const view = createView('x\n<!-- draft -->\nb\n<!-- /draft -->\ny', 0);
    confirmDraft(view, { startLine: 2, endLine: 4 });
    expect(view.state.doc.toString()).toBe('x\nb\ny');
  });

  it('discard removes the block and its markers', () => {
    const view = createView('x\n<!-- draft -->\nb\n<!-- /draft -->\ny', 0);
    discardDraft(view, { startLine: 2, endLine: 4 });
    expect(view.state.doc.toString()).toBe('x\ny');
  });

  it('discard at the start of the document', () => {
    const view = createView('<!-- draft -->\nb\n<!-- /draft -->\ny', 0);
    discardDraft(view, { startLine: 1, endLine: 3 });
    expect(view.state.doc.toString()).toBe('y');
  });
});
