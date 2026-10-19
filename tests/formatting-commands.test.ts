import { describe, it, expect } from 'vitest';
import { EditorState, Extension, TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { ensureSyntaxTree } from '@codemirror/language';
import {
  FORMAT_ACTIONS, clearInlineFormatting, detectActiveFormats, findTagAround, insertLink, insertRuleBelow, insertTableBelow,
  stripInlineFormatting, toggleCodeBlock, toggleHtmlTag, toggleInlineMark, toggleLineKind,
} from '../src/app/shared/codemirror/formatting-commands';
import { slashCommands } from '../src/app/shared/codemirror/slash-commands';
import { translatorFacet } from '../src/app/shared/codemirror/translator';
import { translate } from '../src/app/core/i18n/translate';
import { LANGUAGES } from '../src/app/core/i18n/language.model';

/** Minimal stand-in for EditorView (no DOM in the test environment): only state, dispatch and focus are used. */
function createView(doc: string, anchor: number, head = anchor, extensions: Extension[] = []): EditorView {
  const view = {
    state: EditorState.create({ doc, selection: { anchor, head }, extensions: [markdown({ base: markdownLanguage }), ...extensions] }),
    dispatch(spec: TransactionSpec) { view.state = view.state.update(spec).state; },
    focus() { /* no DOM */ },
  };
  ensureSyntaxTree(view.state, view.state.doc.length, 5000);
  return view as unknown as EditorView;
}

const text = (view: EditorView): string => view.state.doc.toString();

describe('toggleLineKind', () => {
  it('turns a line into a heading, and back when applied again', () => {
    const view = createView('Titre', 2);
    toggleLineKind(view, 'h2');
    expect(text(view)).toBe('## Titre');
    toggleLineKind(view, 'h2');
    expect(text(view)).toBe('Titre');
  });

  it('replaces another heading level', () => {
    const view = createView('# Titre', 3);
    toggleLineKind(view, 'h3');
    expect(text(view)).toBe('### Titre');
  });

  it('numbers an ordered list over several lines and skips blank ones', () => {
    const view = createView('a\n\nb\nc', 0, 6);
    toggleLineKind(view, 'orderedList');
    expect(text(view)).toBe('1. a\n\n2. b\n3. c');
  });

  it('switches a bullet to a task, then removes it', () => {
    const view = createView('- a', 3);
    toggleLineKind(view, 'taskList');
    expect(text(view)).toBe('- [ ] a');
    toggleLineKind(view, 'taskList');
    expect(text(view)).toBe('a');
  });

  it('keeps the indentation of a nested item', () => {
    const view = createView('  a', 3);
    toggleLineKind(view, 'bulletList');
    expect(text(view)).toBe('  - a');
  });

  it('resets quotes and headings with the paragraph style', () => {
    const view = createView('> # Titre', 4);
    toggleLineKind(view, 'paragraph');
    expect(text(view)).toBe('Titre');
  });
});

describe('toggleInlineMark', () => {
  it('wraps the selection and keeps it selected', () => {
    const view = createView('un mot', 3, 6);
    toggleInlineMark(view, '**', 'StrongEmphasis');
    expect(text(view)).toBe('un **mot**');
    expect(view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to)).toBe('mot');
  });

  it('inserts a pair of marks around an empty caret', () => {
    const view = createView('', 0);
    toggleInlineMark(view, '`', 'InlineCode');
    expect(text(view)).toBe('``');
    expect(view.state.selection.main.head).toBe(1);
  });

  it('unwraps when the caret is already inside the mark', () => {
    const view = createView('un **mot** ici', 7);
    toggleInlineMark(view, '**', 'StrongEmphasis');
    expect(text(view)).toBe('un mot ici');
  });

  it('unwraps an italic written with underscores', () => {
    const view = createView('_mot_', 2);
    toggleInlineMark(view, '*', 'Emphasis');
    expect(text(view)).toBe('mot');
  });
});

