import { readFileSync } from 'node:fs';
import { describe, it, expect } from 'vitest';
import { THEMES } from '../src/app/core/models/theme.model';
import { THEME_KINDS } from '../electron/settings-schema';
import { buildThemePickerItems, findTheme, resolveTheme, themeKind } from '../src/app/core/utils/theme.util';

const systemThemes = { dark: 'github-dark-dimmed', light: 'solarized-light' } as const;

describe('resolveTheme', () => {
  it('keeps an explicit preference whatever the OS says', () => {
    expect(resolveTheme('dark', false, systemThemes)).toBe('dark');
    expect(resolveTheme('violet-light', true, systemThemes)).toBe('violet-light');
  });

  it('uses the preferred theme of the OS color scheme in system mode', () => {
    expect(resolveTheme('system', true, systemThemes)).toBe('github-dark-dimmed');
    expect(resolveTheme('system', false, systemThemes)).toBe('solarized-light');
  });
});

describe('themeKind', () => {
  it('returns the family of a theme', () => {
    expect(themeKind('github-dark-dimmed')).toBe('dark');
    expect(themeKind('github-light')).toBe('light');
  });

  it('falls back to dark for an unknown id, like the default block of styles.scss', () => {
    expect(themeKind('nope' as never)).toBe('dark');
    expect(findTheme('nope' as never)).toBeUndefined();
  });
});

describe('buildThemePickerItems', () => {
  it('lists system first, then every theme', () => {
    const items = buildThemePickerItems(systemThemes, 'dark');
    expect(items.map(item => item.id)).toEqual(['system', ...THEMES.map(theme => theme.id)]);
    expect(items[0]?.detail).toBe('Follows Windows: GitHub Dark Dimmed / Solarized Light');
  });

  it('flags the current choice only', () => {
    const items = buildThemePickerItems(systemThemes, 'violet-light');
    expect(items.filter(item => item.detail.endsWith('· Current')).map(item => item.id)).toEqual(['violet-light']);
    expect(items.find(item => item.id === 'dark')?.detail).toBe('Dark theme');
  });
});

describe('theme registry', () => {
  const styles = readFileSync(new URL('../src/styles.scss', import.meta.url), 'utf-8');
  const blocks = new Map(
    [...styles.matchAll(/:root\[data-theme='(?<id>[a-z-]+)'\] \{(?<body>.*?)\n\}/gs)]
      .map(match => [match.groups?.['id'] ?? '', match.groups?.['body'] ?? '']),
  );
  const variablesOf = (body: string): string[] => [...body.matchAll(/(?<name>--evn-[a-z0-9-]+):/g)].map(match => match.groups?.['name'] ?? '').sort();

  it('has a styles.scss block for every theme, and no orphan block', () => {
    expect([...blocks.keys()].sort()).toEqual(THEMES.map(theme => theme.id).sort());
  });

  it('defines every color token of the dark theme in each theme', () => {
    const reference = variablesOf(blocks.get('dark') ?? '');
    expect(reference.length).toBeGreaterThan(0);
    blocks.forEach(body => expect(variablesOf(body)).toEqual(reference));
  });

  it('matches the families known by the main process', () => {
    expect(Object.fromEntries(THEMES.map(theme => [theme.id, theme.kind]))).toEqual(THEME_KINDS);
  });
});
