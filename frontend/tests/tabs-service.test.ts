import { describe, it, expect, beforeEach, vi } from 'vitest';
import { Injector } from '@angular/core';
import { FileDocsService } from '../src/app/core/services/file-docs.service';
import { TabsService } from '../src/app/core/services/tabs.service';

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

describe('TabsService view modes', () => {
  const createService = (): TabsService => Injector.create({
    providers: [
      { provide: FileDocsService, useClass: FileDocsService, deps: [] },
      { provide: TabsService, useClass: TabsService, deps: [] },
    ],
  }).get(TabsService);

  beforeEach(() => {
    localStorage.clear();
  });

  it('remembers the view mode of each tab', () => {
    const service = createService();
    service.restoreSession(new Set([1, 2]));
    service.openNote(1, 'Note 1');
    service.openNote(2, 'Note 2');
    service.setViewMode(1, 'preview');

    expect(service.tabs().map(t => t.viewMode)).toEqual(['preview', undefined]);
  });

  it('restores the view modes of the session and drops unknown ones', () => {
    localStorage.setItem('daftr-session', JSON.stringify({
      tabs: [
        { noteId: 1, title: 'Note 1', isDirty: false, isPinned: false, viewMode: 'split' },
        { noteId: 2, title: 'Note 2', isDirty: false, isPinned: false, viewMode: 'wysiwyg' },
      ],
      activeIndex: 0,
    }));
    const service = createService();
    service.restoreSession(new Set([1, 2]));

    expect(service.tabs()[0]!.viewMode).toBe('split');
    expect(service.tabs()[1]).not.toHaveProperty('viewMode');
  });

  it('saves the view modes with the session', () => {
    const service = createService();
    service.restoreSession(new Set([1]));
    service.openNote(1, 'Note 1');
    service.setViewMode(1, 'preview');

    const saved = JSON.parse(localStorage.getItem('daftr-session') ?? '{}') as { tabs: { viewMode?: string }[] };
    expect(saved.tabs[0]!.viewMode).toBe('preview');
  });
});
