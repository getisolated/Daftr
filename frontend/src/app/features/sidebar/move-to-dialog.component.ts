import { ChangeDetectionStrategy, Component, computed, inject, input, linkedSignal, output, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { FolderOption } from '../../core/models/folder.model';
import { VisibleFolderOption, visibleFolderOptions } from '../../core/utils/folder-tree.util';
import { FocusSelectDirective } from '../../shared/ui/focus-select.directive';

/** Destination picker of the sidebar ("Move to…", "Copy to…"): the folder tree, collapsible and filterable, the root first. */
@Component({
  selector: 'app-move-to-dialog',
  standalone: true,
  imports: [FocusSelectDirective],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'closed.emit()',
  },
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="title()">
      <h2 class="title">{{ title() }}</h2>
      <input
        class="filter"
        appFocusSelect
        [placeholder]="i18n.t('sidebar.moveTo.filter')"
        spellcheck="false"
        [value]="query()"
        (input)="query.set($any($event.target).value)"
        (keydown.enter)="onEnter()"
      />
      <div class="options">
        @for (row of shown(); track row.option.id) {
          <div class="option-row" [style.padding-left.px]="query() ? 0 : row.option.depth * 14">
            @if (row.expandable) {
              <button
                type="button"
                class="twisty"
                [attr.aria-expanded]="!isCollapsed(row.option.id)"
                [attr.aria-label]="row.option.label"
                (click)="toggle(row.option.id)"
              >
                <svg class="chevron" [class.open]="!isCollapsed(row.option.id)" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 6 15 12 9 18"/></svg>
              </button>
            } @else if (!query()) {
              <span class="twisty-space"></span>
            }
            <button
              type="button"
              class="option"
              [class.selected]="isPicked(row.option.id)"
              [disabled]="row.option.disabled"
              (click)="pick(row.option.id)"
              (dblclick)="chosen.emit(row.option.id)"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                @if (row.option.id === null) {
                  <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/>
                } @else {
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                }
              </svg>
              <span class="option-label">{{ row.option.label }}</span>
            </button>
          </div>
        } @empty {
          <div class="empty">{{ i18n.t('sidebar.moveTo.empty') }}</div>
        }
      </div>
      <div class="actions">
        <button type="button" class="btn" (click)="closed.emit()">{{ i18n.t('common.cancel') }}</button>
        @if (confirmLabel(); as label) {
          <button type="button" class="btn primary" [disabled]="picked() === undefined" (click)="confirm()">{{ label }}</button>
        }
      </div>
    </div>
  `,
  styleUrls: ['./move-to-dialog.component.scss'],
})
export class MoveToDialogComponent {
  protected readonly i18n = inject(I18nService);

  readonly title = input.required<string>();
  readonly options = input.required<FolderOption[]>();
  /** The destination: a folder id, `null` for the root. */
  readonly chosen = output<number | null>();
  readonly closed = output<void>();

  /** Label of a confirm button: a click then only selects the folder (a double click still chooses it); `null`: a click chooses. */
  readonly confirmLabel = input<string | null>(null);

  /** true: the tree opens collapsed to its top-level folders, to be browsed; false: fully expanded. */
  readonly startCollapsed = input(false);

  protected readonly query = signal('');

  /** Folder selected, waiting for the confirm button; `undefined` while none is (`null` is the root). */
  protected readonly picked = signal<number | null | undefined>(undefined);

  /** Folders whose sub-folders are hidden; reset with the options. */
  private readonly collapsed = linkedSignal<ReadonlySet<number>>(() => this.startCollapsed()
    ? new Set(this.options().flatMap(option => (option.id === null ? [] : [option.id])))
    : new Set());

  /** A filter lists every matching folder flat; otherwise the tree, folded as the user left it. */
  protected readonly shown = computed<VisibleFolderOption[]>(() => {
    const needle = this.query().trim().toLowerCase();

    if (needle) {
      return this.options().filter(option => option.label.toLowerCase().includes(needle)).map(option => ({ option, expandable: false }));
    }
    return visibleFolderOptions(this.options(), this.collapsed());
  });

  protected isCollapsed(id: number | null): boolean {
    return id !== null && this.collapsed().has(id);
  }

  protected toggle(id: number | null): void {
    if (id === null) {
      return;
    }

    const next = new Set(this.collapsed());

    if (!next.delete(id)) {
      next.add(id);
    }
    this.collapsed.set(next);
  }

  protected isPicked(id: number | null): boolean {
    return this.confirmLabel() !== null && this.picked() === id;
  }

  protected pick(id: number | null): void {
    if (this.confirmLabel() === null) {
      this.chosen.emit(id);
    } else {
      this.picked.set(id);
    }
  }

  protected confirm(): void {
    const id = this.picked();

    if (id !== undefined) {
      this.chosen.emit(id);
    }
  }

  /** Enter confirms the selected folder, or picks the first one the filter shows. */
  protected onEnter(): void {
    if (this.picked() !== undefined) {
      this.confirm();
      return;
    }

    const first = this.shown().find(row => !row.option.disabled);

    if (first) {
      this.pick(first.option.id);
    }
  }
}
