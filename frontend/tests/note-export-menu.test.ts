import { describe, it, expect } from 'vitest';
import {
  folderExportMenu,
  isPdfExportAvailable,
  noteExportMenu,
  selectionExportMenu,
} from '../src/app/core/utils/export-menu.util';
import type { ContextMenuItem } from '../src/app/shared/context-menu/context-menu.component';

const t = ((key: string) => key) as Parameters<typeof noteExportMenu>[0];
const ids = (items: readonly ContextMenuItem[] | undefined): string[] => (items ?? []).map(item => item.id);

describe('isPdfExportAvailable', () => {
  it('is available where the system web view prints (Windows, macOS)', () => {
    expect(isPdfExportAvailable('windows')).toBe(true);
    expect(isPdfExportAvailable('mac')).toBe(true);
  });

  it('is not available on Linux', () => {
    expect(isPdfExportAvailable('linux')).toBe(false);
  });
});

describe('export menus', () => {
  it('offers PDF for a note when available, not otherwise', () => {
    expect(ids(noteExportMenu(t, true).children)).toEqual(['export:copy', 'export:md', 'export:html', 'export:pdf']);
    expect(ids(noteExportMenu(t, false).children)).toEqual(['export:copy', 'export:md', 'export:html']);
  });

  it('offers a PDF archive for a folder when available, not otherwise', () => {
    expect(ids(folderExportMenu(t, true))).toEqual(['export:zip', 'export:pdf-zip']);
    expect(ids(folderExportMenu(t, false))).toEqual(['export:zip']);
  });

  it('offers a PDF archive for a selection when available, not otherwise', () => {
    expect(ids(selectionExportMenu(t, true).children)).toEqual(['export:zip', 'export:pdf-zip']);
    expect(ids(selectionExportMenu(t, false).children)).toEqual(['export:zip']);
  });
});
