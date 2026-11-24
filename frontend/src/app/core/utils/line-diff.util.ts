/** Beyond this many cells, the differing middle of two texts is shown as one replaced block (the table costs O(n·m)). */
const MAX_TABLE_CELLS = 4_000_000;

/** A row of the side-by-side comparison; `same` rows have both sides, the others at least one. */
export interface DiffRow {
  kind: 'same' | 'changed' | 'removed' | 'added';
  /** Line of the left text (the original), `null` for an added line. */
  left: string | null;
  right: string | null;
  /** 1-based line numbers. */
  leftLine: number | null;
  rightLine: number | null;
}

/** Lines of a text without their line break; an empty text has none. */
function toLines(text: string): string[] {
  if (text === '') {
    return [];
  }

  const lines = text.split(/\r?\n/);

  // A final line break does not start a line
  if (lines[lines.length - 1] === '') {
    lines.pop();
  }

  return lines;
}

/** Edit script between two line lists by their longest common subsequence: `=`, `-` (left only), `+` (right only). */
function editScript(left: readonly string[], right: readonly string[]): ('=' | '-' | '+')[] {
  const rows = left.length;
  const columns = right.length;

  if ((rows + 1) * (columns + 1) > MAX_TABLE_CELLS) {
    return [...Array<'-'>(rows).fill('-'), ...Array<'+'>(columns).fill('+')];
  }

  // lengths[i][j]: common length of left[i..] and right[j..]
  const lengths = Array.from({ length: rows + 1 }, () => new Uint32Array(columns + 1));

  for (let i = rows - 1; i >= 0; i--) {
    const row = lengths[i] as Uint32Array;
    const below = lengths[i + 1] as Uint32Array;

    for (let j = columns - 1; j >= 0; j--) {
      row[j] = left[i] === right[j] ? (below[j + 1] as number) + 1 : Math.max(below[j] as number, row[j + 1] as number);
    }
  }

  const script: ('=' | '-' | '+')[] = [];
  let i = 0;
  let j = 0;

  while (i < rows && j < columns) {
    if (left[i] === right[j]) {
      script.push('=');
      i++;
      j++;
    }
    else if (((lengths[i + 1] as Uint32Array)[j] as number) >= ((lengths[i] as Uint32Array)[j + 1] as number)) {
      script.push('-');
      i++;
    }
    else {
      script.push('+');
      j++;
    }
  }

  for (; i < rows; i++) {
    script.push('-');
  }

  for (; j < columns; j++) {
    script.push('+');
  }

  return script;
}

/**
 * Line comparison of `original` against `copy`, as the rows of a side-by-side view. A run of removed lines followed by
 * added ones is paired into `changed` rows, line for line, the rest being `removed` or `added`.
 */
export function diffLines(original: string, copy: string): DiffRow[] {
  const left = toLines(original);
  const right = toLines(copy);
  const script = editScript(left, right);
  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  let index = 0;

  while (index < script.length) {
    if (script[index] === '=') {
      rows.push({ kind: 'same', left: left[i] ?? null, right: right[j] ?? null, leftLine: i + 1, rightLine: j + 1 });
      i++;
      j++;
      index++;
      continue;
    }

    const removed: number[] = [];
    const added: number[] = [];

    while (index < script.length && script[index] !== '=') {
      if (script[index] === '-') {
        removed.push(i++);
      }
      else {
        added.push(j++);
      }

      index++;
    }

    const paired = Math.min(removed.length, added.length);

    for (let k = 0; k < Math.max(removed.length, added.length); k++) {
      const l = removed[k];
      const r = added[k];
      let kind: DiffRow['kind'] = 'changed';

      if (k >= paired) {
        kind = l === undefined ? 'added' : 'removed';
      }

      rows.push({
        kind,
        left: l === undefined ? null : left[l] ?? null,
        right: r === undefined ? null : right[r] ?? null,
        leftLine: l === undefined ? null : l + 1,
        rightLine: r === undefined ? null : r + 1,
      });
    }
  }

  return rows;
}

/** The texts differ: at least one row is not `same`. */
export function hasDifferences(rows: readonly DiffRow[]): boolean {
  return rows.some(row => row.kind !== 'same');
}
