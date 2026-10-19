/**
 * Whether the bar shows one drop-down per group instead of every button: when its full version does not fit along its
 * own axis (width when horizontal, height when vertical). `fullLength` is that full length, `null` until measured.
 */
export function shouldCondense(fullLength: number | null, available: number): boolean {
  return fullLength !== null && available < fullLength;
}