describe('toggleCodeBlock', () => {
  it('fences the selected lines', () => {
    const view = createView('a\nb', 0, 3);
    toggleCodeBlock(view);
    expect(text(view)).toBe('```\na\nb\n```');
  });

  it('inserts an empty block on a blank line, caret inside', () => {
    const view = createView('', 0);
    toggleCodeBlock(view);
    expect(text(view)).toBe('```\n\n```');
    expect(view.state.selection.main.head).toBe(4);
  });

  it('removes the fences from the block under the caret', () => {
    const view = createView('avant\n```js\ncode\n```\naprès', 14);
    toggleCodeBlock(view);
    expect(text(view)).toBe('avant\ncode\naprès');
  });

  it('removes an empty block at the end of the document', () => {
    const view = createView('a\n```\n```', 5);
    toggleCodeBlock(view);
    expect(text(view)).toBe('a');
  });
});

describe('insertRuleBelow', () => {
  it('leaves a blank line so the rule does not turn the text into a heading', () => {
    const view = createView('texte', 2);
    insertRuleBelow(view);
    expect(text(view)).toBe('texte\n\n---\n');
  });

  it('replaces a blank line', () => {
    const view = createView('', 0);
    insertRuleBelow(view);
    expect(text(view)).toBe('---\n');
  });
});

describe('detectActiveFormats', () => {
  const formats = (doc: string, pos: number): string[] => [...detectActiveFormats(createView(doc, pos).state)].sort();

  it('reports a paragraph on plain text', () => {
    expect(formats('texte', 2)).toEqual(['paragraph']);
  });

  it('reports bold and italic nested', () => {
    expect(formats('***mot***', 5)).toEqual(expect.arrayContaining(['bold', 'italic']));
  });

  it('reports the heading level, also with the caret at the start of the line', () => {
    expect(formats('# a\n\n## Titre', 5)).toEqual(['h2']);
  });

  it('reports inline code and strikethrough', () => {
    expect(formats('`x`', 2)).toEqual(['inlineCode']);
    expect(formats('~~x~~', 3)).toEqual(['strikethrough']);
  });

  it('reports lists, with tasks distinct from bullets', () => {
    expect(formats('- a', 3)).toEqual(['bulletList']);
    expect(formats('1. a', 4)).toEqual(['orderedList']);
    expect(formats('- [ ] a', 7)).toEqual(['taskList']);
  });

  it('reports quotes and code blocks', () => {
    expect(formats('> a', 3)).toEqual(expect.arrayContaining(['quote']));
    expect(formats('```\ncode\n```', 6)).toEqual(['codeBlock']);
  });
});

describe('table and draft actions', () => {
  it('inserts a table below the current line, after a blank line', () => {
    const view = createView('text', 2);
    insertTableBelow(view);
    expect(view.state.doc.toString().startsWith('text\n\n|')).toBe(true);
  });

  it('inserts the table in place on a blank line', () => {
    const view = createView('', 0);
    insertTableBelow(view);
    expect(view.state.doc.toString().startsWith('|')).toBe(true);
  });

  it('does not nest a table in a table', () => {
    const view = createView('| a | b |\n| --- | --- |\n| c | d |', 3);
    const before = view.state.doc.toString();
    insertTableBelow(view);
    expect(view.state.doc.toString()).toBe(before);
    expect(detectActiveFormats(view.state).has('table')).toBe(true);
  });

  it('reports the draft block under the caret', () => {
    const doc = '<!-- draft -->\nwip\n<!-- /draft -->\nafter';
    expect(detectActiveFormats(createView(doc, 16).state).has('draft')).toBe(true);
    expect(detectActiveFormats(createView(doc, 40).state).has('draft')).toBe(false);
  });
});

describe('headings 5 and 6', () => {
  it('applies and detects the deepest levels', () => {
    const view = createView('Titre', 2);
    toggleLineKind(view, 'h6');
    expect(text(view)).toBe('###### Titre');
    expect(detectActiveFormats(view.state).has('h6')).toBe(true);
    toggleLineKind(view, 'h5');
    expect(text(view)).toBe('##### Titre');
    expect(detectActiveFormats(view.state).has('h5')).toBe(true);
  });
});

describe('findTagAround', () => {
  it('finds the tags enclosing a position or a span', () => {
    const line = 'a <u>bc</u> d';
    expect(findTagAround(line, 'u', 6)).toEqual({ open: { from: 2, to: 5 }, close: { from: 7, to: 11 } });
    expect(findTagAround(line, 'u', 5, 7)).not.toBeNull();
  });

  it('returns null outside the tags, or for an unclosed tag', () => {
    expect(findTagAround('a <u>bc</u> d', 'u', 12)).toBeNull();
    expect(findTagAround('a <u>bc</u> d', 'u', 1)).toBeNull();
    expect(findTagAround('<u>bc', 'u', 4)).toBeNull();
  });

  it('picks the pair that still encloses the span when another one closes before it', () => {
    const line = '<u>a</u> <u>b</u>';
    expect(findTagAround(line, 'u', 13)?.open.from).toBe(9);
  });
});

