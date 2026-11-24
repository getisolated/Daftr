import { describe, it, expect } from 'vitest';
import {
  OTP_LENGTH, OtpState, otpBackspace, otpCode, otpComplete, otpCut, otpDelete, otpDigitsOf, otpEmpty, otpFromCode, otpMove, otpMoveTo, otpPaste,
  otpSelectAll, otpType,
} from '../src/app/core/utils/otp.util';

/** Types each character in the box that has the focus, as the user does. */
function typeAll(text: string, from: OtpState = otpEmpty()): OtpState {
  return [...text].reduce((state, char) => otpType(state, state.focus, char), from);
}

describe('otpDigitsOf', () => {
  it('keeps the digits only', () => {
    expect(otpDigitsOf('12 34-5a6')).toBe('123456');
    expect(otpDigitsOf('abc')).toBe('');
  });
});

describe('typing', () => {
  it('fills a box and moves on to the next', () => {
    const state = otpType(otpEmpty(), 0, '4');

    expect(state.digits[0]).toBe('4');
    expect(state.focus).toBe(1);
  });

  it('fills the six boxes one digit after the other, and stays on the last', () => {
    const state = typeAll('123456');

    expect(otpCode(state)).toBe('123456');
    expect(otpComplete(state)).toBe(true);
    expect(state.focus).toBe(OTP_LENGTH - 1);
  });

  it('ignores anything but a digit', () => {
    const state = typeAll('1');

    expect(otpType(state, state.focus, 'a')).toBe(state);
    expect(otpType(state, state.focus, ' ')).toBe(state);
    expect(otpType(state, state.focus, '-')).toBe(state);
  });

  it('replaces the digit of a filled box', () => {
    const state = otpType(typeAll('123456'), 2, '9');

    expect(otpCode(state)).toBe('129456');
    expect(state.focus).toBe(3);
  });

  it('takes several characters at once (autofill) as a paste', () => {
    expect(otpCode(otpType(otpEmpty(), 0, '123456'))).toBe('123456');
  });
});

describe('Backspace and Delete', () => {
  it('clears the digit of the box and stays there', () => {
    const state = otpBackspace(typeAll('123'), 2);

    expect(state.digits.slice(0, 3)).toEqual(['1', '2', '']);
    expect(state.focus).toBe(2);
  });

  it('goes back to the previous box when the box is empty, and clears it', () => {
    const state = otpBackspace(typeAll('12'), 2);

    expect(otpCode(state)).toBe('1');
    expect(state.focus).toBe(1);
  });

  it('stays on the first box at the start', () => {
    const state = otpBackspace(otpEmpty(), 0);

    expect(state.focus).toBe(0);
    expect(otpCode(state)).toBe('');
  });

  it('clears the box with Delete without moving', () => {
    const state = otpDelete(typeAll('123456'), 3);

    expect(state.digits[3]).toBe('');
    expect(state.focus).toBe(3);
    expect(otpComplete(state)).toBe(false);
  });
});

describe('arrows, Home and End', () => {
  it('moves one box, stopping at both ends', () => {
    const state = typeAll('123');

    expect(otpMove(state, 3, -1).focus).toBe(2);
    expect(otpMove(state, 3, 1).focus).toBe(4);
    expect(otpMove(state, 0, -1).focus).toBe(0);
    expect(otpMove(state, OTP_LENGTH - 1, 1).focus).toBe(OTP_LENGTH - 1);
  });

  it('jumps to a box', () => {
    expect(otpMoveTo(otpEmpty(), 5).focus).toBe(5);
    expect(otpMoveTo(otpEmpty(), 99).focus).toBe(5);
    expect(otpMoveTo(otpEmpty(), -3).focus).toBe(0);
  });

  it('keeps the digits', () => {
    expect(otpCode(otpMove(typeAll('123'), 1, 1))).toBe('123');
  });
});

describe('paste', () => {
  it('fills the six boxes with six digits', () => {
    const state = otpPaste(otpEmpty(), 0, '123456');

    expect(otpCode(state)).toBe('123456');
    expect(state.focus).toBe(OTP_LENGTH - 1);
  });

  it('ignores spaces and other characters', () => {
    expect(otpCode(otpPaste(otpEmpty(), 0, ' 123 456\n'))).toBe('123456');
    expect(otpCode(otpPaste(otpEmpty(), 0, '12a-34'))).toBe('1234');
  });

  it('fills the six boxes whatever the focused one, when the code is complete', () => {
    expect(otpCode(otpPaste(typeAll('9'), 4, '123456'))).toBe('123456');
  });

  it('drops the digits beyond the sixth', () => {
    expect(otpCode(otpPaste(otpEmpty(), 0, '12345678'))).toBe('123456');
  });

  it('fills from the focused box when shorter', () => {
    const state = otpPaste(otpFromCode('1'), 1, '23');

    expect(otpCode(state)).toBe('123');
    expect(state.focus).toBe(3);
  });

  it('overwrites the boxes it covers and stops at the last one', () => {
    const state = otpPaste(otpFromCode('123456'), 4, '789');

    expect(otpCode(state)).toBe('123478');
    expect(state.focus).toBe(OTP_LENGTH - 1);
  });

  it('changes nothing without a digit', () => {
    const state = typeAll('12');

    expect(otpPaste(state, 2, 'abc ')).toBe(state);
  });
});

describe('select all and cut', () => {
  it('replaces the whole code with the next digit typed', () => {
    const state = otpType(otpSelectAll(otpFromCode('123456')), 3, '7');

    expect(otpCode(state)).toBe('7');
    expect(state.focus).toBe(1);
    expect(state.allSelected).toBe(false);
  });

  it('replaces the whole code with a paste', () => {
    expect(otpCode(otpPaste(otpSelectAll(otpFromCode('123456')), 2, '98'))).toBe('98');
  });

  it('clears everything with Backspace or Delete', () => {
    expect(otpCode(otpBackspace(otpSelectAll(otpFromCode('123456')), 3))).toBe('');
    expect(otpCode(otpDelete(otpSelectAll(otpFromCode('123456')), 3))).toBe('');
    expect(otpBackspace(otpSelectAll(otpFromCode('123456')), 3).focus).toBe(0);
  });

  it('drops the selection when the focus moves', () => {
    expect(otpMove(otpSelectAll(otpFromCode('12')), 1, 1).allSelected).toBe(false);
    expect(otpMoveTo(otpSelectAll(otpFromCode('12')), 0).allSelected).toBe(false);
  });

  it('cuts the whole code after a select all', () => {
    const cut = otpCut(otpSelectAll(otpFromCode('123456')), 2);

    expect(cut.text).toBe('123456');
    expect(otpCode(cut.state)).toBe('');
  });

  it('cuts only the digit of the box otherwise', () => {
    const cut = otpCut(otpFromCode('123456'), 2);

    expect(cut.text).toBe('3');
    expect(cut.state.digits[2]).toBe('');
    expect(cut.state.focus).toBe(2);
    expect(otpCode(cut.state)).toBe('12456');
  });
});

describe('completeness', () => {
  it('is complete only with a digit in every box', () => {
    expect(otpComplete(otpEmpty())).toBe(false);
    expect(otpComplete(typeAll('12345'))).toBe(false);
    expect(otpComplete(typeAll('123456'))).toBe(true);
    expect(otpComplete(otpBackspace(typeAll('123456'), 0))).toBe(false);
  });
});
