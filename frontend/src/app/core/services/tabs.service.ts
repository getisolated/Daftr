import { Injectable, signal, computed, inject, effect, untracked } from '@angular/core';
import { CollectionFileRef } from '../models/collection.model';
import { QUICK_NOTES_TAB_ID, QUICK_NOTES_TITLE, Tab } from '../models/note.model';
import { FileDocsService } from './file-docs.service';
import { SettingsService } from './settings.service';
import { closeTabsWhere, TabsAfterClosing } from '../utils/tabs-closing.util';
import { keepTabs, openTab } from '../utils/tabs-opening.util';

const SESSION_KEY = 'daftr-session';

@Injectable({ providedIn: 'root' })
export class TabsService {
  private readonly fileDocs = inject(FileDocsService);
  private readonly settings = inject(SettingsService);
  private readonly _tabs = signal<Tab[]>([]);
  private readonly _activeIndex = signal<number>(-1);
  private _initialized = false;

  constructor() {
    // Setting turned off (or loaded off after the session was restored): the temporary tabs become regular ones
    effect(() => {
      if (!this.settings.settings().temporaryTabs) {
        untracked(() => this.keepWhere(() => true));
      }
    });
  }

  readonly tabs = this._tabs.asReadonly();
  readonly activeIndex = this._activeIndex.asReadonly();

  readonly activeTab = computed<Tab | null>(() => {
    const idx = this._activeIndex();
    const tabs = this._tabs();
    return idx >= 0 && idx < tabs.length ? tabs[idx] : null;
  });

  readonly activeNoteId = computed<number | null>(() => {
    return this.activeTab()?.noteId ?? null;
  });

  isOpen(noteId: number): boolean {
    return this._tabs().some(t => t.noteId === noteId);
  }

  /** Number of pinned tabs (always at the start of the array). */
  readonly pinnedCount = computed(() =>
    this._tabs().filter(t => t.isPinned).length
  );

  // ── Session persistence ────────────────────────────────────────────────────

  restoreSession(validNoteIds: Set<number>): void {
    try {
      const raw = localStorage.getItem(SESSION_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as { tabs: Tab[]; activeIndex: number };
        const validTabs = parsed.tabs
          .filter(t => validNoteIds.has(t.noteId) || t.noteId === QUICK_NOTES_TAB_ID || !!t.file)
          // File ids only last a session: the saved file gets a fresh one (a missing file shows an error when opened).
          // Sessions saved before the rename still say "Overlay notes"
          .map(t => ({
            ...t,
            isPinned: t.isPinned ?? false,
            isDirty: false,
            isTemporary: t.isTemporary === true && this.settings.settings().temporaryTabs,
            noteId: t.file ? this.fileDocs.idFor(t.file) : t.noteId,
            title: t.noteId === QUICK_NOTES_TAB_ID ? QUICK_NOTES_TITLE : t.title,
          }));
        if (validTabs.length > 0) {
          this._tabs.set(validTabs);
          const clamped = Math.min(Math.max(parsed.activeIndex, -1), validTabs.length - 1);
          this._activeIndex.set(clamped);
        }
      }
    } catch { /* ignore corrupt storage */ }
    this._initialized = true;
  }

  private persist(): void {
    if (!this._initialized) return;
    localStorage.setItem(SESSION_KEY, JSON.stringify({
      tabs: this._tabs(),
      activeIndex: this._activeIndex(),
    }));
  }

  // ── Navigation ─────────────────────────────────────────────────────────────

  goHome(): void {
    this._activeIndex.set(-1);
    this.persist();
  }

  /** Opens a note in a temporary tab (setting `temporaryTabs`): it is kept once edited, pinned or double-clicked. */
  openNote(noteId: number, title: string, file?: CollectionFileRef): void {
    this.open({ noteId, title, isDirty: false, isPinned: false, ...(file ? { file } : {}) }, this.settings.settings().temporaryTabs);
  }

  /** A note just created is meant to be written: its tab is kept from the start. */
  openNewNote(noteId: number, title: string): void {
    this.open({ noteId, title, isDirty: false, isPinned: false }, false);
  }

  private open(tab: Tab, temporary: boolean): void {
    const result = openTab(this._tabs(), tab, temporary);
    this._tabs.set(result.tabs);
    this._activeIndex.set(result.activeIndex);
    this.persist();
  }

  /** Double-click on a temporary tab: it stays open when the next note is opened. */
  keepTab(index: number): void {
    const tab = this._tabs()[index];
    if (tab) this.keepWhere(t => t.noteId === tab.noteId);
  }

  private keepWhere(shouldKeep: (tab: Tab) => boolean): void {
    const tabs = this._tabs();
    const kept = keepTabs(tabs, shouldKeep);
    if (kept !== tabs) {
      this._tabs.set(kept);
      this.persist();
    }
  }

  /** A file of a collection was renamed or moved: its tab follows. */
  updateFile(noteId: number, file: CollectionFileRef, title: string): void {
    this._tabs.update(tabs => tabs.map(t => (t.noteId === noteId ? { ...t, file, title } : t)));
    this.persist();
  }

  openQuickNotes(): void {
    this.openNote(QUICK_NOTES_TAB_ID, QUICK_NOTES_TITLE);
  }

