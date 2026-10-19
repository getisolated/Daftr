import { Injectable, signal } from '@angular/core';
import { clampSidebarWidth, SIDEBAR_DEFAULT_WIDTH } from '../utils/folder-tree.util';

const STORAGE_KEY = 'evnote-sidebar';

interface PersistedSidebarState {
  visible: boolean;
  width: number;
  expandedIds: number[];
  selectedFolderId: number | null;
  /** Heights set by dragging the separators, by section key; a section missing here takes its natural height. */
  sectionHeights: Record<string, number>;
}

function readHeights(value: unknown): Record<string, number> {
  if (typeof value !== 'object' || value === null) {
    return {};
  }

  const entries = Object.entries(value).filter((entry): entry is [string, number] => typeof entry[1] === 'number' && Number.isFinite(entry[1]));
  return Object.fromEntries(entries);
}

/** Layout and navigation state of the sidebar, restored from the localStorage at startup. */
@Injectable({ providedIn: 'root' })
export class SidebarStateService {
  private readonly restored = this.load();

  private readonly _visible = signal(this.restored.visible);
  private readonly _width = signal(this.restored.width);
  private readonly _expandedIds = signal<ReadonlySet<number>>(new Set(this.restored.expandedIds));
  private readonly _selectedFolderId = signal<number | null>(this.restored.selectedFolderId);
  private readonly _activeTag = signal<string | null>(null);
  private readonly _sectionHeights = signal<Readonly<Record<string, number>>>(this.restored.sectionHeights);

  readonly visible = this._visible.asReadonly();
  readonly width = this._width.asReadonly();
  readonly expandedIds = this._expandedIds.asReadonly();
  /** Folder whose notes are listed on the home page; `null` lists every note. */
  readonly selectedFolderId = this._selectedFolderId.asReadonly();
  readonly activeTag = this._activeTag.asReadonly();
  readonly sectionHeights = this._sectionHeights.asReadonly();

  toggleVisible(): void {
    this._visible.update(visible => !visible);
    this.persist();
  }

  /** Called on every pointer move while resizing: only the final value is worth persisting. */
  setWidth(width: number, persist = true): void {
    this._width.set(clampSidebarWidth(width));

    if (persist) {
      this.persist();
    }
  }

  /** `null` gives the section its natural height back. Persisted only at the end of the drag, like the width. */
  setSectionHeight(key: string, height: number | null, persist = true): void {
    const next = { ...this._sectionHeights() };

    if (height === null) {
      delete next[key];
    } else {
      next[key] = height;
    }

    this._sectionHeights.set(next);

    if (persist) {
      this.persist();
    }
  }

  toggleExpanded(folderId: number): void {
    const next = new Set(this._expandedIds());

    if (!next.delete(folderId)) {
      next.add(folderId);
    }

    this._expandedIds.set(next);
    this.persist();
  }

  expand(folderId: number): void {
    if (this._expandedIds().has(folderId)) {
      return;
    }

    this._expandedIds.set(new Set(this._expandedIds()).add(folderId));
    this.persist();
  }

  collapseAll(): void {
    this._expandedIds.set(new Set());
    this.persist();
  }

  selectFolder(folderId: number | null): void {
    this._selectedFolderId.set(folderId);
    this.persist();
  }

  setActiveTag(tag: string | null): void {
    this._activeTag.set(tag);
  }

  /** Forgets the folders that no longer exist (deleted from another window, or by a previous session). */
  prune(existingFolderIds: ReadonlySet<number>): void {
    const kept = [...this._expandedIds()].filter(id => existingFolderIds.has(id));

    if (kept.length !== this._expandedIds().size) {
      this._expandedIds.set(new Set(kept));
    }

    const selected = this._selectedFolderId();

    if (selected !== null && !existingFolderIds.has(selected)) {
      this._selectedFolderId.set(null);
    }

    this.persist();
  }

  private persist(): void {
    const state: PersistedSidebarState = {
      visible: this._visible(),
      width: this._width(),
      expandedIds: [...this._expandedIds()],
      selectedFolderId: this._selectedFolderId(),
      sectionHeights: this._sectionHeights(),
    };

    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch { /* storage full or unavailable: the sidebar just starts from defaults next time */ }
  }

  private load(): PersistedSidebarState {
    const fallback: PersistedSidebarState = {
      visible: true,
      width: SIDEBAR_DEFAULT_WIDTH,
      expandedIds: [],
      selectedFolderId: null,
      sectionHeights: {},
    };

    try {
      const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<PersistedSidebarState> | null;

      if (!raw) {
        return fallback;
      }

      return {
        visible: typeof raw.visible === 'boolean' ? raw.visible : fallback.visible,
        width: clampSidebarWidth(raw.width),
        expandedIds: Array.isArray(raw.expandedIds) ? raw.expandedIds.filter(id => typeof id === 'number') : [],
        selectedFolderId: typeof raw.selectedFolderId === 'number' ? raw.selectedFolderId : null,
        sectionHeights: readHeights(raw.sectionHeights),
      };
    } catch {
      return fallback;
    }
  }
}
