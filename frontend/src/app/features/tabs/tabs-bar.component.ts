import {
  Component, inject, ElementRef, ViewChild,
  ChangeDetectionStrategy, HostListener, signal, DestroyRef, computed
} from '@angular/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { TabsService } from '../../core/services/tabs.service';
import { DesktopBridgeService } from '../../core/services/desktop-bridge.service';
import { LayoutService } from '../../core/services/layout.service';
import { SettingsService } from '../../core/services/settings.service';
import { PlatformService } from '../../core/services/platform.service';
import { I18nService } from '../../core/i18n/i18n.service';
import { noteDisplayTitle } from '../../core/utils/note-title.util';
import { SidebarStateService } from '../../core/services/sidebar-state.service';
import { QUICK_NOTES_TAB_ID, Tab } from '../../core/models/note.model';
import { ContextMenuComponent, ContextMenuEntry } from '../../shared/context-menu/context-menu.component';
import { NoteExportService, noteExportMenu } from '../../core/services/note-export.service';

@Component({
  selector: 'app-tabs-bar',
  standalone: true,
  imports: [ContextMenuComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <!-- Title bar: drags the window, double-click maximizes (Tauri drag region; tabs and buttons excluded) -->
    <div class="tabs-bar" data-tauri-drag-region="deep">
      <!-- macOS: the native traffic lights float over the top left corner; this empty zone keeps the tabs clear of them but still drags the window -->
      @if (trafficLightsInset()) {
        <div class="traffic-lights-inset" data-tauri-drag-region></div>
      }

      <!-- Sidebar toggle -->
      <button
        class="home-btn"
        [class.active]="sidebarVisible()"
        (click)="toggleSidebar()"
        (dblclick)="$event.stopPropagation()"
        [title]="i18n.t('workspace.tabs.toggleSidebar', { shortcut: shortcutLabel('sidebar.toggle') })"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/>
        </svg>
      </button>

      <!-- Home button -->
      <button
        class="home-btn"
        [class.active]="activeIndex() === -1"
        (click)="goHome()"
        (dblclick)="$event.stopPropagation()"
        [title]="i18n.t('workspace.tabs.home')"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
          <polyline points="9 22 9 12 15 12 15 22"/>
        </svg>
      </button>

      <!-- Window drag region -->
      <div class="drag-region"></div>

      <!-- Tabs -->
      <div class="tabs-container" #tabsContainer>
        @for (tab of tabs(); track tab.noteId; let i = $index) {
          <div
            class="tab"
            data-tauri-drag-region="false"
            [class.active]="i === activeIndex()"
            [class.dirty]="tab.isDirty"
            [class.pinned]="tab.isPinned"
            [class.drag-over-left]="dragOverIndex() === i && dragSide() === 'left'"
            [class.drag-over-right]="dragOverIndex() === i && dragSide() === 'right'"
            [class.dragging]="dragIndex() === i"
            (click)="selectTab(i)"
            (dblclick)="$event.stopPropagation()"
            (mousedown)="onTabMouseDown($event)"
            (auxclick)="onAuxClick($event, i)"
            (contextmenu)="onTabContextMenu($event, i)"
            [title]="tabTitle(tab)"
            draggable="true"
            (dragstart)="onDragStart($event, i)"
            (dragover)="onDragOver($event, i)"
            (dragleave)="onDragLeave()"
            (drop)="onDrop($event, i)"
            (dragend)="onDragEnd()"
          >
            @if (tab.isPinned) {
              <svg class="pin-icon" width="11" height="11" viewBox="0 0 24 24" fill="currentColor" stroke="none">
                <path d="M16 2L7.5 10.5 2 12l3 3-5 7 7-5 3 3 1.5-5.5L20 6l-2-2z"/>
              </svg>
            }
            <span class="tab-title">{{ tabTitle(tab) }}</span>
            @if (tab.isDirty) {
              <span class="dirty-dot" [title]="i18n.t('workspace.tabs.unsaved')"></span>
            }
            @if (!tab.isPinned) {
              <button
                class="tab-close"
                (click)="closeTab($event, i)"
                tabindex="-1"
                [title]="i18n.t('workspace.tabs.closeWithShortcut', { shortcut: shortcutLabel('tab.close') })"
              >×</button>
            }
          </div>
        }
      </div>

      <!-- Wide mode toggle -->
      <button
        class="titlebar-icon-btn"
        (click)="toggleWideMode()"
        (dblclick)="$event.stopPropagation()"
        [title]="i18n.t(wideMode() ? 'workspace.tabs.layoutCentered' : 'workspace.tabs.layoutFullWidth', { shortcut: shortcutLabel('layout.toggle') })"
      >
        @if (wideMode()) {
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="4 14 10 14 10 20"/>
            <polyline points="20 10 14 10 14 4"/>
            <line x1="14" y1="10" x2="21" y2="3"/>
            <line x1="3" y1="21" x2="10" y2="14"/>
          </svg>
        } @else {
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="15 3 21 3 21 9"/>
            <polyline points="9 21 3 21 3 15"/>
            <line x1="21" y1="3" x2="14" y2="10"/>
            <line x1="3" y1="21" x2="10" y2="14"/>
          </svg>
        }
      </button>

      <!-- Window controls (macOS has the native traffic lights instead) -->
      @if (!platform.isMac) {
      <div class="window-controls">
        <button class="wc-btn minimize" (click)="minimize()" [title]="i18n.t('workspace.tabs.minimize')">
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor"/></svg>
        </button>
        <button class="wc-btn maximize" (click)="toggleMaximize()" [title]="i18n.t('workspace.tabs.maximize')">
          <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor"/></svg>
        </button>
        <button class="wc-btn close" (click)="closeWindow()" [title]="i18n.t('common.close')">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1.2"/>
            <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1.2"/>
          </svg>
        </button>
      </div>
      }
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
  styleUrls: ['./tabs-bar.component.scss'],
})
export class TabsBarComponent {
  protected readonly i18n = inject(I18nService);
  protected readonly platform = inject(PlatformService);
  private tabs$ = inject(TabsService);
  private desktop = inject(DesktopBridgeService);
  private layout = inject(LayoutService);
  private settings = inject(SettingsService);
  private sidebarState = inject(SidebarStateService);
  private destroyRef = inject(DestroyRef);

  @ViewChild('tabsContainer') tabsContainer!: ElementRef<HTMLElement>;

  /** True while the macOS window is fullscreen: the traffic lights are hidden, so no room is kept for them. */
  private readonly fullscreen = signal(false);
  readonly trafficLightsInset = computed(() => this.platform.isMac && !this.fullscreen());

  constructor() {
    if (this.platform.isMac && this.desktop.inDesktop) {
      void this.trackFullscreen();
    }
  }

  private async trackFullscreen(): Promise<void> {
    const win = getCurrentWindow();
    const refresh = async () => this.fullscreen.set(await win.isFullscreen());
    try {
      await refresh();
      const unlisten = await win.onResized(() => void refresh());
      this.destroyRef.onDestroy(unlisten);
    } catch {
      // Window API unavailable: keep the fixed inset
    }
  }

  readonly wideMode = this.layout.wideMode;
  readonly sidebarVisible = this.sidebarState.visible;

  readonly tabs = this.tabs$.tabs;
  readonly activeIndex = this.tabs$.activeIndex;

  // ── Drag state ─────────────────────────────────────────────────────────────
  readonly dragIndex = signal<number | null>(null);
  readonly dragOverIndex = signal<number | null>(null);
  readonly dragSide = signal<'left' | 'right' | null>(null);

  // ── Context menu state ─────────────────────────────────────────────────────
  private readonly noteExport = inject(NoteExportService);
  readonly ctxMenu = signal<{ items: ContextMenuEntry[]; x: number; y: number; tabIndex: number } | null>(null);

  // ── Navigation ─────────────────────────────────────────────────────────────

  goHome(): void { this.tabs$.goHome(); }
  selectTab(index: number): void { this.tabs$.setActiveIndex(index); }
  toggleWideMode(): void { this.layout.toggle(); }

  toggleSidebar(): void { this.sidebarState.toggleVisible(); }

  protected shortcutLabel(id: string): string { return this.settings.shortcutLabel(id); }

  /** The quick notes tab is stored with a fixed title: shown in the current language, like the untitled notes. */
  protected tabTitle(tab: Tab): string {
    return tab.noteId === QUICK_NOTES_TAB_ID ? this.i18n.t('common.quickNotes') : noteDisplayTitle(tab.title, this.i18n.t);
  }

  closeTab(event: MouseEvent, index: number): void {
    event.stopPropagation();
    this.tabs$.closeTab(index);
  }

  onTabMouseDown(event: MouseEvent): void {
    if (event.button === 1) event.preventDefault();
  }

  onAuxClick(event: MouseEvent, index: number): void {
    if (event.button === 1) this.tabs$.closeTab(index);
  }

  /** The wheel (vertical) and a touchpad swipe (horizontal) both scroll the tabs sideways. */
  @HostListener('wheel', ['$event'])
  onWheel(event: WheelEvent): void {
    const el = this.tabsContainer?.nativeElement;
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (!el || delta === 0) {
      return;
    }

    const before = el.scrollLeft;
    el.scrollLeft += delta;
    // Only a scroll that moved the tabs is kept from the page
    if (el.scrollLeft !== before) {
      event.preventDefault();
    }
  }

  minimize(): void { this.desktop.minimize(); }
  async toggleMaximize(): Promise<void> { await this.desktop.maximize(); }
  closeWindow(): void { this.desktop.close(); }
  // ── Drag and drop ──────────────────────────────────────────────────────────

  onDragStart(event: DragEvent, index: number): void {
    this.dragIndex.set(index);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/plain', String(index));
    }
  }

  onDragOver(event: DragEvent, index: number): void {
    if (this.dragIndex() === null) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';

    // Determine which side of the tab we're hovering
    const target = (event.currentTarget as HTMLElement);
    const rect = target.getBoundingClientRect();
    const midX = rect.left + rect.width / 2;
    this.dragSide.set(event.clientX < midX ? 'left' : 'right');
    this.dragOverIndex.set(index);
  }

  onDragLeave(): void {
    this.dragOverIndex.set(null);
    this.dragSide.set(null);
  }

  onDrop(event: DragEvent, toIndex: number): void {
    event.preventDefault();
    const fromIndex = this.dragIndex();
    if (fromIndex === null || fromIndex === toIndex) {
      this.resetDragState();
      return;
    }

    // If dropping on the right side, we want to insert after
    const side = this.dragSide();
    let targetIndex = toIndex;
    if (side === 'right' && fromIndex < toIndex) targetIndex = toIndex;
    else if (side === 'right' && fromIndex > toIndex) targetIndex = toIndex + 1;
    else if (side === 'left' && fromIndex > toIndex) targetIndex = toIndex;
    else if (side === 'left' && fromIndex < toIndex) targetIndex = toIndex - 1;

    this.tabs$.moveTab(fromIndex, Math.max(0, Math.min(targetIndex, this.tabs().length - 1)));
    this.resetDragState();
  }

  onDragEnd(): void {
    this.resetDragState();
  }

  private resetDragState(): void {
    this.dragIndex.set(null);
    this.dragOverIndex.set(null);
    this.dragSide.set(null);
  }

  // ── Context menu ───────────────────────────────────────────────────────────

  onTabContextMenu(event: MouseEvent, index: number): void {
    event.preventDefault();
    event.stopPropagation();

    const tab = this.tabs()[index];
    const pinLabel = this.i18n.t(tab.isPinned ? 'workspace.tabs.unpin' : 'workspace.tabs.pin');
    // SVG paths (24x24 viewBox)
    const pinIcon = 'M16 2L7.5 10.5 2 12l3 3-5 7 7-5 3 3 1.5-5.5L20 6l-2-2z';
    const closeIcon = 'M18 6L6 18M6 6l12 12';

    // Pinned tabs are never closed in bulk: grey the entries out when they would do nothing
    const allTabs = this.tabs();
    const hasClosableTab = allTabs.some(t => !t.isPinned);
    const hasClosableOthers = allTabs.some((t, i) => i !== index && !t.isPinned);

    const items: ContextMenuEntry[] = [
      { id: 'pin', label: pinLabel, icon: pinIcon, shortcut: this.shortcutLabel('tab.pin') },
      { divider: true },
      { id: 'close', label: this.i18n.t('workspace.tabs.closeTab'), icon: closeIcon, shortcut: this.shortcutLabel('tab.close'), disabled: tab.isPinned },
      { id: 'closeOthers', label: this.i18n.t('workspace.tabs.closeOthers'), icon: closeIcon, disabled: !hasClosableOthers },
      { id: 'closeAll', label: this.i18n.t('workspace.tabs.closeAll'), icon: closeIcon, disabled: !hasClosableTab },
    ];

    if (this.noteExport.canExport(tab.noteId)) {
      items.push({ divider: true }, noteExportMenu(this.i18n.t));
    }

    this.ctxMenu.set({ items, x: event.clientX, y: event.clientY, tabIndex: index });
  }

  onCtxAction(actionId: string): void {
    const menu = this.ctxMenu();
    if (!menu) return;
    const index = menu.tabIndex;
    const noteId = this.tabs()[index]?.noteId;

    if (noteId !== undefined && actionId.startsWith('export:')) {
      void this.noteExport.runNoteAction(actionId, noteId);
    }

    switch (actionId) {
      case 'pin':
        this.tabs$.togglePin(index);
        break;
      case 'close':
        this.tabs$.closeTab(index);
        break;
      case 'closeOthers':
        this.tabs$.closeOthers(index);
        break;
      case 'closeAll':
        this.tabs$.closeAll();
        break;
    }
    this.ctxMenu.set(null);
  }
}
