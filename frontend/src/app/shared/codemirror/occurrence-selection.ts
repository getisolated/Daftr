import { CharCategory, EditorSelection, EditorState, Extension, Prec, SelectionRange, StateEffect, StateField, Transaction } from '@codemirror/state';
import { EditorView, Tooltip, TooltipView, keymap, showTooltip } from '@codemirror/view';
import { RegExpCursor } from '@codemirror/search';
import { translatorOf } from './translator';

/** How Ctrl+D / Ctrl+Shift+L compare the selected text with the document, toggled from the buttons shown next to the selection. */
export interface OccurrenceOptions {
  caseSensitive: boolean;
  wholeWord: boolean;
}

/** Whole words by default: selecting `le` must not reach into `table`. */
export const DEFAULT_OCCURRENCE_OPTIONS: OccurrenceOptions = { caseSensitive: false, wholeWord: true };

/** Beyond this, Ctrl+Shift+L gives up, like CodeMirror's own command. */
const MAX_OCCURRENCES = 1000;

/** The command that last extended the selection: a toggle redoes its work with the new options. */
type OccurrenceMode = 'next' | 'all';

interface OccurrenceState {
  options: OccurrenceOptions;
  /** Null while the selection is not the result of one of the commands: the toggles are hidden. */
  mode: OccurrenceMode | null;
  tooltip: Tooltip | null;
}

interface OccurrenceUpdate {
  options?: OccurrenceOptions;
  mode: OccurrenceMode | null;
}

const updateOccurrence = StateEffect.define<OccurrenceUpdate>();

const occurrenceField = StateField.define<OccurrenceState>({
  create: () => ({ options: DEFAULT_OCCURRENCE_OPTIONS, mode: null, tooltip: null }),
  update: (value, tr) => nextOccurrenceState(value, tr),
  provide: field => showTooltip.from(field, value => value.tooltip),
});

function nextOccurrenceState(value: OccurrenceState, tr: Transaction): OccurrenceState {
  const effect = tr.effects.find(candidate => candidate.is(updateOccurrence));

  if (effect?.is(updateOccurrence)) {
    const { mode } = effect.value;
    const options = effect.value.options ?? value.options;
    const pos = tr.state.selection.main.from;

    if (!mode) {
      return { options, mode, tooltip: null };
    }
    // Kept while the selection grows from the same main range, so the buttons are not redrawn at each Ctrl+D
    const tooltip = value.tooltip?.pos === pos ? value.tooltip : togglesTooltip(pos);
    return { options, mode, tooltip };
  }

  // Any other selection move or edit ends the multi-cursor search
  if (value.mode && (tr.docChanged || tr.selection)) {
    return { ...value, mode: null, tooltip: null };
  }
  return value;
}