  closeTab(index: number): void {
    const tabs = this._tabs();
    if (index < 0 || index >= tabs.length) return;
    // Cannot close a pinned tab
    if (tabs[index].isPinned) return;

    this._tabs.update(t => t.filter((_, i) => i !== index));

    const newTabs = this._tabs();
    if (newTabs.length === 0) {
      this._activeIndex.set(-1);
    } else {
      const currentActive = this._activeIndex();
      if (index < currentActive) {
        this._activeIndex.set(currentActive - 1);
      } else if (index === currentActive) {
        this._activeIndex.set(Math.min(index, newTabs.length - 1));
      }
    }
    this.persist();
  }

  closeActiveTab(): void {
    this.closeTab(this._activeIndex());
  }

  closeByNoteId(noteId: number): void {
    const idx = this._tabs().findIndex(t => t.noteId === noteId);
    if (idx >= 0) this.closeTab(idx);
  }

  /** Closes every tab but the one at `index` (pinned tabs stay), which becomes the active one when needed. */
  closeOthers(index: number): void {
    const kept = this._tabs()[index];
    if (!kept) return;
    this.applyClosing(closeTabsWhere(this._tabs(), this.activeNoteId(), t => t.noteId !== kept.noteId, kept.noteId));
  }

  /** Closes every tab except the pinned ones. */
  closeAll(): void {
    this.applyClosing(closeTabsWhere(this._tabs(), this.activeNoteId(), () => true));
  }

  private applyClosing(result: TabsAfterClosing): void {
    this._tabs.set(result.tabs);
    this._activeIndex.set(result.activeIndex);
    this.persist();
  }

  nextTab(): void {
    const len = this._tabs().length;
    if (len === 0) return;
    this._activeIndex.update(i => (i + 1) % len);
    this.persist();
  }

  prevTab(): void {
    const len = this._tabs().length;
    if (len === 0) return;
    this._activeIndex.update(i => (i - 1 + len) % len);
    this.persist();
  }

  setActiveIndex(index: number): void {
    if (index >= 0 && index < this._tabs().length) {
      this._activeIndex.set(index);
      this.persist();
    }
  }

  /** An edited note keeps its tab: it is no longer temporary. */
  markDirty(noteId: number, dirty: boolean): void {
    if (dirty) this.keepWhere(t => t.noteId === noteId);
    this._tabs.update(tabs =>
      tabs.map(t => t.noteId === noteId ? { ...t, isDirty: dirty } : t)
    );
  }

  updateTitle(noteId: number, title: string): void {
    this._tabs.update(tabs =>
      tabs.map(t => t.noteId === noteId ? { ...t, title } : t)
    );
    this.persist();
  }

  // ── Pin / Unpin ────────────────────────────────────────────────────────────

  togglePin(index: number): void {
    const tabs = this._tabs();
    if (index < 0 || index >= tabs.length) return;
    const tab = tabs[index];
    const newPinned = !tab.isPinned;

    // Update the pin state (a pinned tab is never temporary)
    const updated = tabs.map((t, i) => i === index ? { ...t, isPinned: newPinned, isTemporary: false } : t);

    if (newPinned) {
      // Move tab to end of pinned section
      const [pinned] = updated.splice(index, 1);
      const pinnedCount = updated.filter(t => t.isPinned).length;
      updated.splice(pinnedCount, 0, pinned);
      // Update active index to follow the moved tab
      this._tabs.set(updated);
      this._activeIndex.set(updated.findIndex(t => t.noteId === tab.noteId));
    } else {
      // Move tab to start of unpinned section (right after last pinned)
      const [unpinned] = updated.splice(index, 1);
      const pinnedCount = updated.filter(t => t.isPinned).length;
      updated.splice(pinnedCount, 0, unpinned);
      this._tabs.set(updated);
      this._activeIndex.set(updated.findIndex(t => t.noteId === tab.noteId));
    }
    this.persist();
  }

  togglePinActiveTab(): void {
    const idx = this._activeIndex();
    if (idx >= 0) this.togglePin(idx);
  }

  isPinned(index: number): boolean {
    const tabs = this._tabs();
    return index >= 0 && index < tabs.length && tabs[index].isPinned;
  }

  // ── Drag reorder ───────────────────────────────────────────────────────────

  moveTab(fromIndex: number, toIndex: number): void {
    if (fromIndex === toIndex) return;
    const tabs = [...this._tabs()];
    if (fromIndex < 0 || fromIndex >= tabs.length) return;
    if (toIndex < 0 || toIndex >= tabs.length) return;

    const movingTab = tabs[fromIndex];
    const targetTab = tabs[toIndex];

    // Pinned tabs can only move within pinned zone, unpinned within unpinned zone
    if (movingTab.isPinned !== targetTab.isPinned) return;

    const activeNoteId = this.activeNoteId();
    const [moved] = tabs.splice(fromIndex, 1);
    tabs.splice(toIndex, 0, moved);
    this._tabs.set(tabs);

    // Keep active index following the active tab
    if (activeNoteId !== null) {
      const newIdx = tabs.findIndex(t => t.noteId === activeNoteId);
      if (newIdx >= 0) this._activeIndex.set(newIdx);
    }
    this.persist();
  }
}
