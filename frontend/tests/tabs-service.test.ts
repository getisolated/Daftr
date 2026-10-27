import { describe, it, expect, beforeEach, vi } from 'vitest';
import { Tab as AppTab } from '../src/app/core/models/note.model';
import { keepTabs, openTab } from '../src/app/core/utils/tabs-opening.util';

// Mock localStorage
const store: Record<string, string> = {};
vi.stubGlobal('localStorage', {
  getItem: (key: string) => store[key] ?? null,
  setItem: (key: string, value: string) => { store[key] = value; },
  removeItem: (key: string) => { delete store[key]; },
  clear: () => { Object.keys(store).forEach(k => delete store[k]); },
});

// Minimal signal implementation for testing outside Angular
function signal<T>(initial: T) {
  let value = initial;
  const fn = () => value;
  fn.set = (v: T) => { value = v; };
  fn.update = (updater: (v: T) => T) => { value = updater(value); };
  fn.asReadonly = () => fn;
  return fn;
}

function computed<T>(fn: () => T) {
  return fn;
}

interface Tab {
  noteId: number;
  title: string;
  isDirty: boolean;
  isPinned: boolean;
}

// Inline a minimal TabsService to test pure logic without Angular DI
class TestTabsService {
  private _tabs = signal<Tab[]>([]);
  private _activeIndex = signal(-1);

  readonly tabs = this._tabs.asReadonly();
  readonly activeIndex = this._activeIndex.asReadonly();
  readonly activeNoteId = computed(() => {
    const idx = this._activeIndex();
    return idx >= 0 ? this._tabs()[idx]?.noteId ?? null : null;
  });

  openNote(noteId: number, title: string): void {
    const tabs = this._tabs();
    const existingIdx = tabs.findIndex(t => t.noteId === noteId);
    if (existingIdx >= 0) {
      this._activeIndex.set(existingIdx);
      return;
    }
    const newTab: Tab = { noteId, title, isDirty: false, isPinned: false };
    this._tabs.update(list => [...list, newTab]);
    this._activeIndex.set(this._tabs().length - 1);
  }

  closeTab(index: number): void {
    const tabs = this._tabs();
    if (index < 0 || index >= tabs.length) return;
    if (tabs[index].isPinned) return;
    this._tabs.update(list => list.filter((_, i) => i !== index));
    const newLen = this._tabs().length;
    if (newLen === 0) {
      this._activeIndex.set(-1);
    } else if (this._activeIndex() >= newLen) {
      this._activeIndex.set(newLen - 1);
    }
  }

  markDirty(noteId: number, dirty: boolean): void {
    this._tabs.update(list =>
      list.map(t => t.noteId === noteId ? { ...t, isDirty: dirty } : t)
    );
  }

  togglePin(index: number): void {
    this._tabs.update(list =>
      list.map((t, i) => i === index ? { ...t, isPinned: !t.isPinned } : t)
    );
  }
}

describe('TabsService', () => {
  let service: TestTabsService;

  beforeEach(() => {
    service = new TestTabsService();
  });

  it('starts with no tabs and home view (index -1)', () => {
    expect(service.tabs()).toHaveLength(0);
    expect(service.activeIndex()).toBe(-1);
    expect(service.activeNoteId()).toBeNull();
  });

  it('opens a new tab and sets it active', () => {
    service.openNote(1, 'Note 1');
    expect(service.tabs()).toHaveLength(1);
    expect(service.activeIndex()).toBe(0);
    expect(service.activeNoteId()).toBe(1);
  });

  it('does not duplicate tabs for the same note', () => {
    service.openNote(1, 'Note 1');
    service.openNote(2, 'Note 2');
    service.openNote(1, 'Note 1');
    expect(service.tabs()).toHaveLength(2);
    expect(service.activeIndex()).toBe(0);
  });

  it('closes a tab and adjusts active index', () => {
    service.openNote(1, 'Note 1');
    service.openNote(2, 'Note 2');
    service.closeTab(1);
    expect(service.tabs()).toHaveLength(1);
    expect(service.tabs()[0].noteId).toBe(1);
  });

  it('cannot close a pinned tab', () => {
    service.openNote(1, 'Note 1');
    service.togglePin(0);
    service.closeTab(0);
    expect(service.tabs()).toHaveLength(1);
  });

  it('marks a tab as dirty', () => {
    service.openNote(1, 'Note 1');
    expect(service.tabs()[0].isDirty).toBe(false);
    service.markDirty(1, true);
    expect(service.tabs()[0].isDirty).toBe(true);
    service.markDirty(1, false);
    expect(service.tabs()[0].isDirty).toBe(false);
  });

  it('returns to home when last tab is closed', () => {
    service.openNote(1, 'Note 1');
    service.closeTab(0);
    expect(service.tabs()).toHaveLength(0);
    expect(service.activeIndex()).toBe(-1);
    expect(service.activeNoteId()).toBeNull();
  });

  it('toggles pin state', () => {
    service.openNote(1, 'Note 1');
    expect(service.tabs()[0].isPinned).toBe(false);
    service.togglePin(0);
    expect(service.tabs()[0].isPinned).toBe(true);
    service.togglePin(0);
    expect(service.tabs()[0].isPinned).toBe(false);
  });
});

