import { TextChange } from '../models/editor.model';

/**
 * Beyond this many differing lines, the middle of the texts is replaced as one block: the line diff costs O(D²) memory,
 * and a note rewritten that much has no cursor worth keeping in place anyway.
 */
const MAX_LINE_EDITS = 1000;

/** A run of differing lines: `[oldStart, oldEnd)` of the old lines became `[newStart, newEnd)` of the new ones. */
interface LineHunk {
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
}

/** Lines of a text, each with its line break: joined back, they give the text. */
function splitLines(text: string): string[] {
  return text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
}

function isHighSurrogate(code: number): boolean {
  return code >= 0xd800 && code <= 0xdbff;
}

function isLowSurrogate(code: number): boolean {
  return code >= 0xdc00 && code <= 0xdfff;
}

/** `pos` falls between the two halves of a character outside the BMP (an emoji): a change must not start or end there. */
function splitsPair(text: string, pos: number): boolean {
  return pos > 0 && pos < text.length && isHighSurrogate(text.charCodeAt(pos - 1)) && isLowSurrogate(text.charCodeAt(pos));
}

/** Smallest change turning `before` into `after` (common start and end kept), offsets shifted by `offset`. */
function refine(before: string, after: string, offset: number): TextChange | null {
  if (before === after) {
    return null;
  }

  const shortest = Math.min(before.length, after.length);
  let start = 0;

  while (start < shortest && before.charCodeAt(start) === after.charCodeAt(start)) {
    start++;
  }

  while (splitsPair(before, start) || splitsPair(after, start)) {
    start--;
  }

  let end = 0;

  while (end < shortest - start && before.charCodeAt(before.length - 1 - end) === after.charCodeAt(after.length - 1 - end)) {
    end++;
  }

  while (splitsPair(before, before.length - end) || splitsPair(after, after.length - end)) {
    end--;
  }

  return { from: offset + start, to: offset + before.length - end, insert: after.slice(start, after.length - end) };
}

/**
 * Myers' diff of two line lists: the runs of differing lines, in order, `null` past `maxEdits` inserted or deleted
 * lines. Each step keeps only the diagonals it reached (2d + 1 values), so memory stays O(D²).
 */
function lineHunks(before: string[], after: string[], maxEdits: number): LineHunk[] | null {
  const n = before.length;
  const m = after.length;
  const max = n + m;
  const furthest = new Int32Array(2 * max + 2);
  const trace: Int32Array[] = [];
  let reached = false;

  for (let d = 0; d <= Math.min(max, maxEdits) && !reached; d++) {
    trace.push(furthest.slice(max - d, max + d + 1));

    for (let k = -d; k <= d && !reached; k += 2) {
      const down = k === -d || (k !== d && (furthest[max + k - 1] ?? 0) < (furthest[max + k + 1] ?? 0));
      let x = down ? furthest[max + k + 1] ?? 0 : (furthest[max + k - 1] ?? 0) + 1;
      let y = x - k;

      while (x < n && y < m && before[x] === after[y]) {
        x++;
        y++;
      }

      furthest[max + k] = x;
      reached = x >= n && y >= m;
    }
  }

  return reached ? backtrack(trace, n, m) : null;
}

/** Walks the trace back from the end: equal lines close a hunk, inserted or deleted ones widen it. */
function backtrack(trace: Int32Array[], n: number, m: number): LineHunk[] {
  const hunks: LineHunk[] = [];
  let x = n;
  let y = m;
  let open: LineHunk | null = null;

  for (let d = trace.length - 1; d > 0; d--) {
    const reachedBefore = trace[d] ?? new Int32Array(0);
    const at = (k: number): number => reachedBefore[k + d] ?? 0;
    const k = x - y;
    const fromAbove = k === -d || (k !== d && at(k - 1) < at(k + 1));
    const previousK = fromAbove ? k + 1 : k - 1;
    const previousX = at(previousK);
    const previousY = previousX - previousK;

    while (x > previousX && y > previousY) {
      if (open) {
        hunks.push(open);
        open = null;
      }

      x--;
      y--;
    }

    // One line inserted or deleted: it starts the run, or widens the one already open
    open ??= { oldStart: x, oldEnd: x, newStart: y, newEnd: y };
    open.oldStart = previousX;
    open.newStart = previousY;
    x = previousX;
    y = previousY;
  }

  if (open) {
    hunks.push(open);
  }

  return hunks.reverse();
}

function lineOffsets(lines: string[]): number[] {
  const offsets = [0];

  for (const line of lines) {
    offsets.push((offsets[offsets.length - 1] ?? 0) + line.length);
  }

  return offsets;
}

/**
 * Changes turning `before` into `after`, small enough for an editor to keep the cursors where they are: the lines that
 * differ (Myers' diff), each run narrowed to the characters that differ. Offsets refer to `before`, in order, never
 * overlapping, and never cut an emoji in two.
 */
export function diffText(before: string, after: string): TextChange[] {
  if (before === after) {
    return [];
  }

  const oldLines = splitLines(before);
  const newLines = splitLines(after);
  const oldOffsets = lineOffsets(oldLines);
  const newOffsets = lineOffsets(newLines);
  const hunks = lineHunks(oldLines, newLines, MAX_LINE_EDITS)
    ?? [{ oldStart: 0, oldEnd: oldLines.length, newStart: 0, newEnd: newLines.length }];
  const changes: TextChange[] = [];

  for (const hunk of hunks) {
    const from = oldOffsets[hunk.oldStart] ?? 0;
    const oldText = before.slice(from, oldOffsets[hunk.oldEnd]);
    const newText = after.slice(newOffsets[hunk.newStart], newOffsets[hunk.newEnd]);
    const change = refine(oldText, newText, from);

    if (change) {
      changes.push(change);
    }
  }

  return changes;
}
