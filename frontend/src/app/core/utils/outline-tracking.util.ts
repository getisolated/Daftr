/** Vertical span (px) in the content of a scroller. */
export interface VerticalSpan {
  top: number;
  bottom: number;
}

/** Index of the heading the reader is currently in: the last one whose top went past `offset` px from the viewport top. */
export function findActiveHeading(tops: readonly number[], scrollTop: number, atBottom: boolean, offset = 40): number {
  if (!tops.length) return -1;
  if (atBottom) return tops.length - 1;

  let active = 0;
  tops.forEach((top, i) => {
    if (top <= scrollTop + offset) active = i;
  });
  return active;
}

/**
 * For each heading (by its source line), index of the preview block holding it: the last block starting at or before its
 * line, a heading being its own block unless it is nested (quote, list). -1 for a heading above the first block.
 * Both lists are in source order: one pass over them, however long the note.
 */
export function headingBlockIndexes(blockLines: readonly number[], headingLines: readonly number[]): number[] {
  const indexes: number[] = [];
  let block = -1;

  for (const line of headingLines) {
    let next = blockLines[block + 1];

    while (next !== undefined && next <= line) {
      block++;
      next = blockLines[block + 1];
    }

    indexes.push(block);
  }

  return indexes;
}

/**
 * Scroll that brings `item` into `view` (the visible span of the scroller, in the same content coordinates) by the shortest
 * move, like `scrollIntoView({ block: 'nearest' })` but without moving any other scroller; unchanged when already in view.
 */
export function revealScrollTop(scrollTop: number, view: VerticalSpan, item: VerticalSpan): number {
  const above = view.top - item.top;
  const below = item.bottom - view.bottom;

  // An item taller than the view shows its start
  if (above > 0 || (below > 0 && item.bottom - item.top > view.bottom - view.top)) {
    return Math.max(0, scrollTop - above);
  }

  if (below > 0) {
    return scrollTop + below;
  }

  return scrollTop;
}
