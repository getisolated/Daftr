import { EditorState } from '@codemirror/state';
import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import { anchorMatches, createHeadingSlugger, headingSourceText } from '../../core/utils/heading-anchor.util';

/**
 * CodeMirror only parses around the visible part (and not at all while the editor is hidden behind the preview): the
 * headings further down a long note would be missing. The rest is parsed by slices of this duration, so that a very long
 * note does not freeze the window. The parse is kept by the state: after an edit, only the changed part is parsed again.
 */
const PARSE_SLICE_MS = 100;

/** Following a link fragment is a one-off action: the whole document can be parsed at once. */
const ANCHOR_PARSE_BUDGET_MS = 2000;

export interface OutlineHeading {
  level: 1 | 2 | 3;
  text: string;
  /** Document position of the start of the heading line. */
  from: number;
  /** Its line (1-based), as the preview numbers its blocks. */
  line: number;
}

/** Headings found so far, and whether the document was parsed to its end (when not, a new call continues the parse). */
export interface OutlineExtraction {
  headings: OutlineHeading[];
  complete: boolean;
}

/** h1, h2 and h3 headings of the document, read from the syntax tree so that `#` lines inside code blocks are ignored. */
export function extractHeadings(state: EditorState, budgetMs = PARSE_SLICE_MS): OutlineExtraction {
  const headings: OutlineHeading[] = [];
  const fullTree = ensureSyntaxTree(state, state.doc.length, budgetMs);
  const tree = fullTree ?? syntaxTree(state);

  tree.iterate({
    enter: node => {
      const match = /^(?:ATXHeading([123])|SetextHeading([12]))$/.exec(node.name);
      if (!match) return;

      const level = Number(match[1] ?? match[2]) as 1 | 2 | 3;
      const line = state.doc.lineAt(node.from);
      const text = line.text.replace(/^\s*#{1,3}\s+/, '').replace(/\s+#+\s*$/, '').trim();
      if (text) {
        headings.push({ level, text, from: line.from, line: line.number });
      }
      return false;
    },
  });

  return { headings, complete: fullTree !== null };
}

/** Start of the heading (any level) a link fragment designates, named as in the preview; `null` when none matches. */
export function findHeadingByAnchor(state: EditorState, anchor: string): number | null {
  const slugOf = createHeadingSlugger();
  let found: number | null = null;

  const tree = ensureSyntaxTree(state, state.doc.length, ANCHOR_PARSE_BUDGET_MS) ?? syntaxTree(state);

  tree.iterate({
    enter: node => {
      if (found !== null) return false;
      if (!/^(?:ATXHeading[1-6]|SetextHeading[12])$/.test(node.name)) return;

      const line = state.doc.lineAt(node.from);
      const text = line.text.replace(/^\s*#{1,6}(?:\s+|$)/, '').replace(/\s+#+\s*$/, '');
      if (anchorMatches(anchor, slugOf(headingSourceText(text)))) {
        found = line.from;
      }
      return false;
    },
  });

  return found;
}
