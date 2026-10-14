// Pure GFM table logic (parse / format / edit / TSV import), free of any CodeMirror or DOM dependency.

export type TableAlign = 'none' | 'left' | 'center' | 'right';

export interface TableModel {
  header: string[];
  align: TableAlign[];
  rows: string[][];
}

/** Offsets of a cell inside a line (or inside the whole table text once serialized). */
export interface CellSpan {
  /** Trimmed content boundaries. */
  from: number;
  to: number;
  /** Boundaries between two pipes, whitespace included. */
  segFrom: number;
  segTo: number;
}

export interface ParsedTable {
  model: TableModel;
  /** Cell spans per raw line (header, delimiter, then body rows), relative to the line start. */
  lines: CellSpan[][];
  lineStarts: number[];
}

export interface SerializedTable {
  text: string;
  /** Row 0 is the header, rows 1.. the body; offsets are relative to `text`. */
  cells: CellSpan[][];
}

export interface TableCellPosition {
  /** 0 for the header (or the delimiter line), 1.. for the body rows. */
  row: number;
  col: number;
}

const MIN_COLUMN_WIDTH = 3;
const DELIMITER_CELL = /^:?-+:?$/;

/** Splits a raw table line on its unescaped pipes; the optional leading / trailing pipes do not create cells. */
function splitRow(line: string): CellSpan[] {
  const pipes: number[] = [];
  let backslashes = 0;

  for (let i = 0; i < line.length; i++) {
    const char = line[i];

    if (char === '\\') {
      backslashes++;
      continue;
    }

    if (char === '|' && backslashes % 2 === 0) {
      pipes.push(i);
    }

    backslashes = 0;
  }

  const firstNonBlank = line.search(/\S/);
  let cursor = 0;

  if (firstNonBlank !== -1 && pipes[0] === firstNonBlank) {
    cursor = firstNonBlank + 1;
    pipes.shift();
  }

  const spans: CellSpan[] = [];

  for (const pipe of pipes) {
    spans.push(toSpan(line, cursor, pipe));
    cursor = pipe + 1;
  }

  if (line.slice(cursor).trim() !== '') {
    spans.push(toSpan(line, cursor, line.length));
  }

  return spans;
}

function toSpan(line: string, segFrom: number, segTo: number): CellSpan {
  const segment = line.slice(segFrom, segTo);
  const trimmed = segment.trim();
  const leading = segment.length - segment.trimStart().length;
  // An empty cell keeps a caret position right after the usual single space
  const from = trimmed === '' ? segFrom + Math.min(1, segment.length) : segFrom + leading;

  return { from, to: from + trimmed.length, segFrom, segTo };
}

function alignFromDelimiter(cell: string): TableAlign {
  const left = cell.startsWith(':');
  const right = cell.endsWith(':');

  if (left && right) {
    return 'center';
  }

  if (left) {
    return 'left';
  }

  return right ? 'right' : 'none';
}

/** Returns null when the text is not a valid GFM table (header + matching delimiter row). */
export function parseTable(text: string): ParsedTable | null {
  const rawLines = text.split('\n');

  if (rawLines.length < 2) {
    return null;
  }

  const lines = rawLines.map(splitRow);
  const lineStarts: number[] = [];
  let offset = 0;

  for (const rawLine of rawLines) {
    lineStarts.push(offset);
    offset += rawLine.length + 1;
  }

  const [headerSpans, delimiterSpans] = lines as [CellSpan[], CellSpan[]];
  const delimiters = delimiterSpans.map(span => rawLines[1]!.slice(span.from, span.to));

  if (headerSpans.length === 0 || headerSpans.length !== delimiters.length || !delimiters.every(cell => DELIMITER_CELL.test(cell))) {
    return null;
  }

  const columnCount = headerSpans.length;
  const cellTexts = (lineIndex: number): string[] => {
    const texts = lines[lineIndex]!.map(span => rawLines[lineIndex]!.slice(span.from, span.to));

    // GFM: extra cells are dropped, missing ones are empty
    while (texts.length < columnCount) {
      texts.push('');
    }

    return texts.slice(0, columnCount);
  };

  const rows: string[][] = [];

  for (let i = 2; i < rawLines.length; i++) {
    rows.push(cellTexts(i));
  }

  return {
    model: { header: cellTexts(0), align: delimiters.map(alignFromDelimiter), rows },
    lines,
    lineStarts,
  };
}

/** Locates the cell holding `offset` (relative to the table start). */
export function cellAt(parsed: ParsedTable, offset: number): TableCellPosition {
  let lineIndex = 0;

  for (let i = 0; i < parsed.lineStarts.length; i++) {
    if (parsed.lineStarts[i]! <= offset) {
      lineIndex = i;
    }
  }

  const spans = parsed.lines[lineIndex]!;
  const relative = offset - parsed.lineStarts[lineIndex]!;
  let col = spans.findIndex(span => relative <= span.segTo);

  if (col === -1) {
    col = spans.length - 1;
  }

  return {
    row: lineIndex <= 1 ? 0 : lineIndex - 1,
    col: Math.max(0, Math.min(col, parsed.model.header.length - 1)),
  };
}

function padCell(cell: string, width: number, align: TableAlign): { before: number; after: number } {
  const free = width - cell.length;

  if (align === 'right') {
    return { before: free, after: 0 };
  }

  if (align === 'center') {
    const before = Math.floor(free / 2);

    return { before, after: free - before };
  }

  return { before: 0, after: free };
}

