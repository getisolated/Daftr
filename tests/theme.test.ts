import { describe, it, expect } from 'vitest';
import { oppositeTheme, resolveTheme } from '../src/app/core/utils/theme.util';

describe('resolveTheme', () => {
  it('keeps an explicit preference whatever the OS says', () => {
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('light', true)).toBe('light');
  });

  it('follows the OS color scheme in system mode', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
  });
});

describe('oppositeTheme', () => {
  it('flips between light and dark', () => {
    expect(oppositeTheme('dark')).toBe('light');
    expect(oppositeTheme('light')).toBe('dark');
  });
});
