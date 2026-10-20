import { ChangeDetectionStrategy, Component, ElementRef, afterRenderEffect, inject, input } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SidebarStateService } from '../../core/services/sidebar-state.service';
import { clampSectionHeight } from '../../core/utils/folder-tree.util';

/** Room always left to the notes tree above the resized sections. */
const TREE_RESERVED_HEIGHT = 120;

/**
 * Separator line between two sections of the sidebar; dragging it resizes the section right after it (its next
 * sibling), double-clicking gives that section its natural height back. The height is handed to the section as the
 * `--section-height` CSS variable: the section decides when it applies (not while collapsed).
 */
@Component({
  selector: 'app-section-splitter',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: '',
  host: {
    role: 'separator',
    'aria-orientation': 'horizontal',
    '[attr.title]': "i18n.t('ui.splitter.title')",
    '(pointerdown)': 'onStart($event)',
    '(pointermove)': 'onMove($event)',
    '(pointerup)': 'onEnd($event)',
    '(pointercancel)': 'onEnd($event)',
    '(dblclick)': 'state.setSectionHeight(key(), null)',
  },
  styles: `
    :host {
      position: relative;
      flex-shrink: 0;
      height: 1px;
      background: var(--daftr-border);
      cursor: row-resize;
      touch-action: none;
    }

    /* Wider grab area than the visible line */
    :host::before {
      content: '';
      position: absolute;
      inset: -3px 0;
      z-index: 5;
    }

    :host(:hover),
    :host(:active) {
      background: rgba(var(--daftr-accent-strong-rgb), 0.8);
    }
  `,
})
export class SectionSplitterComponent {
  /** Key under which the height of the section is saved. */
  readonly key = input.required<string>();

  protected readonly i18n = inject(I18nService);
  protected readonly state = inject(SidebarStateService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private drag: { y: number; height: number } | null = null;

  constructor() {
    afterRenderEffect(() => {
      const height = this.state.sectionHeights()[this.key()];
      const target = this.target();

      if (!target) {
        return;
      }

      if (height === undefined) {
        target.style.removeProperty('--section-height');
        target.style.removeProperty('--section-max-height');
      } else {
        target.style.setProperty('--section-height', `${height}px`);
        target.style.setProperty('--section-max-height', 'none');
      }
    });
  }

  private target(): HTMLElement | null {
    const sibling = this.host.nativeElement.nextElementSibling;
    return sibling instanceof HTMLElement ? sibling : null;
  }

  protected onStart(event: PointerEvent): void {
    const target = this.target();

    if (!target || event.button !== 0) {
      return;
    }

    this.host.nativeElement.setPointerCapture(event.pointerId);
    this.drag = { y: event.clientY, height: target.offsetHeight };
  }

  protected onMove(event: PointerEvent): void {
    const parent = this.host.nativeElement.parentElement;

    if (!this.drag || !parent) {
      return;
    }

    // The section is below the separator: dragging up makes it taller
    const wanted = this.drag.height + this.drag.y - event.clientY;
    const max = parent.clientHeight - TREE_RESERVED_HEIGHT;
    this.state.setSectionHeight(this.key(), clampSectionHeight(wanted, max), false);
  }

  protected onEnd(event: PointerEvent): void {
    if (!this.drag) {
      return;
    }

    this.drag = null;
    this.host.nativeElement.releasePointerCapture(event.pointerId);
    const height = this.state.sectionHeights()[this.key()];

    if (height !== undefined) {
      this.state.setSectionHeight(this.key(), height);
    }
  }
}