describe('toggleHtmlTag', () => {
  it('wraps the selection, keeping it selected inside the tags', () => {
    const view = createView('mot', 0, 3);
    toggleHtmlTag(view, 'sup');
    expect(text(view)).toBe('<sup>mot</sup>');
    expect(view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to)).toBe('mot');
  });

  it('unwraps when the caret is inside the tags', () => {
    const view = createView('a <u>mot</u> b', 6);
    toggleHtmlTag(view, 'u');
    expect(text(view)).toBe('a mot b');
  });

  it('puts the caret between the tags on an empty selection', () => {
    const view = createView('', 0);
    toggleHtmlTag(view, 'sub');
    expect(text(view)).toBe('<sub></sub>');
    expect(view.state.selection.main.head).toBe(5);
  });

  it('is reported as active under the caret', () => {
    const active = detectActiveFormats(createView('x <u>a</u> <sup>b</sup> <sub>c</sub>', 6).state);
    expect(active.has('underline')).toBe(true);
    expect(active.has('superscript')).toBe(false);
  });
});

describe('stripInlineFormatting', () => {
  it('removes bold, italic, strike, code and inline HTML styling', () => {
    expect(stripInlineFormatting('**a** *b* _c_ ~~d~~ `e` <u>f</u> <sup>g</sup> <sub>h</sub> __i__'))
      .toBe('a b c d e f g h i');
  });

  it('keeps links, snake_case words and lone markers', () => {
    expect(stripInlineFormatting('[**x**](url) snake_case_name 2 * 3')).toBe('[x](url) snake_case_name 2 * 3');
  });
});

describe('clearInlineFormatting', () => {
  it('clears the whole caret line past its block prefix when nothing is selected', () => {
    const view = createView('- [ ] **gras** et *italique*', 3);
    clearInlineFormatting(view);
    expect(text(view)).toBe('- [ ] gras et italique');
  });

  it('only clears the selection when there is one', () => {
    const view = createView('**a** **b**', 0, 5);
    clearInlineFormatting(view);
    expect(text(view)).toBe('a **b**');
  });

  it('keeps a heading marker', () => {
    const view = createView('## *Titre*', 0);
    clearInlineFormatting(view);
    expect(text(view)).toBe('## Titre');
  });
});

describe('insertLink', () => {
  const selected = (view: EditorView): string => view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to);

  it('uses the selection as the label and selects the url placeholder', () => {
    const view = createView('voir doc', 5, 8);
    insertLink(view);
    expect(text(view)).toBe('voir [doc](url)');
    expect(selected(view)).toBe('url');
  });

  it('inserts an image with a placeholder label', () => {
    const view = createView('', 0);
    insertLink(view, true);
    expect(text(view)).toBe('![image](url)');
    expect(selected(view)).toBe('url');
  });

  it('writes the placeholder label in the language of the editor', () => {
    const view = createView('', 0, 0, [translatorFacet.of((key, params) => translate('fr', key, params))]);
    insertLink(view, true);
    expect(text(view)).toBe('![image](url)');
    const link = createView('', 0, 0, [translatorFacet.of((key, params) => translate('fr', key, params))]);
    insertLink(link);
    expect(text(link)).toBe('[texte](url)');
  });
});

describe('translated labels of the actions', () => {
  it('give each formatting action and slash command a label and a description in every language', () => {
    for (const { id: language } of LANGUAGES) {
      for (const action of Object.values(FORMAT_ACTIONS)) {
        expect(translate(language, action.labelKey), `${language}:${action.labelKey}`).not.toBe('');
        expect(translate(language, action.descriptionKey), `${language}:${action.descriptionKey}`).not.toBe('');
      }
      for (const command of slashCommands) {
        expect(translate(language, command.groupKey)).not.toBe('');
      }
    }
  });

  it('are in French in the French dictionary', () => {
    expect(translate('fr', FORMAT_ACTIONS.bold.labelKey)).toBe('Gras');
    expect(translate('en', FORMAT_ACTIONS.bold.labelKey)).toBe('Bold');
  });
});
