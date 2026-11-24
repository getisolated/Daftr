import { ChangeDetectionStrategy, Component, computed, inject, input, output, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionConflict } from '../../core/models/collection-sync.model';
import { CollectionsService } from '../../core/services/collections.service';
import { DiffRow, diffLines, hasDifferences } from '../../core/utils/line-diff.util';

/**
 * "Compare" of a conflict: the original and the copy side by side, line by line, the differing lines highlighted. Both
 * files are read from the disk when it opens; nothing is changed here.
 */
@Component({
  selector: 'app-collection-compare-dialog',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'closed.emit()',
  },
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="i18n.t('csync.compare.title')">
      <h2 class="title">{{ i18n.t('csync.compare.title') }}</h2>
      @if (loading()) {
        <p class="hint">{{ i18n.t('csync.compare.loading') }}</p>
      } @else if (failed()) {
        <p class="error" role="alert">{{ i18n.t('csync.compare.failed') }}</p>
      } @else {
        @if (!different()) {
          <p class="message">{{ i18n.t('csync.compare.identical') }}</p>
        }
        <div class="diff" role="table">
          <div class="head left">{{ originalLabel() }}</div>
          <div class="head right">{{ copyLabel() }}</div>
          @for (row of rows(); track $index) {
            <div class="cell left" [attr.data-kind]="row.kind"><span class="num">{{ row.leftLine }}</span><span class="text">{{ row.left }}</span></div>
            <div class="cell right" [attr.data-kind]="row.kind"><span class="num">{{ row.rightLine }}</span><span class="text">{{ row.right }}</span></div>
          }
        </div>
      }
      <div class="actions">
        <button class="btn secondary" type="button" (click)="closed.emit()">{{ i18n.t('common.close') }}</button>
      </div>
    </div>
  `,
  styleUrls: ['../profiles/profile-dialog.scss'],
  styles: [`
    .dialog { width: 900px; max-width: calc(100vw - 32px); }
    .diff {
      display: grid;
      grid-template-columns: 1fr 1fr;
      min-height: 0;
      overflow: auto;
      border: 1px solid var(--daftr-border-soft);
      border-radius: 4px;
      font-family: var(--daftr-font-mono);
      font-size: 12px;
      line-height: 1.5;
    }
    .head { position: sticky; top: 0; padding: 4px 8px; font-family: inherit; font-weight: 600; color: var(--daftr-text-strong); background: var(--daftr-bg-panel); border-bottom: 1px solid var(--daftr-border-soft); }
    .head.left, .cell.left { border-right: 1px solid var(--daftr-border-soft); }
    .cell { display: flex; gap: 8px; min-width: 0; padding: 0 8px; color: var(--daftr-text); }
    .num { flex: none; width: 3ch; text-align: right; color: var(--daftr-text-faint); user-select: none; }
    .text { min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
    .cell[data-kind='changed'] { background: color-mix(in srgb, #d9a13b 18%, transparent); }
    .cell.left[data-kind='removed'] { background: color-mix(in srgb, var(--daftr-danger) 16%, transparent); }
    .cell.right[data-kind='added'] { background: color-mix(in srgb, var(--daftr-syntax-comment) 22%, transparent); }
  `],
})
export class CollectionCompareDialogComponent {
  protected readonly i18n = inject(I18nService);
  private readonly collections = inject(CollectionsService);

  readonly root = input.required<string>();
  readonly conflict = input.required<CollectionConflict>();
  readonly closed = output<void>();

  protected readonly loading = signal(true);
  protected readonly failed = signal(false);
  protected readonly rows = signal<DiffRow[]>([]);
  protected readonly different = computed(() => hasDifferences(this.rows()));

  /** The copy keeps one side; the original holds the other. */
  protected readonly originalLabel = computed(() => {
    const side = this.conflict().mine ? this.i18n.t('csync.compare.otherVersion') : this.i18n.t('csync.compare.mine');

    return `${this.i18n.t('csync.compare.original')} · ${side}`;
  });

  protected readonly copyLabel = computed(() => {
    const conflict = this.conflict();
    const side = conflict.mine ? this.i18n.t('csync.compare.mine') : this.i18n.t('csync.compare.theirs', { device: conflict.deviceName });

    return `${this.i18n.t('csync.compare.copy')} · ${side}`;
  });

  constructor() {
    void this.load();
  }

  private async load(): Promise<void> {
    const conflict = this.conflict();

    try {
      const [original, copy] = await Promise.all([
        this.collections.read({ root: this.root(), path: conflict.original }),
        this.collections.read({ root: this.root(), path: conflict.path }),
      ]);
      this.rows.set(diffLines(original, copy));
    } catch {
      this.failed.set(true);
    } finally {
      this.loading.set(false);
    }
  }
}
