import {
  ChangeDetectionStrategy, Component, ElementRef, Injector, afterNextRender, computed, effect, inject, signal, untracked,
} from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { TranslationKey } from '../../core/i18n/translations';
import { CollectionEditing, CollectionRow } from '../../core/models/collection-tree.model';
import { CollectionFileRef, CollectionNode, CollectionState } from '../../core/models/collection.model';
import { CollectionsService } from '../../core/services/collections.service';
import { FileDocsService } from '../../core/services/file-docs.service';
import { TabsService } from '../../core/services/tabs.service';
import { SidebarStateService } from '../../core/services/sidebar-state.service';
import { baseName, parentPath } from '../../core/utils/collection.util';
import { PlatformService } from '../../core/services/platform.service';
import { formatAccelerator } from '../../core/utils/accelerator.util';
import { ContextMenuComponent, ContextMenuEntry } from '../../shared/context-menu/context-menu.component';
import { ConfirmDialogAction, ConfirmDialogComponent } from '../../shared/ui/confirm-dialog.component';
import { FocusSelectDirective } from '../../shared/ui/focus-select.directive';

const EXPANDED_KEY = 'daftr-collections-expanded';
const SECTION_KEY = 'daftr-collections-open';

const ICON_FILE = 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6';
const ICON_RENAME = 'M12 20h9M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4z';
const ICON_DELETE = 'M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2';
const ICON_REVEAL = 'M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2zM12 11v6M9 14l3 3 3-3';
const ICON_REFRESH = 'M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5';
const ICON_REMOVE = 'M18 6 6 18M6 6l12 12';

function nodeKey(root: string, path: string): string {
  return `${root}\u0000${path}`;
}

function readSet(key: string): Set<string> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(key) ?? '[]');
    return new Set(Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === 'string') : []);
  } catch {
    return new Set();
  }
}

/** "Collections" section of the sidebar: folders and markdown files of the disk, apart from the notes. */
/** Messages thrown by the main process (electron/collections.ts) that reach the user. */
const COLLECTION_ERROR_KEYS: Readonly<Record<string, TranslationKey>> = {
  'Unknown collection': 'sidebar.collections.error.unknownCollection',
  'Path outside of the collection': 'sidebar.collections.error.outsideCollection',
  'Invalid name': 'sidebar.collections.error.invalidName',
  'Only markdown files can be written': 'sidebar.collections.error.markdownOnly',
  'Files can only be created in a folder collection': 'sidebar.collections.error.createInFolderOnly',
  'Rename the file from its folder collection': 'sidebar.collections.error.renameFromFolder',
  'Cannot move a folder into itself': 'sidebar.collections.error.moveIntoItself',
  'A file or folder with this name already exists': 'sidebar.collections.error.nameTaken',
};

