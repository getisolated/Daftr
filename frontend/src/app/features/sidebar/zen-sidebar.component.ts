import { ChangeDetectionStrategy, Component, ElementRef, OnDestroy, computed, inject } from '@angular/core';

import { LayoutService } from '../../core/services/layout.service';
import { SidebarStateService } from '../../core/services/sidebar-state.service';
import { zenSidebarPointerAction } from '../../core/utils/zen-sidebar.util';
import { SidebarComponent } from './sidebar.component';

/** Gap between the slid-in sidebar and the left edge of the window. */
const GAP_PX = 8;
/** Part of the tucked-away sidebar left inside the window, so that it can be guessed. */
const PEEK_PX = 13;
/** Its border, around the width of the sidebar itself. */
const BORDER_PX = 1;
/** Grace period before the sidebar slides away: crossing its edge by accident does not close it. */
const HIDE_DELAY_MS = 300;

/** The sidebar of the zen mode: floating, tucked away on the left edge, slid in by the pointer reaching that edge. */
@Component({
  selector: 'app-zen-sidebar',
  standalone: true,
  imports: [SidebarComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '[class.open]': 'open()',
    '[style.left.px]': 'left()',
  },
  template: `<app-sidebar />`,
  styles: [`
    /* No transform to slide it: menus and dialogs of the sidebar are position: fixed, a transform would pin them to it */
    :host {
      position: absolute;
      top: 12px;
      bottom: 12px;
      z-index: 100;
      display: flex;
      border: ${BORDER_PX}px solid var(--daftr-border-soft);
      border-radius: 10px;
      overflow: hidden;
      background: var(--daftr-bg-panel);
      box-shadow: 0 8px 28px rgba(0, 0, 0, 0.18), 0 2px 6px rgba(0, 0, 0, 0.12);
      transition: left 0.22s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.22s ease;
    }
    :host(:not(.open)) { box-shadow: none; }
    app-sidebar {
      border-right: none;
      transition: opacity 0.15s ease;
    }
    /* Only the tucked-away panel peeks: its content (and its scrollbar) would show through the visible strip */
    :host(:not(.open)) app-sidebar {
      opacity: 0;
      visibility: hidden;
      transition: opacity 0.15s ease, visibility 0s linear 0.15s;
    }
    @media (prefers-reduced-motion: reduce) {
      :host, app-sidebar, :host(:not(.open)) app-sidebar { transition: none; }
    }
  `],
})
export class ZenSidebarComponent implements OnDestroy {
  private readonly layout = inject(LayoutService);
  private readonly state = inject(SidebarStateService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private hideTimer: ReturnType<typeof setTimeout> | null = null;
  // Plain listeners rather than host bindings: every pointer move of the window would otherwise run a change detection
  private readonly pointerMove = (event: MouseEvent): void => this.onPointerMove(event);
  private readonly pointerDown = (event: MouseEvent): void => this.onPointerDown(event);

  protected readonly open = computed(() => this.layout.zenSidebar() !== null);
  protected readonly left = computed(() => (this.open() ? GAP_PX : PEEK_PX - this.state.width() - 2 * BORDER_PX));

  constructor() {
    document.addEventListener('mousemove', this.pointerMove, { passive: true });
    document.addEventListener('mousedown', this.pointerDown, true);
  }

  ngOnDestroy(): void {
    document.removeEventListener('mousemove', this.pointerMove);
    document.removeEventListener('mousedown', this.pointerDown, true);
    this.cancelHide();
  }

  private onPointerMove(event: MouseEvent): void {
    const panel = this.host.nativeElement;
    const focused = document.activeElement;
    const action = zenSidebarPointerAction({
      x: event.clientX,
      // Its menus and dialogs are inside it: hovering them keeps it open
      overPanel: event.target instanceof Node && panel.contains(event.target),
      revealed: this.layout.zenSidebar(),
      editing: focused instanceof HTMLElement && panel.contains(focused) && focused.matches('input, textarea, [contenteditable="true"]'),
    });

    if (action === 'open') {
      this.cancelHide();
      this.layout.setZenSidebar('pointer');
    } else if (action === 'close' && this.hideTimer === null) {
      this.hideTimer = setTimeout(() => {
        this.hideTimer = null;
        this.layout.setZenSidebar(null);
      }, HIDE_DELAY_MS);
    } else if (action === 'stay') {
      this.cancelHide();
    }
  }

  /** A click elsewhere sends it away, even when the shortcut brought it in. */
  private onPointerDown(event: MouseEvent): void {
    if (this.open() && event.target instanceof Node && !this.host.nativeElement.contains(event.target)) {
      this.cancelHide();
      this.layout.setZenSidebar(null);
    }
  }

  private cancelHide(): void {
    if (this.hideTimer !== null) {
      clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }
}