function optionsOf(state: EditorState): OccurrenceOptions {
  return state.field(occurrenceField, false)?.options ?? DEFAULT_OCCURRENCE_OPTIONS;
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function isWordChar(state: EditorState, pos: number): boolean {
  return state.charCategorizer(pos)(state.sliceDoc(pos, pos + 1)) === CharCategory.Word;
}

/** A match bounded by non-word characters (or the document edges) on both sides; a query starting or ending with one is bounded by itself. */
export function isWholeWord(state: EditorState, from: number, to: number): boolean {
  const startBounded = from === 0 || !isWordChar(state, from - 1) || !isWordChar(state, from);
  const endBounded = to === state.doc.length || !isWordChar(state, to) || !isWordChar(state, to - 1);
  return startBounded && endBounded;
}

/**
 * Occurrences of `query` between `from` and `to`. Unlike CodeMirror's search cursor, which strips the accents
 * (NFKD), letters are compared as they are: `le` matches neither `lé` nor `lè`.
 */
export function* occurrences(state: EditorState, query: string, options: OccurrenceOptions, from = 0, to = state.doc.length): Generator<{ from: number; to: number }> {
  const cursor = new RegExpCursor(state.doc, escapeRegExp(query), { ignoreCase: !options.caseSensitive }, from, to);

  for (let next = cursor.next(); !next.done; next = cursor.next()) {
    const match = next.value;

    if (!options.wholeWord || isWholeWord(state, match.from, match.to)) {
      yield { from: match.from, to: match.to };
    }
  }
}

/** Whether a selected range is an occurrence of `query` with these options. */
function isOccurrence(state: EditorState, range: SelectionRange, query: string, options: OccurrenceOptions): boolean {
  const text = state.sliceDoc(range.from, range.to);
  const sameText = options.caseSensitive ? text === query : text.toLowerCase() === query.toLowerCase();
  return sameText && (!options.wholeWord || isWholeWord(state, range.from, range.to));
}

/** First occurrence after the last range (wrapping to the start) that is not selected yet. */
function nextOccurrence(state: EditorState, query: string, options: OccurrenceOptions): { from: number; to: number } | null {
  const { ranges } = state.selection;
  const start = ranges[ranges.length - 1]?.to ?? 0;
  const segments: [number, number][] = [[start, state.doc.length], [0, start]];

  for (const [from, to] of segments) {
    for (const match of occurrences(state, query, options, from, to)) {
      const taken = ranges.some(range => range.from < match.to && range.to > match.from);

      if (!taken) {
        return match;
      }
    }
  }
  return null;
}

/** Every occurrence of the main range's text, the main range staying the main one; null beyond MAX_OCCURRENCES. */
function allOccurrences(state: EditorState, options: OccurrenceOptions): EditorSelection | null {
  const main = state.selection.main;
  const query = state.sliceDoc(main.from, main.to);
  const ranges: SelectionRange[] = [];
  let mainIndex = 0;

  for (const match of occurrences(state, query, options)) {
    if (ranges.length >= MAX_OCCURRENCES) {
      return null;
    }

    if (match.from === main.from) {
      mainIndex = ranges.length;
    }
    ranges.push(EditorSelection.range(match.from, match.to));
  }

  // The selection itself may not qualify (part of a word, while matching whole words): it is kept
  if (!ranges.some(range => range.from === main.from && range.to === main.to)) {
    return state.selection;
  }
  return EditorSelection.create(ranges, mainIndex);
}

/** The words around empty cursors, like CodeMirror's `selectWord`; null when it changes nothing. */
function expandToWords(state: EditorState): EditorSelection | null {
  const { selection } = state;
  const expanded = EditorSelection.create(
    selection.ranges.map(range => state.wordAt(range.head) ?? EditorSelection.cursor(range.head)),
    selection.mainIndex,
  );
  return expanded.eq(selection) ? null : expanded;
}

interface StateCommandTarget {
  state: EditorState;
  dispatch: (tr: Transaction) => void;
}

/**
 * Ctrl+D: adds the next occurrence of the selected text to the selection (an empty cursor first selects its word),
 * like VS Code. The toggles show up even without another occurrence, to loosen the options.
 */
export function selectNextMatch({ state, dispatch }: StateCommandTarget): boolean {
  const { ranges } = state.selection;
  const effects = updateOccurrence.of({ mode: 'next' });

  if (ranges.some(range => range.empty)) {
    const words = expandToWords(state);

    if (!words) {
      return false;
    }
    dispatch(state.update({ selection: words, effects, userEvent: 'select' }));
    return true;
  }

  const options = optionsOf(state);
  const main = state.selection.main;
  const query = state.sliceDoc(main.from, main.to);
  const sameText = ranges.every(range => isOccurrence(state, range, query, { ...options, wholeWord: false }));

  if (!sameText) {
    return false;
  }

  const match = nextOccurrence(state, query, options);
  const selection = match ? state.selection.addRange(EditorSelection.range(match.from, match.to), false) : undefined;
  const scroll = match ? [EditorView.scrollIntoView(match.to)] : [];
  dispatch(state.update({ selection, effects: [effects, ...scroll], userEvent: 'select' }));
  return true;
}

/** Ctrl+Shift+L: selects every occurrence of the selected text (of the word at the cursor when empty), like VS Code. */
export function selectAllMatches({ state, dispatch }: StateCommandTarget): boolean {
  const words = state.selection.main.empty ? expandToWords(state) : null;
  const source = words ? state.update({ selection: words }).state : state;

  if (source.selection.main.empty) {
    return false;
  }

  const selection = allOccurrences(source, optionsOf(source));

  if (!selection) {
    return false;
  }
  dispatch(state.update({ selection, effects: updateOccurrence.of({ mode: 'all' }), userEvent: 'select.search.matches' }));
  return true;
}

/** Switches an option and redoes the last command with it: Ctrl+Shift+L selects again, Ctrl+D keeps the ranges still matching. */
export function toggleOccurrenceOption({ state, dispatch }: StateCommandTarget, key: keyof OccurrenceOptions): boolean {
  const current = state.field(occurrenceField, false);

  if (!current?.mode) {
    return false;
  }

  const options = { ...current.options, [key]: !current.options[key] };
  const effects = updateOccurrence.of({ options, mode: current.mode });
  const main = state.selection.main;
  const query = state.sliceDoc(main.from, main.to);
  const kept = state.selection.ranges.filter(range => range === main || isOccurrence(state, range, query, options));
  const nextSelection = current.mode === 'all'
    ? allOccurrences(state, options)
    : EditorSelection.create(kept, kept.indexOf(main));
  dispatch(state.update({ selection: nextSelection ?? state.selection, effects }));
  return true;
}

const CASE_ICON = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M2 17 6.5 6 11 17M3.6 13.5h5.8"/><path d="M19 17v-5.5a2.5 2.5 0 0 0-5 0M19 14.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0"/></svg>';
const WORD_ICON = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 19h18M3 15v2M21 15v2"/><path d="M10 13v-3a2 2 0 0 0-4 0M10 12a2 2 0 1 1-4 0 2 2 0 0 1 4 0M13 6v7M13 11a2 2 0 1 0 4 0 2 2 0 0 0-4 0"/></svg>';

/** Match Case / Match Whole Word buttons above the main range, with the find panel's icons. */
function togglesTooltip(pos: number): Tooltip {
  return {
    pos,
    above: true,
    create: (view: EditorView): TooltipView => {
      const t = translatorOf(view.state);
      const dom = document.createElement('div');
      const buttons: [keyof OccurrenceOptions, HTMLButtonElement][] = [
        ['caseSensitive', toggleButton(view, 'caseSensitive', CASE_ICON, t('editor.occurrence.matchCase'))],
        ['wholeWord', toggleButton(view, 'wholeWord', WORD_ICON, t('editor.occurrence.wholeWord'))],
      ];
      const refresh = (state: EditorState): void => {
        const options = optionsOf(state);

        for (const [key, button] of buttons) {
          button.classList.toggle('cm-occurrence-on', options[key]);
          button.setAttribute('aria-pressed', `${options[key]}`);
        }
      };

      dom.className = 'cm-occurrence-toggles';
      dom.append(...buttons.map(([, button]) => button));
      refresh(view.state);
      return { dom, update: update => refresh(update.state) };
    },
  };
}

function toggleButton(view: EditorView, key: keyof OccurrenceOptions, icon: string, label: string): HTMLButtonElement {
  const button = document.createElement('button');

  button.type = 'button';
  button.className = 'cm-occurrence-toggle';
  button.title = label;
  button.setAttribute('aria-label', label);
  button.innerHTML = icon;
  // Keeps the editor focus and its selection
  button.addEventListener('mousedown', event => event.preventDefault());
  button.addEventListener('click', () => toggleOccurrenceOption(view, key));
  return button;
}

const occurrenceTheme = EditorView.baseTheme({
  '.cm-tooltip.cm-occurrence-toggles': {
    display: 'flex',
    gap: '1px',
    padding: '2px',
    background: 'var(--daftr-bg-chip)',
    border: '1px solid var(--daftr-border-soft)',
    borderRadius: '5px',
  },
  '.cm-occurrence-toggle': {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '22px',
    height: '22px',
    padding: '0',
    background: 'transparent',
    border: '1px solid transparent',
    borderRadius: '4px',
    color: 'var(--daftr-text-soft)',
    cursor: 'pointer',
  },
  '.cm-occurrence-toggle:hover': { background: 'rgba(var(--daftr-hover-rgb), 0.08)', color: 'var(--daftr-text)' },
  '.cm-occurrence-toggle.cm-occurrence-on': {
    background: 'rgba(var(--daftr-accent-strong-rgb), 0.3)',
    borderColor: 'var(--daftr-accent)',
    color: 'var(--daftr-text-bright)',
  },
});

/** Escape hides the toggles once the selection has nothing left to simplify (lowest precedence). */
function hideToggles({ state, dispatch }: StateCommandTarget): boolean {
  if (!state.field(occurrenceField, false)?.mode) {
    return false;
  }
  dispatch(state.update({ effects: updateOccurrence.of({ mode: null }) }));
  return true;
}

/**
 * State and toggles of Ctrl+D / Ctrl+Shift+L (the commands are bound by the host). `onChange` gets the options each
 * time a toggle changes them, so that the next editor can start with them.
 */
export function occurrenceSelection(initial: OccurrenceOptions, onChange: (options: OccurrenceOptions) => void): Extension {
  return [
    occurrenceField.init(() => ({ options: initial, mode: null, tooltip: null })),
    occurrenceTheme,
    Prec.lowest(keymap.of([{ key: 'Escape', run: hideToggles }])),
    EditorView.updateListener.of(update => {
      const before = update.startState.field(occurrenceField).options;
      const after = update.state.field(occurrenceField).options;

      if (before !== after) {
        onChange(after);
      }
    }),
  ];
}