function delimiterCell(width: number, align: TableAlign): string {
  const dashes = width + 2;

  switch (align) {
    case 'left':
      return `:${'-'.repeat(dashes - 1)}`;
    case 'right':
      return `${'-'.repeat(dashes - 1)}:`;
    case 'center':
      return `:${'-'.repeat(dashes - 2)}:`;
    default:
      return '-'.repeat(dashes);
  }
}

/** Renders the table with every pipe vertically aligned. */
export function serializeTable(model: TableModel): SerializedTable {
  const widths = model.header.map((cell, col) =>
    Math.max(MIN_COLUMN_WIDTH, cell.length, ...model.rows.map(row => row[col]!.length)),
  );
  const lines: string[] = [];
  const cells: CellSpan[][] = [];
  let lineStart = 0;

  const addRow = (row: string[]): void => {
    let line = '|';
    const spans: CellSpan[] = [];

    row.forEach((cell, col) => {
      const { before, after } = padCell(cell, widths[col]!, model.align[col]!);
      const segFrom = line.length;
      const from = segFrom + 1 + before;

      line += ` ${' '.repeat(before)}${cell}${' '.repeat(after)} |`;
      spans.push({ from: lineStart + from, to: lineStart + from + cell.length, segFrom: lineStart + segFrom, segTo: lineStart + line.length - 1 });
    });

    lines.push(line);
    cells.push(spans);
    lineStart += line.length + 1;
  };

  addRow(model.header);

  const delimiter = `|${widths.map((width, col) => delimiterCell(width, model.align[col]!)).join('|')}|`;

  lines.push(delimiter);
  lineStart += delimiter.length + 1;
  model.rows.forEach(addRow);

  return { text: lines.join('\n'), cells };
}

function emptyRow(columnCount: number): string[] {
  return Array.from({ length: columnCount }, () => '');
}

/** Empty, pre-formatted table: the default 2×2 is the header row plus one body row. */
export function createEmptyTable(columns = 2, bodyRows = 1): SerializedTable {
  return serializeTable({
    header: emptyRow(columns),
    align: Array.from({ length: columns }, () => 'none' as TableAlign),
    rows: Array.from({ length: bodyRows }, () => emptyRow(columns)),
  });
}

/** Inserts an empty body row before `bodyIndex` (`rows.length` appends). */
export function insertRow(model: TableModel, bodyIndex: number): TableModel {
  const rows = [...model.rows];

  rows.splice(bodyIndex, 0, emptyRow(model.header.length));

  return { ...model, rows };
}

export function removeRow(model: TableModel, bodyIndex: number): TableModel {
  return { ...model, rows: model.rows.filter((_, index) => index !== bodyIndex) };
}

/** Inserts an empty column before `col` (`header.length` appends). */
export function insertColumn(model: TableModel, col: number): TableModel {
  const withEmpty = (row: string[]): string[] => [...row.slice(0, col), '', ...row.slice(col)];

  return {
    header: withEmpty(model.header),
    align: [...model.align.slice(0, col), 'none', ...model.align.slice(col)],
    rows: model.rows.map(withEmpty),
  };
}

export function removeColumn(model: TableModel, col: number): TableModel {
  const without = <T>(row: T[]): T[] => row.filter((_, index) => index !== col);

  return { header: without(model.header), align: without(model.align), rows: model.rows.map(without) };
}

export function alignColumn(model: TableModel, col: number, align: TableAlign): TableModel {
  return { ...model, align: model.align.map((current, index) => (index === col ? align : current)) };
}

/** Parses spreadsheet clipboard data, honouring quoted fields (`"a\tb"`, `""` escapes, embedded newlines). */
function parseTsv(text: string): string[][] {
  const records: string[][] = [];
  let record: string[] = [];
  let field = '';
  let quoted = false;

  for (let i = 0; i < text.length; i++) {
    const char = text[i]!;

    if (quoted) {
      if (char === '"' && text[i + 1] === '"') {
        field += '"';
        i++;
      } else if (char === '"') {
        quoted = false;
      } else {
        field += char;
      }
    } else if (char === '"' && field === '') {
      quoted = true;
    } else if (char === '\t') {
      record.push(field);
      field = '';
    } else if (char === '\n' || char === '\r') {
      if (char === '\r' && text[i + 1] === '\n') {
        i++;
      }

      record.push(field);
      records.push(record);
      record = [];
      field = '';
    } else {
      field += char;
    }
  }

  if (field !== '' || record.length > 0) {
    record.push(field);
    records.push(record);
  }

  return records;
}

function toMarkdownCell(value: string): string {
  return value.trim().replace(/\r?\n/g, '<br>').replace(/\|/g, '\\|');
}

/**
 * Converts spreadsheet clipboard data to a markdown table (first row = header).
 * Returns null unless the data is rectangular with at least 2 rows and 2 columns, so that
 * ordinary text or code containing a stray tab is pasted untouched.
 */
export function tsvToMarkdownTable(text: string): string | null {
  if (!text.includes('\t')) {
    return null;
  }

  const records = parseTsv(text);
  const columnCount = records[0]?.length ?? 0;

  if (records.length < 2 || columnCount < 2 || records.some(record => record.length !== columnCount)) {
    return null;
  }

  const [header, ...rows] = records.map(record => record.map(toMarkdownCell)) as [string[], ...string[][]];

  return serializeTable({ header, align: header.map(() => 'none' as TableAlign), rows }).text;
}