function appTab(noteId: number, extra: Partial<AppTab> = {}): AppTab {
  return { noteId, title: `n${noteId}`, isDirty: false, isPinned: false, ...extra };
}

/** Opens the notes one after the other, as TabsService.openNote does. */
function openAll(noteIds: number[], temporary: boolean, tabs: AppTab[] = []): { tabs: AppTab[]; activeIndex: number } {
  let state = { tabs, activeIndex: -1 };
  for (const id of noteIds) state = openTab(state.tabs, appTab(id), temporary);
  return state;
}

describe('openTab (temporary tabs)', () => {
  it('opens a note in a temporary tab', () => {
    const result = openAll([1], true);

    expect(result.tabs).toEqual([{ ...appTab(1), isTemporary: true }]);
    expect(result.activeIndex).toBe(0);
  });

  it('replaces the temporary tab, at its position, with the next note opened', () => {
    const result = openAll([2, 3], true, [appTab(1)]);

    expect(result.tabs.map(t => t.noteId)).toEqual([1, 3]);
    expect(result.tabs[1]?.isTemporary).toBe(true);
    expect(result.activeIndex).toBe(1);
  });

  it('replaces the temporary tab even when it is not the last one nor the active one', () => {
    const tabs = [appTab(1), appTab(2, { isTemporary: true }), appTab(3)];
    const result = openTab(tabs, appTab(4), true);

    expect(result.tabs.map(t => t.noteId)).toEqual([1, 4, 3]);
    expect(result.activeIndex).toBe(1);
  });

  it('shows the tab of a note already open without replacing anything', () => {
    const tabs = [appTab(1), appTab(2, { isTemporary: true })];
    const result = openTab(tabs, appTab(1), true);

    expect(result.tabs).toBe(tabs);
    expect(result.activeIndex).toBe(0);
  });

  it('keeps a tab opened as a regular one (new note, setting off): nothing is replaced', () => {
    const result = openAll([2, 3], false, [appTab(1)]);

    expect(result.tabs.map(t => t.noteId)).toEqual([1, 2, 3]);
    expect(result.tabs.some(t => t.isTemporary)).toBe(false);
  });

  it('opening a temporary note to keep it makes its tab a regular one', () => {
    const result = openTab([appTab(1, { isTemporary: true })], appTab(1), false);

    expect(result.tabs[0]?.isTemporary).toBe(false);
  });

  it('adds a regular tab next to the temporary one without replacing it', () => {
    const result = openTab([appTab(1, { isTemporary: true })], appTab(2), false);

    expect(result.tabs.map(t => [t.noteId, t.isTemporary ?? false])).toEqual([[1, true], [2, false]]);
  });
});

describe('keepTabs', () => {
  it('makes the matching temporary tabs regular ones (edited, pinned, double-clicked)', () => {
    const tabs = [appTab(1, { isTemporary: true }), appTab(2)];
    const kept = keepTabs(tabs, t => t.noteId === 1);

    expect(kept[0]?.isTemporary).toBe(false);
    // A kept tab is no longer replaced
    expect(openTab(kept, appTab(3), true).tabs.map(t => t.noteId)).toEqual([1, 2, 3]);
  });

  it('returns the same array when there is nothing to keep', () => {
    const tabs = [appTab(1), appTab(2, { isTemporary: true })];

    expect(keepTabs(tabs, t => t.noteId === 1)).toBe(tabs);
  });
});
