import { describe, it, expect } from 'vitest';
import { EditorState, TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import {
  alignColumn, cellAt, createEmptyTable, insertColumn, insertRow, parseTable, removeColumn, removeRow, serializeTable,
  tsvToMarkdownTable,
} from '../src/app/shared/codemirror/markdown-table';
import { applyTableAction, describeTableAtCursor, formatTableAtCursor, moveToCell } from '../src/app/shared/codemirror/table-extension';

const SAMPLE = [
  '| Colonne 1 | Colonne 2 | Colonne 3 |',
  '|-----------|:---------:|----------:|',
  '| Gauche    |  Centre   |    Droite |',
  '| Cellule   |  Cellule  |   Cellule |',
].join('\n');

/** Minimal stand-in for EditorView (no DOM in the test environment): only state, dispatch and focus are used. */
function createView(doc: string, anchor: number): EditorView {
  const view = {
    state: EditorState.create({ doc, selection: { anchor }, extensions: [markdown({ base: markdownLanguage })] }),
    focus: () => undefined,
  };

  return Object.assign(view, {
    dispatch: (spec: TransactionSpec) => {
      view.state = view.state.update(spec).state;
    },
  }) as unknown as EditorView;
}

describe('table parsing', () => {
  it('reads header, alignments and rows', () => {
    const { model } = parseTable(SAMPLE)!;

    expect(model.header).toEqual(['Colonne 1', 'Colonne 2', 'Colonne 3']);
    expect(model.align).toEqual(['none', 'center', 'right']);
    expect(model.rows).toEqual([['Gauche', 'Centre', 'Droite'], ['Cellule', 'Cellule', 'Cellule']]);
  });

  it('rejects text without a matching delimiter row', () => {
    expect(parseTable('| a | b |\n| c | d |')).toBeNull();
    expect(parseTable('| a | b |\n|---|')).toBeNull();
    expect(parseTable('| a | b |')).toBeNull();
  });

  it('pads short rows, drops extra cells and keeps escaped pipes', () => {
    const { model } = parseTable('a | b\n--|--\n1\n2 | 3 | 4\n x \\| y | z')!;

    expect(model.rows).toEqual([['1', ''], ['2', '3'], ['x \\| y', 'z']]);
  });

  it('locates the cell under an offset', () => {
    const parsed = parseTable(SAMPLE)!;

    expect(cellAt(parsed, 2)).toEqual({ row: 0, col: 0 });
    expect(cellAt(parsed, SAMPLE.indexOf('Centre'))).toEqual({ row: 1, col: 1 });
    expect(cellAt(parsed, SAMPLE.lastIndexOf('Cellule'))).toEqual({ row: 2, col: 2 });
    expect(cellAt(parsed, SAMPLE.indexOf(':---'))).toEqual({ row: 0, col: 1 });
  });
});

describe('table formatting', () => {
  it('is idempotent on an aligned table', () => {
    expect(serializeTable(parseTable(SAMPLE)!.model).text).toBe(SAMPLE);
  });

  it('aligns ragged pipes and honours column alignment', () => {
    const ragged = '|a|b|\n|:-|-:|\n|longer cell|x|';
    const formatted = serializeTable(parseTable(ragged)!.model).text;

    expect(formatted).toBe([
      '| a           |   b |',
      '|:------------|----:|',
      '| longer cell |   x |',
    ].join('\n'));
  });

  it('reports cell offsets in the serialized text', () => {
    const { text, cells } = serializeTable(parseTable(SAMPLE)!.model);

    expect(text.slice(cells[1]![1]!.from, cells[1]![1]!.to)).toBe('Centre');
    expect(text.slice(cells[0]![2]!.from, cells[0]![2]!.to)).toBe('Colonne 3');
  });

  it('creates an empty 2×2 table', () => {
    expect(createEmptyTable().text).toBe('|     |     |\n|-----|-----|\n|     |     |');
  });
});

describe('table edition', () => {
  const model = parseTable(SAMPLE)!.model;

  it('inserts and removes rows', () => {
    expect(insertRow(model, 1).rows).toEqual([model.rows[0], ['', '', ''], model.rows[1]]);
    expect(removeRow(model, 0).rows).toEqual([model.rows[1]]);
  });

  it('inserts and removes columns, alignment included', () => {
    const added = insertColumn(model, 1);

    expect(added.header).toEqual(['Colonne 1', '', 'Colonne 2', 'Colonne 3']);
    expect(added.align).toEqual(['none', 'none', 'center', 'right']);

    const removed = removeColumn(model, 1);

    expect(removed.header).toEqual(['Colonne 1', 'Colonne 3']);
    expect(removed.align).toEqual(['none', 'right']);
    expect(removed.rows[0]).toEqual(['Gauche', 'Droite']);
  });

  it('aligns a single column', () => {
    expect(alignColumn(model, 0, 'left').align).toEqual(['left', 'center', 'right']);
  });
});

describe('table actions on the editor', () => {
  it('describes the cursor cell and formats the table in place', () => {
    const doc = '| a | b |\n|---|---|\n| c | d |';
    const view = createView(doc, doc.length - 3);

    expect(describeTableAtCursor(view.state)).toEqual({ row: 1, col: 1, rowCount: 2, columnCount: 2 });
    formatTableAtCursor(view);
    expect(view.state.doc.toString()).toBe('| a   | b   |\n|-----|-----|\n| c   | d   |');
    // The caret stays in the same cell, at the same place
    expect(view.state.selection.main.head).toBe(view.state.doc.toString().indexOf('d'));
  });

  it('Tab / Shift-Tab move between cells and Tab on the last cell appends a row', () => {
    const doc = '| a | b |\n|---|---|\n| c | d |';
    const view = createView(doc, 2);
    const selected = (): string => view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to);

    moveToCell(view, 1);
    expect(selected()).toBe('b');
    moveToCell(view, 1);
    expect(selected()).toBe('c');
    moveToCell(view, -1);
    expect(selected()).toBe('b');
    moveToCell(view, 1);
    moveToCell(view, 1);
    expect(describeTableAtCursor(view.state)).toMatchObject({ row: 1, col: 1 });
    moveToCell(view, 1);
    expect(describeTableAtCursor(view.state)).toMatchObject({ row: 2, col: 0, rowCount: 3 });
    expect(view.state.doc.toString().split('\n')).toHaveLength(4);
  });

  it('adds and deletes rows and columns from the cursor cell', () => {
    const doc = '| a | b |\n|---|---|\n| c | d |';
    const view = createView(doc, 2);

    applyTableAction(view, 'addRow');
    expect(view.state.doc.toString()).toBe('| a   | b   |\n|-----|-----|\n|     |     |\n| c   | d   |');
    expect(describeTableAtCursor(view.state)).toMatchObject({ row: 1, col: 0 });

    applyTableAction(view, 'deleteRow');
    applyTableAction(view, 'addColumn');
    expect(view.state.doc.toString().split('\n')[0]).toBe('| a   |     | b   |');

    applyTableAction(view, 'alignCenter');
    expect(view.state.doc.toString().split('\n')[1]).toBe('|-----|:---:|-----|');
  });

  it('refuses to delete the header row', () => {
    const view = createView('| a | b |\n|---|---|\n| c | d |', 2);

    expect(applyTableAction(view, 'deleteRow')).toBe(false);
  });

  it('recognises the empty table inserted by /table', () => {
    const view = createView(`intro

${createEmptyTable().text}`, 8);

    expect(describeTableAtCursor(view.state)).toMatchObject({ row: 0, col: 0, rowCount: 2, columnCount: 2 });
  });

  it('ignores a cursor outside of any table', () => {
    const view = createView('text\n\n| a | b |\n|---|---|\n| c | d |', 1);

    expect(describeTableAtCursor(view.state)).toBeNull();
    expect(formatTableAtCursor(view)).toBe(false);
  });
});

describe('spreadsheet paste', () => {
  it('converts a rectangular TSV block', () => {
    expect(tsvToMarkdownTable('Nom\tÂge\nAda\t36\nLinus\t54\n')).toBe([
      '| Nom   | Âge |',
      '|-------|-----|',
      '| Ada   | 36  |',
      '| Linus | 54  |',
    ].join('\n'));
  });

  it('escapes pipes and converts quoted multi-line cells', () => {
    const table = tsvToMarkdownTable('a\tb\r\n"x\ny"\tp|q\r\n')!;

    expect(parseTable(table)!.model.rows).toEqual([['x<br>y', 'p\\|q']]);
  });

  it('leaves ordinary text and ragged data alone', () => {
    expect(tsvToMarkdownTable('no tab here')).toBeNull();
    expect(tsvToMarkdownTable('one\ttwo')).toBeNull();
    expect(tsvToMarkdownTable('a\tb\nc')).toBeNull();
  });
});
