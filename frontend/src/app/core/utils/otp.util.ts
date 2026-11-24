/** Boxes of the code entry (digits of a pairing code). */
export const OTP_LENGTH = 6;

/**
 * State of the code boxes: a digit or an empty string per box, the box that takes the focus after the last edit, and
 * "select all" (Ctrl+A), which the next edit replaces as a whole.
 */
export interface OtpState {
  readonly digits: readonly string[];
  readonly focus: number;
  readonly allSelected: boolean;
}

const EMPTY_DIGITS: readonly string[] = Array.from({ length: OTP_LENGTH }, () => '');
const LAST = OTP_LENGTH - 1;

export function otpEmpty(): OtpState {
  return { digits: EMPTY_DIGITS, focus: 0, allSelected: false };
}

/** The digits of a typed or pasted text, in order (spaces, dashes and any other character are dropped). */
export function otpDigitsOf(text: string): string {
  return text.replace(/\D/g, '');
}

/** The digits typed so far, joined (a gap shortens the code: it is not complete). */
export function otpCode(state: OtpState): string {
  return state.digits.join('');
}

export function otpComplete(state: OtpState): boolean {
  return state.digits.every(digit => digit !== '');
}

/** A state holding `code` (its digits only, six at most), the focus after the last one. */
export function otpFromCode(code: string): OtpState {
  return otpPaste(otpEmpty(), 0, code);
}

function clampIndex(index: number): number {
  return Math.min(Math.max(index, 0), LAST);
}

function withDigit(digits: readonly string[], index: number, digit: string): string[] {
  return digits.map((current, position) => (position === index ? digit : current));
}

/** A "select all" is replaced as a whole: the edit starts from empty boxes, at the first one. */
function start(state: OtpState, index: number): { digits: readonly string[]; at: number } {
  return state.allSelected ? { digits: EMPTY_DIGITS, at: 0 } : { digits: state.digits, at: clampIndex(index) };
}

/**
 * Text typed in box `index`: a single digit fills it and moves on; anything else is ignored (letters, symbols, spaces);
 * several characters (autofill, a keyboard suggestion) are handled as a paste.
 */
export function otpType(state: OtpState, index: number, text: string): OtpState {
  const typed = otpDigitsOf(text);

  if (typed.length === 0) {
    return state;
  }

  if (typed.length > 1) {
    return otpPaste(state, index, typed);
  }

  const from = start(state, index);

  return { digits: withDigit(from.digits, from.at, typed), focus: clampIndex(from.at + 1), allSelected: false };
}

/**
 * Text pasted in box `index`: six digits or more fill every box (whatever the focus), fewer fill from the focused box;
 * the extra digits are dropped. A paste without digits changes nothing.
 */
export function otpPaste(state: OtpState, index: number, text: string): OtpState {
  const pasted = otpDigitsOf(text);

  if (pasted.length === 0) {
    return state;
  }

  const from = start(state, index);
  const first = pasted.length >= OTP_LENGTH ? 0 : from.at;
  const digits = [...from.digits];
  const count = Math.min(pasted.length, OTP_LENGTH - first);

  for (let offset = 0; offset < count; offset++) {
    digits[first + offset] = pasted.charAt(offset);
  }

  return { digits, focus: clampIndex(first + count), allSelected: false };
}

/**
 * Backspace in box `index`: clears it, or, when it is already empty, goes back to the previous box and clears that one.
 * After a "select all", clears everything.
 */
export function otpBackspace(state: OtpState, index: number): OtpState {
  if (state.allSelected) {
    return otpEmpty();
  }

  const at = clampIndex(index);

  if (state.digits[at] !== '') {
    return { digits: withDigit(state.digits, at, ''), focus: at, allSelected: false };
  }

  const previous = clampIndex(at - 1);

  return { digits: withDigit(state.digits, previous, ''), focus: previous, allSelected: false };
}

/** Delete in box `index`: clears it and stays there (everything after a "select all"). */
export function otpDelete(state: OtpState, index: number): OtpState {
  if (state.allSelected) {
    return otpEmpty();
  }

  const at = clampIndex(index);

  return { digits: withDigit(state.digits, at, ''), focus: at, allSelected: false };
}

/** Arrow keys: moves the focus by `step` boxes from `from`, stopping at the ends (and drops a "select all"). */
export function otpMove(state: OtpState, from: number, step: number): OtpState {
  return { ...state, focus: clampIndex(from + step), allSelected: false };
}

/** Home, End, a click: puts the focus on a box (and drops a "select all"). */
export function otpMoveTo(state: OtpState, index: number): OtpState {
  return { ...state, focus: clampIndex(index), allSelected: false };
}

/** Ctrl+A: the next edit replaces every box. */
export function otpSelectAll(state: OtpState): OtpState {
  return { ...state, allSelected: true };
}

/** Cut: the text to put in the clipboard (the whole code after a "select all", else the digit) and what is left. */
export function otpCut(state: OtpState, index: number): { text: string; state: OtpState } {
  const text = state.allSelected ? otpCode(state) : (state.digits[clampIndex(index)] ?? '');

  return { text, state: otpDelete(state, index) };
}