@Component({
  selector: 'app-collections-section',
  standalone: true,
  imports: [ContextMenuComponent, ConfirmDialogComponent, FocusSelectDirective],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="header">
      <button type="button" class="section-toggle" [attr.aria-expanded]="open()" (click)="toggleSection()">
        <svg class="chevron" [class.open]="open()" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 6 15 12 9 18"/></svg>
        {{ i18n.t('sidebar.collections.title') }}
      </button>
      <button type="button" class="icon-btn" [title]="i18n.t('sidebar.collections.openFolder')" (click)="addFolder()">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2zM12 11v6M9 14h6"/></svg>
      </button>
      <button type="button" class="icon-btn" [title]="i18n.t('sidebar.collections.openFile')" (click)="collections.add('file')">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6M12 18v-6M9 15h6"/></svg>
      </button>
    </div>

    @if (open()) {
      <div class="rows" role="tree" [attr.aria-label]="i18n.t('sidebar.collections.title')">
        @for (row of rows(); track row.key) {
          @if (row.kind === 'collection') {
            <div
              class="row collection"
              role="treeitem"
              tabindex="0"
              [attr.aria-selected]="false"
              [attr.aria-expanded]="row.state.collection.kind === 'folder' ? isCollectionOpen(row.state) : null"
              [class.error]="row.state.status === 'error'"
              [class.selected]="collections.selectedRoot() === row.state.collection.path && tabs.activeNoteId() === null"
              [class.loading]="row.state.status === 'loading'"
              [class.drop-target]="dropKey() === row.key"
              [title]="rootTitle(row.state)"
              (click)="onCollectionClick(row.state)"
              (keydown.enter)="onCollectionClick(row.state)"
              (keydown.f2)="startEditing({ kind: 'renameCollection', root: row.state.collection.path })"
              (contextmenu)="onCollectionMenu($event, row.state)"
              (dragover)="onDragOver($event, row.state.collection.path, '', row.key)"
              (drop)="onDrop($event, row.state.collection.path, '')"
            >
              @if (row.state.collection.kind === 'folder') {
                <svg class="chevron" [class.open]="isCollectionOpen(row.state)" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 6 15 12 9 18"/></svg>
              } @else {
                <span class="chevron-space"></span>
              }
              <svg class="row-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                @if (row.state.collection.kind === 'folder') {
                  <path d="M2 6a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"/><path d="M2 10h20"/>
                } @else {
                  <path [attr.d]="iconFile"/>
                }
              </svg>
              @if (isEditing('renameCollection', row.state.collection.path)) {
                <input class="inline-input" appFocusSelect maxlength="100" [value]="row.state.collection.displayName"
                  (click)="$event.stopPropagation()" (keydown.enter)="commitEditing($event)" (keydown.escape)="editing.set(null); $event.stopPropagation()" (blur)="commitEditing($event)" />
              } @else {
                <span class="row-label">{{ row.state.label }}</span>
                @if (row.state.hint) {
                  <span class="hint">{{ row.state.hint }}</span>
                }
                @switch (row.state.status) {
                  @case ('error') {
                    <span class="status" [title]="i18n.t('sidebar.collections.notFound')">!</span>
                  }
                  @case ('loading') {
                    <span class="spinner" role="status" [attr.aria-label]="i18n.t('common.loading')"></span>
                  }
                  @case ('unloaded') {
                    <span class="status unloaded" [title]="i18n.t('sidebar.collections.notLoaded')">N</span>
                  }
                }
              }
            </div>
            @if (isEditing('newFile', row.state.collection.path, '') && isCollectionOpen(row.state)) {
              <div class="row" [style.padding-left.px]="22">
                <span class="chevron-space"></span>
                <svg class="row-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path [attr.d]="iconFile"/></svg>
                <input class="inline-input" appFocusSelect [placeholder]="i18n.t('sidebar.collections.fileName')" (keydown.enter)="commitEditing($event)" (keydown.escape)="editing.set(null)" (blur)="commitEditing($event)" />
              </div>
            }
          } @else {
            <div
              class="row"
              role="treeitem"
              tabindex="0"
              [attr.data-key]="row.key"
              [attr.aria-selected]="isActive(row.root, row.node)"
              [attr.aria-expanded]="row.node.kind === 'folder' ? isExpanded(row.key) : null"
              [draggable]="!isEditing('renamePath', row.root, row.node.path)"
              [class.selected]="isActive(row.root, row.node)"
              [class.drop-target]="dropKey() === row.key"
              [style.padding-left.px]="8 + row.depth * 14"
              [title]="absolutePath(row.root, row.node.path)"
              (click)="onNodeClick(row.root, row.node)"
              (keydown.enter)="onNodeClick(row.root, row.node)"
              (keydown.f2)="startEditing({ kind: 'renamePath', root: row.root, path: row.node.path })"
              (keydown.delete)="confirmTrash.set({ root: row.root, path: row.node.path })"
              (keydown.meta.backspace)="isMacDeleteKey($event) && confirmTrash.set({ root: row.root, path: row.node.path })"
              (contextmenu)="onNodeMenu($event, row.root, row.node)"
              (dragstart)="onDragStart($event, row.root, row.node.path)"
              (dragend)="dragged = null; dropKey.set(null)"
              (dragover)="onDragOver($event, row.root, folderOf(row.node), row.key)"
              (drop)="onDrop($event, row.root, folderOf(row.node))"
            >
              @if (row.node.kind === 'folder') {
                <svg class="chevron" [class.open]="isExpanded(row.key)" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 6 15 12 9 18"/></svg>
              } @else {
                <span class="chevron-space"></span>
              }
              <svg class="row-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                @if (row.node.kind === 'folder') {
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                } @else {
                  <path [attr.d]="iconFile"/>
                }
              </svg>
              @if (isEditing('renamePath', row.root, row.node.path)) {
                <input class="inline-input" appFocusSelect maxlength="200" [value]="row.node.name"
                  (click)="$event.stopPropagation()" (keydown.enter)="commitEditing($event)" (keydown.escape)="editing.set(null); $event.stopPropagation()" (blur)="commitEditing($event)" />
              } @else {
                <span class="row-label">{{ row.node.name }}</span>
              }
            </div>
            @if (row.node.kind === 'folder' && isEditing('newFile', row.root, row.node.path) && isExpanded(row.key)) {
              <div class="row" [style.padding-left.px]="22 + row.depth * 14">
                <span class="chevron-space"></span>
                <svg class="row-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path [attr.d]="iconFile"/></svg>
                <input class="inline-input" appFocusSelect [placeholder]="i18n.t('sidebar.collections.fileName')" (keydown.enter)="commitEditing($event)" (keydown.escape)="editing.set(null)" (blur)="commitEditing($event)" />
              </div>
            }
          }
        } @empty {
          <div class="empty">{{ i18n.t('sidebar.collections.empty') }}</div>
        }
      </div>
    }

    @if (menu(); as current) {
      <app-context-menu [items]="current.items" [x]="current.x" [y]="current.y" (action)="onMenuAction($event)" (closed)="menu.set(null)" />
    }

    @if (confirmTrash(); as target) {
      <app-confirm-dialog
        [title]="i18n.t(platformKey('sidebar.collections.trash.title'), { name: baseName(target.path) })"
        [message]="i18n.t(platformKey('sidebar.collections.trash.message'))"
        [actions]="trashActions()"
        (chosen)="onTrashChoice($event)"
      />
    }

    @if (failure(); as message) {
      <div class="failure" role="alert" (click)="failure.set(null)">{{ message }}</div>
    }
  `,
  styleUrls: ['./collections-section.component.scss'],
  host: { '[class.open]': 'open()' },
})
export class CollectionsSectionComponent {
  protected readonly i18n = inject(I18nService);
  protected readonly collections = inject(CollectionsService);
  private readonly fileDocs = inject(FileDocsService);
  protected readonly tabs = inject(TabsService);
  private readonly sidebarState = inject(SidebarStateService);
  private readonly platform = inject(PlatformService);

  /** Delete key label of the context menus: `⌘⌫` on macOS (Finder), `Del` elsewhere. */
  protected readonly deleteShortcut = this.platform.isMac ? formatAccelerator('CommandOrControl+Backspace', this.platform.platform) : 'Del';

  /**
   * ⌘⌫ deletes like Delete on macOS (MacBooks have no forward Delete key). Only on the row itself: in a rename
   * field ⌘⌫ keeps erasing the text. Always false elsewhere.
   */
  protected isMacDeleteKey(event: Event): boolean {
    return this.platform.isMac && event.target === event.currentTarget;
  }

  protected readonly iconFile = ICON_FILE;
  protected readonly trashActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'trash', label: this.i18n.t(this.platformKey('sidebar.collections.trash.confirm')), danger: true },
  ]);
  protected readonly baseName = baseName;

  /** Picks the macOS wording (Trash, Finder) over the Windows one. */
  protected platformKey<K extends string>(key: K): K | `${K}.mac` {
    return this.platform.isMac ? `${key}.mac` as `${K}.mac` : key;
  }

  protected readonly open = signal(!readSet(SECTION_KEY).has('closed'));
  private readonly expanded = signal<ReadonlySet<string>>(readSet(EXPANDED_KEY));
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly injector = inject(Injector);
  protected readonly editing = signal<CollectionEditing | null>(null);
  protected readonly menu = signal<{ items: ContextMenuEntry[]; x: number; y: number; target: { root: string; node: CollectionNode | null } } | null>(null);
  protected readonly dropKey = signal<string | null>(null);
  protected readonly failure = signal<string | null>(null);
  protected readonly confirmTrash = signal<CollectionFileRef | null>(null);
  protected dragged: CollectionFileRef | null = null;

  private readonly activeFile = computed(() => this.fileDocs.refFor(this.tabs.activeNoteId()));

  protected readonly rows = computed<CollectionRow[]>(() => {
    const expanded = this.expanded();
    const rows: CollectionRow[] = [];

    const visit = (root: string, nodes: readonly CollectionNode[], depth: number): void => {
      for (const node of nodes) {
        const key = nodeKey(root, node.path);
        rows.push({ kind: 'node', key, root, node, depth });

        if (node.kind === 'folder' && expanded.has(key)) {
          visit(root, node.children ?? [], depth + 1);
        }
      }
    };

    for (const state of this.collections.states()) {
      const key = nodeKey(state.collection.path, '');
      rows.push({ kind: 'collection', key, state, depth: 0 });

      // An unloaded collection has no tree yet: expanded in a previous session, it waits for a click
      if (state.collection.kind === 'folder' && state.status !== 'unloaded' && expanded.has(key)) {
        visit(state.collection.path, state.tree, 1);
      }
    }

    return rows;
  });

  constructor() {
    // A link to a folder without an index page shows that folder here
    effect(() => {
      const request = this.collections.folderToReveal();
      if (request) {
        untracked(() => this.revealFolder(request.root, request.path));
      }
    });
  }

  /** Opens the section and the collection, expands the folder and its parents, then scrolls to it and focuses it. */
  private revealFolder(root: string, path: string): void {
    const state = this.collections.states().find(candidate => candidate.collection.path === root);
    if (!state) {
      return;
    }
    if (!this.open()) {
      this.toggleSection();
    }
    this.openCollection(state);

    const segments = path.split('/').filter(Boolean);
    segments.forEach((_, index) => this.expand(nodeKey(root, segments.slice(0, index + 1).join('/'))));

    afterNextRender(() => {
      const row = this.host.nativeElement.querySelector<HTMLElement>(`[data-key="${CSS.escape(nodeKey(root, path))}"]`);
      row?.scrollIntoView({ block: 'nearest' });
      row?.focus();
    }, { injector: this.injector });
  }

  protected toggleSection(): void {
    this.open.update(open => !open);
    this.save(SECTION_KEY, this.open() ? [] : ['closed']);
  }

  protected isExpanded(key: string): boolean {
    return this.expanded().has(key);
  }

  protected isCollectionOpen(state: CollectionState): boolean {
    return state.status !== 'unloaded' && this.isExpanded(nodeKey(state.collection.path, ''));
  }

  /** Expands a folder collection, loading it the first time. */
  private openCollection(state: CollectionState): void {
    this.expand(nodeKey(state.collection.path, ''));

    if (state.status === 'unloaded') {
      void this.collections.load(state.collection.path);
    }
  }

  protected async addFolder(): Promise<void> {
    const added = await this.collections.add('folder');

    for (const collection of added) {
      const state = this.collections.states().find(candidate => candidate.collection.path === collection.path);

      if (state) {
        this.openCollection(state);
      }
    }
  }

  private toggle(key: string): void {
    const next = new Set(this.expanded());

    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }

    this.expanded.set(next);
    this.save(EXPANDED_KEY, [...next]);
  }

  private expand(key: string): void {
    if (!this.expanded().has(key)) {
      this.toggle(key);
    }
  }

  private save(key: string, values: string[]): void {
    try {
      localStorage.setItem(key, JSON.stringify(values));
    } catch {
      // Storage unavailable: the tree simply starts collapsed next time
    }
  }

  protected rootTitle(state: CollectionState): string {
    return state.error ? `${state.collection.path}\n${state.error}` : state.collection.path;
  }

  protected absolutePath(root: string, relative: string): string {
    return `${root}\\${relative.split('/').join('\\')}`;
  }

  protected isActive(root: string, node: CollectionNode): boolean {
    const active = this.activeFile();
    return node.kind === 'file' && active?.root === root && active.path === node.path;
  }

  protected folderOf(node: CollectionNode): string {
    return this.collections.folderOf(node);
  }

  protected onCollectionClick(state: CollectionState): void {
    if (state.collection.kind === 'file') {
      this.collections.open({ root: state.collection.path, path: baseName(state.collection.path) });
      return;
    } else if (state.status === 'unloaded') {
      this.openCollection(state);
    } else {
      this.toggle(nodeKey(state.collection.path, ''));
    }

    // Like a folder of the notes: browsed on the home page (its graph), and expanded / collapsed
    this.sidebarState.selectFolder(null);
    this.collections.select(state.collection.path);
    this.tabs.goHome();
  }

  protected onNodeClick(root: string, node: CollectionNode): void {
    if (node.kind === 'folder') {
      this.toggle(nodeKey(root, node.path));
    } else {
      this.collections.open({ root, path: node.path });
    }
  }

  // ── Inline inputs ──────────────────────────────────────────────────────────

  protected isEditing(kind: CollectionEditing['kind'], root: string, path?: string): boolean {
    const editing = this.editing();

    if (!editing || editing.kind !== kind || editing.root !== root) {
      return false;
    }

    if (editing.kind === 'renamePath') {
      return editing.path === path;
    }
    return editing.kind === 'newFile' ? editing.folder === path : true;
  }

  protected startEditing(editing: CollectionEditing): void {
    this.editing.set(editing);
  }

  protected async commitEditing(event: Event): Promise<void> {
    const editing = this.editing();

    // Enter then the blur it causes both land here: only the first one counts
    if (!editing) {
      return;
    }

    this.editing.set(null);
    const value = (event.target as HTMLInputElement).value.trim();

    if (!value) {
      return;
    }

    await this.report(async () => {
      if (editing.kind === 'renameCollection') {
        await this.collections.rename(editing.root, value);
      } else if (editing.kind === 'renamePath') {
        if (value !== baseName(editing.path)) {
          await this.collections.renamePath({ root: editing.root, path: editing.path }, value);
        }
      } else {
        await this.collections.createFile(editing.root, editing.folder, value);
      }
    });
  }

  /** Disk operations may fail (name taken, file locked…): the message shows under the section. */
  private async report(operation: () => Promise<void>): Promise<void> {
    try {
      await operation();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      // Electron prefixes IPC errors with the channel: only the reason matters
      const reason = message.replace(/^Error invoking remote method '[^']+': (Error: )?/, '');
      // The reasons the main process raises itself are translated; system errors (disk, permissions) show as they are
      const key = COLLECTION_ERROR_KEYS[reason];
      this.failure.set(key ? this.i18n.t(key) : reason);
    }
  }

  // ── Context menus ──────────────────────────────────────────────────────────

  protected onCollectionMenu(event: MouseEvent, state: CollectionState): void {
    event.preventDefault();
    const t = this.i18n.t;
    const isFolder = state.collection.kind === 'folder';
    const items: ContextMenuEntry[] = [];

    if (state.status === 'error') {
      items.push({ id: 'relocate', label: t('sidebar.collections.menu.locate'), icon: ICON_REVEAL }, { divider: true });
    } else if (isFolder) {
      items.push({ id: 'newFile', label: t('sidebar.collections.menu.newFile'), icon: ICON_FILE }, { divider: true });
    }

    items.push(
      { id: 'renameCollection', label: t('sidebar.collections.menu.renameCollection'), icon: ICON_RENAME, shortcut: 'F2' },
      { id: 'reveal', label: t(this.platformKey('sidebar.collections.menu.reveal')), icon: ICON_REVEAL, disabled: state.status === 'error' },
      { id: 'refresh', label: t('sidebar.collections.menu.refresh'), icon: ICON_REFRESH, disabled: !isFolder },
      { divider: true },
      { id: 'remove', label: t('sidebar.collections.menu.remove'), icon: ICON_REMOVE },
    );

    this.menu.set({ items, x: event.clientX, y: event.clientY, target: { root: state.collection.path, node: null } });
  }

  protected onNodeMenu(event: MouseEvent, root: string, node: CollectionNode): void {
    event.preventDefault();
    event.stopPropagation();
    const t = this.i18n.t;
    const items: ContextMenuEntry[] = [];

    if (node.kind === 'folder') {
      items.push({ id: 'newFile', label: t('sidebar.collections.menu.newFile'), icon: ICON_FILE }, { divider: true });
    }

    items.push(
      { id: 'renamePath', label: t('common.rename'), icon: ICON_RENAME, shortcut: 'F2' },
      { id: 'reveal', label: t(this.platformKey('sidebar.collections.menu.reveal')), icon: ICON_REVEAL },
      { divider: true },
      { id: 'trash', label: t('common.delete'), icon: ICON_DELETE, danger: true, shortcut: this.deleteShortcut },
    );

    this.menu.set({ items, x: event.clientX, y: event.clientY, target: { root, node } });
  }

  protected async onMenuAction(actionId: string): Promise<void> {
    const current = this.menu();
    this.menu.set(null);

    if (!current) {
      return;
    }

    const { root, node } = current.target;
    const ref: CollectionFileRef = { root, path: node?.path ?? '' };

    switch (actionId) {
      case 'newFile': {
        const folder = node?.path ?? '';
        const state = this.collections.states().find(candidate => candidate.collection.path === root);

        if (!node && state) {
          this.openCollection(state);
        } else {
          this.expand(nodeKey(root, folder));
        }

        this.startEditing({ kind: 'newFile', root, folder });
        break;
      }
      case 'renameCollection':
        this.startEditing({ kind: 'renameCollection', root });
        break;
      case 'renamePath':
        this.startEditing({ kind: 'renamePath', root, path: ref.path });
        break;
      case 'reveal':
        this.collections.reveal(ref);
        break;
      case 'refresh':
        await this.collections.load(root);
        break;
      case 'relocate':
        await this.report(() => this.collections.relocate(root));
        break;
      case 'remove':
        await this.collections.remove(root);
        break;
      case 'trash':
        this.confirmTrash.set(ref);
        break;
    }
  }

  protected async onTrashChoice(choice: string | null): Promise<void> {
    const target = this.confirmTrash();
    this.confirmTrash.set(null);

    if (choice === 'trash' && target) {
      await this.report(() => this.collections.trash(target));
    }
  }

  // ── Drag & drop (inside one collection) ───────────────────────────────────

  protected onDragStart(event: DragEvent, root: string, path: string): void {
    this.dragged = { root, path };

    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/plain', path);
    }
  }

  /** A file / folder can move into another folder of the same collection, never into itself. */
  private canDrop(root: string, folder: string): boolean {
    const dragged = this.dragged;

    if (!dragged || dragged.root !== root) {
      return false;
    }

    const insideItself = folder === dragged.path || folder.startsWith(`${dragged.path}/`);
    return !insideItself && parentPath(dragged.path) !== folder;
  }

  protected onDragOver(event: DragEvent, root: string, folder: string, key: string): void {
    if (!this.canDrop(root, folder)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    this.dropKey.set(key);
  }

  protected async onDrop(event: DragEvent, root: string, folder: string): Promise<void> {
    const dragged = this.dragged;
    this.dropKey.set(null);
    this.dragged = null;

    if (!dragged || !this.canDrop(root, folder)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    this.expand(nodeKey(root, folder));
    await this.report(() => this.collections.movePath(dragged, folder));
  }
}
