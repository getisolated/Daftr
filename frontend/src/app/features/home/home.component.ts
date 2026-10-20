import {
  Component, inject, signal, computed, ChangeDetectionStrategy
} from '@angular/core';

import { FoldersService } from '../../core/services/folders.service';
import { CollectionsService } from '../../core/services/collections.service';
import { SidebarStateService } from '../../core/services/sidebar-state.service';
import { NotesService } from '../../core/services/notes.service';
import { TabsService } from '../../core/services/tabs.service';
import { HomeView, LayoutService } from '../../core/services/layout.service';
import { SettingsService } from '../../core/services/settings.service';
import { I18nService } from '../../core/i18n/i18n.service';
import { NotePreview } from '../../core/models/note.model';
import { parseNoteDate } from '../../core/utils/note-timestamp.util';
import { noteDisplayTitle } from '../../core/utils/note-title.util';
import { QUICK_NOTE_TAG } from '../../core/models/settings.model';
import { GraphViewComponent } from '../graph/graph-view.component';
import { ContextMenuComponent, ContextMenuEntry } from '../../shared/context-menu/context-menu.component';
import { DaftrIconComponent } from '../../shared/ui/brand.component';

@Component({
  selector: 'app-home',
  standalone: true,
  imports: [ContextMenuComponent, GraphViewComponent, DaftrIconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="home" [class.graph-mode]="view() === 'graph'">
      <div class="home-inner">
      <div class="home-header">
        <div class="home-brand">
          <app-daftr-icon class="brand-logo" [size]="28" />
          <h1 class="home-title">{{ title() }}</h1>
        </div>
        <div class="view-switch" role="group" [attr.aria-label]="i18n.t('workspace.home.viewSwitch')">
          <button type="button" class="view-btn" [class.active]="view() === 'graph'" (click)="setView('graph')">{{ i18n.t('workspace.home.viewGraph') }}</button>
          <button type="button" class="view-btn" [class.active]="view() === 'list'" (click)="setView('list')">{{ i18n.t('workspace.home.viewList') }}</button>
        </div>
      </div>

      @if (view() === 'graph') {
        <app-graph-view />
      } @else {
      <div class="notes-list">
        @if (filteredNotes().length === 0) {
          <div class="empty-state">
            <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
            </svg>
            <p>{{ i18n.t('workspace.home.emptyBefore') }} <kbd>{{ shortcutLabel('note.new') }}</kbd> {{ i18n.t('workspace.home.emptyAfter') }}</p>
          </div>
        }
        @for (note of filteredNotes(); track note.id) {
          <div
            class="note-row"
            (click)="openNote(note)"
            (contextmenu)="onNoteContextMenu($event, note)"
            tabindex="0"
            (keydown.enter)="openNote(note)"
          >
            <div class="note-main">
              <span class="note-title">{{ displayTitle(note) }}</span>
              @if (note.tags.length > 0) {
                <div class="note-tags">
                  @for (tag of note.tags.slice(0, 3); track tag) {
                    <span class="tag-bubble">{{ tag }}</span>
                  }
                  @if (note.tags.length > 3) {
                    <span class="tag-bubble overflow">+{{ note.tags.length - 3 }}</span>
                  }
                </div>
              }
            </div>
            <span class="note-date" [title]="note.updatedAt">{{ formatDate(note.updatedAt) }}</span>
          </div>
        }
      </div>
      }
      </div>
    </div>

    @if (ctxMenu()) {
      <app-context-menu
        [items]="ctxMenu()!.items"
        [x]="ctxMenu()!.x"
        [y]="ctxMenu()!.y"
        (action)="onCtxAction($event)"
        (closed)="ctxMenu.set(null)"
      />
    }
  `,
  styleUrls: ['./home.component.scss'],
})
export class HomeComponent {
  protected readonly i18n = inject(I18nService);
  private notesService = inject(NotesService);
  private settingsService = inject(SettingsService);
  private tabsService = inject(TabsService);
  private foldersService = inject(FoldersService);
  private sidebarState = inject(SidebarStateService);
  private readonly collections = inject(CollectionsService);
  private layout = inject(LayoutService);

  readonly view = this.layout.homeView;

  /** The folder (and the tag) selected in the sidebar drive what is listed here. */
  readonly title = computed(() => {
    const collection = this.collections.selectedState();

    if (collection) {
      return collection.label;
    }

    const folderId = this.sidebarState.selectedFolderId();
    const tag = this.sidebarState.activeTag();
    const allNotes = this.i18n.t('workspace.home.allNotes');
    const base = folderId === null ? allNotes : (this.foldersService.getById(folderId)?.name ?? allNotes);

    return tag ? `${base} · #${tag}` : base;
  });

  readonly filteredNotes = computed<NotePreview[]>(() => {
    const tag = this.sidebarState.activeTag();
    const folderId = this.sidebarState.selectedFolderId();
    const previews = this.notesService.notePreviews();
    // Quick notes are not listed one by one: they live together in the "Quick notes" document
    return previews.filter(n =>
      !n.tags.includes(QUICK_NOTE_TAG)
      && (folderId === null || n.folderId === folderId)
      && (!tag || n.tags.includes(tag))
    );
  });

  // ── Context menu ───────────────────────────────────────────────────────────
  readonly ctxMenu = signal<{ items: ContextMenuEntry[]; x: number; y: number; noteId: number } | null>(null);

  setView(view: HomeView): void {
    this.layout.setHomeView(view);
  }

  openNote(note: NotePreview): void {
    this.tabsService.openNote(note.id, note.title);
  }

  protected displayTitle(note: NotePreview): string {
    return noteDisplayTitle(note.title, this.i18n.t);
  }

  protected shortcutLabel(id: string): string {
    return this.settingsService.shortcutLabel(id);
  }

  onNoteContextMenu(event: MouseEvent, note: NotePreview): void {
    event.preventDefault();
    event.stopPropagation();

    const deleteIcon = 'M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2';

    const items: ContextMenuEntry[] = [
      { id: 'delete', label: this.i18n.t('workspace.home.deleteNote'), icon: deleteIcon, danger: true },
    ];

    this.ctxMenu.set({ items, x: event.clientX, y: event.clientY, noteId: note.id });
  }

  async onCtxAction(actionId: string): Promise<void> {
    const menu = this.ctxMenu();
    if (!menu) return;

    if (actionId === 'delete') {
      this.tabsService.closeByNoteId(menu.noteId);
      await this.notesService.delete(menu.noteId);
    }
    this.ctxMenu.set(null);
  }

  formatDate(isoDate: string): string {
    if (!isoDate) return '';
    const d = parseNoteDate(isoDate);
    if (isNaN(d.getTime())) return '';
    const now = new Date();
    const isToday =
      d.getDate() === now.getDate() &&
      d.getMonth() === now.getMonth() &&
      d.getFullYear() === now.getFullYear();
    if (isToday) {
      return d.toLocaleTimeString(this.i18n.language(), { hour: '2-digit', minute: '2-digit' });
    }
    return d.toLocaleDateString(this.i18n.language(), { month: 'short', day: 'numeric' });
  }
}
