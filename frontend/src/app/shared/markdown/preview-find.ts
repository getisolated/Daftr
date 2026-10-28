import { FindOptions } from '../../core/models/find.model';
import { MAX_FIND_MATCHES } from '../codemirror/find-matches';

/** An occurrence in a text, by offsets. */
export interface TextSpan {
  from: number;
  to: number;
}

const WORD_CHAR = /[\p{L}\p{N}_]/u;
/** Elements whose text is not read as such in the preview (diagrams, styles). */
const SKIPPED_TEXT = 'svg, style, script';
/** Text in two different blocks is never one match: a line break is put between them. */
const TEXT_BLOCK = 'p, li, h1, h2, h3, h4, h5, h6, td, th, pre, blockquote, dt, dd, summary, div';

/** `\n`, `\r`, `\t` and `\\` typed in a plain search, read as CodeMirror's SearchQuery reads them. */
function unquote(search: string): string {
  return search.replace(/\\([nrt\\])/g, (_match, char: string) => {
    if (char === 'n') {
      return '\n';
    }
    if (char === 'r') {
      return '\r';
    }
    return char === 't' ? '\t' : '\\';
  });
}

/** CodeMirror's rule: a match is not a whole word when it continues a word on either side. */
function isWholeWord(text: string, from: number, to: number): boolean {
  const joins = (outside: string | undefined, inside: string | undefined): boolean =>
    outside !== undefined && inside !== undefined && WORD_CHAR.test(outside) && WORD_CHAR.test(inside);
  return !joins(text[from - 1], text[from]) && !joins(text[to], text[to - 1]);
}

/** The query of the find panel as a regexp; null when it matches nothing (empty search, invalid regexp). */
export function findRegExp(options: FindOptions): RegExp | null {
  if (options.search === '') {
    return null;
  }

  const source = options.regexp ? options.search : unquote(options.search).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

  try {
    return new RegExp(source, `gmu${options.caseSensitive ? '' : 'i'}`);
  }
  catch {
    return null;
  }
}

/** Every occurrence of the query in `text`, same options as the editor search, up to `limit`. */
export function findInText(text: string, options: FindOptions, limit = MAX_FIND_MATCHES): TextSpan[] {
  const regexp = findRegExp(options);
  const spans: TextSpan[] = [];

  if (!regexp) {
    return spans;
  }

  for (const match of text.matchAll(regexp)) {
    const from = match.index;
    const to = from + match[0].length;

    if (spans.length >= limit) {
      break;
    }

    // An empty match (`^`, `\b`…) has nothing to highlight
    if (to > from && (!options.wholeWord || isWholeWord(text, from, to))) {
      spans.push({ from, to });
    }
  }

  return spans;
}

/**
 * Rank of the current source match among the matches of its preview block (source lines `[start, end)`): the
 * preview has no source positions, so the n-th match of a block is shown as its n-th occurrence.
 */
export function rankInBlock(lines: readonly number[], current: number, start: number, end: number): number {
  return lines.slice(0, current).filter(line => line >= start && line < end).length;
}

/** The occurrences of the query in the rendered text of `root`, as DOM ranges (they may span inline elements). */
export function findRanges(root: HTMLElement, options: FindOptions): Range[] {
  const nodes: Text[] = [];
  const starts: number[] = [];
  let text = '';
  let block: Element | null = null;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: node => (node.parentElement?.closest(SKIPPED_TEXT) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });

  for (let node = walker.nextNode() as Text | null; node; node = walker.nextNode() as Text | null) {
    const nodeBlock = node.parentElement?.closest(TEXT_BLOCK) ?? null;

    if (text !== '' && nodeBlock !== block) {
      text += '\n';
    }

    block = nodeBlock;
    nodes.push(node);
    starts.push(text.length);
    text += node.data;
  }

  // Last node starting at or before `offset` (`end`: the one holding the character just before)
  const nodeAt = (offset: number, end: boolean): number => {
    const index = starts.findIndex(start => (end ? start >= offset : start > offset));
    return (index === -1 ? starts.length : index) - 1;
  };

  return findInText(text, options).map(span => {
    const first = nodeAt(span.from, false);
    const last = nodeAt(span.to, true);
    const range = document.createRange();
    range.setStart(nodes[first]!, span.from - starts[first]!);
    range.setEnd(nodes[last]!, Math.min(nodes[last]!.length, span.to - starts[last]!));
    return range;
  });
}
