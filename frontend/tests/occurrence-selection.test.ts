import { describe, it, expect } from 'vitest';
import { EditorSelection, EditorState, Transaction } from '@codemirror/state';
import {
  DEFAULT_OCCURRENCE_OPTIONS, OccurrenceOptions, isWholeWord, occurrenceSelection, occurrences, selectAllMatches,
  selectNextMatch, toggleOccurrenceOption,
} from '../src/app/shared/codemirror/occurrence-selection';

interface Target {
  state: EditorState;
  dispatch: (tr: Transaction) => void;
}

/** Stand-in for the view (no DOM in the test environment): the commands only use state and dispatch. */
function createTarget(doc: string, selection: EditorSelection, options = DEFAULT_OCCURRENCE_OPTIONS): Target {
  const target: Target = {
    state: EditorState.create({
      doc,
      selection,
      extensions: [EditorState.allowMultipleSelections.of(true), occurrenceSelection(options, () => undefined)],
    }),
    dispatch: tr => {
      target.state = tr.state;
    },
  };
  return target;
}

const selected = (target: Target): string[] => target.state.selection.ranges.map(range => target.state.sliceDoc(range.from, range.to));
const starts = (target: Target): number[] => target.state.selection.ranges.map(range => range.from);
const looseOptions: OccurrenceOptions = { caseSensitive: false, wholeWord: false };

describe('occurrences', () => {
  const state = EditorState.create({ doc: 'Le lé table le' });
  const found = (options: OccurrenceOptions): number[] => [...occurrences(state, 'le', options)].map(match => match.from);

  it('compares accents exactly', () => {
    expect(found(looseOptions)).toEqual([0, 9, 12]);
  });

  it('follows the case and whole word options', () => {
    expect(found({ caseSensitive: true, wholeWord: false })).toEqual([9, 12]);
    expect(found({ caseSensitive: false, wholeWord: true })).toEqual([0, 12]);
    expect(found({ caseSensitive: true, wholeWord: true })).toEqual([12]);
  });

  it('matches the regexp characters literally', () => {
    const text = EditorState.create({ doc: 'a.b axb (a.b)' });
    expect([...occurrences(text, 'a.b', looseOptions)].map(match => match.from)).toEqual([0, 9]);
  });

  it('folds the case of accented letters', () => {
    const text = EditorState.create({ doc: 'Été été ete' });
    expect([...occurrences(text, 'été', DEFAULT_OCCURRENCE_OPTIONS)].map(match => match.from)).toEqual([0, 4]);
  });
});

describe('isWholeWord', () => {
  const state = EditorState.create({ doc: 'un mot, des mots' });

  it('needs non-word characters around the match', () => {
    expect(isWholeWord(state, 3, 6)).toBe(true);
    expect(isWholeWord(state, 12, 15)).toBe(false);
  });

  it('accepts a match bounded by its own punctuation', () => {
    expect(isWholeWord(state, 2, 7)).toBe(true);
  });
});

describe('selectNextMatch', () => {
  it('first selects the word around an empty cursor', () => {
    const target = createTarget('Le lé table', EditorSelection.single(1));
    expect(selectNextMatch(target)).toBe(true);
    expect(selected(target)).toEqual(['Le']);
  });

  it('skips accented and partial occurrences', () => {
    const target = createTarget('Le lé table le', EditorSelection.single(0, 2));
    selectNextMatch(target);
    expect(starts(target)).toEqual([0, 12]);
    // Wraps around, nothing left: the selection stays
    selectNextMatch(target);
    expect(starts(target)).toEqual([0, 12]);
  });

  it('wraps to the start of the document', () => {
    const target = createTarget('mot un mot', EditorSelection.single(7, 10));
    selectNextMatch(target);
    expect(starts(target)).toEqual([0, 7]);
    expect(target.state.selection.main.from).toBe(7);
  });

  it('refuses ranges holding different texts', () => {
    const selection = EditorSelection.create([EditorSelection.range(0, 2), EditorSelection.range(3, 6)]);
    expect(selectNextMatch(createTarget('un mot un mot', selection))).toBe(false);
  });

  it('keeps the case when Match Case is on', () => {
    const target = createTarget('Mot mot Mot', EditorSelection.single(0, 3), { caseSensitive: true, wholeWord: true });
    selectNextMatch(target);
    expect(starts(target)).toEqual([0, 8]);
  });
});

describe('selectAllMatches', () => {
  it('selects every whole-word occurrence, accents compared', () => {
    const target = createTarget('Le lé table le', EditorSelection.single(0, 2));
    expect(selectAllMatches(target)).toBe(true);
    expect(starts(target)).toEqual([0, 12]);
    expect(target.state.selection.main.from).toBe(0);
  });

  it('uses the word at an empty cursor', () => {
    const target = createTarget('chat chats chat', EditorSelection.single(12));
    selectAllMatches(target);
    expect(starts(target)).toEqual([0, 11]);
    expect(target.state.selection.main.from).toBe(11);
  });

  it('keeps a selection that is not a whole word', () => {
    const target = createTarget('table', EditorSelection.single(1, 3));
    expect(selectAllMatches(target)).toBe(true);
    expect(starts(target)).toEqual([1]);
  });
});

describe('toggleOccurrenceOption', () => {
  it('does nothing before Ctrl+D / Ctrl+Shift+L', () => {
    expect(toggleOccurrenceOption(createTarget('le', EditorSelection.single(0, 2)), 'wholeWord')).toBe(false);
  });

  it('selects all again after Ctrl+Shift+L', () => {
    const target = createTarget('Le lé table le', EditorSelection.single(0, 2));
    selectAllMatches(target);
    toggleOccurrenceOption(target, 'wholeWord');
    expect(starts(target)).toEqual([0, 9, 12]);
    toggleOccurrenceOption(target, 'caseSensitive');
    // `Le` itself no longer matches `le`, but stays the main range
    expect(starts(target)).toEqual([0]);
  });

  it('drops the Ctrl+D ranges that no longer match', () => {
    const target = createTarget('Le le', EditorSelection.single(0, 2));
    selectNextMatch(target);
    expect(starts(target)).toEqual([0, 3]);
    toggleOccurrenceOption(target, 'caseSensitive');
    expect(starts(target)).toEqual([0]);
  });

  it('is unavailable once the selection moved elsewhere', () => {
    const target = createTarget('le le', EditorSelection.single(0, 2));
    selectNextMatch(target);
    target.dispatch(target.state.update({ selection: EditorSelection.single(1) }));
    expect(toggleOccurrenceOption(target, 'wholeWord')).toBe(false);
  });
});
