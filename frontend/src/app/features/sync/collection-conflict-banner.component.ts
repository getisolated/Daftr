import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionFileRef } from '../../core/models/collection.model';
import { ConflictKeep } from '../../core/models/collection-sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { conflictOfFile } from '../../core/utils/collection-sync.util';
import { CollectionCompareDialogComponent } from './collection-compare-dialog.component';

/**
 * Banner at the top of the editor of a conflict copy (`name (conflit <device>).md`): "Keep my version", "Keep the
 * other" or "Compare". A choice puts the kept version in place of the original and sends the copy to the recycle bin,
 * which closes this tab. Nothing is shown for any other file.
 */
@Component({
  selector: 'app-collection-conflict-banner',
  standalone: true,
  imports: [CollectionCompareDialogComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (conflict(); as current) {
      <div class="banner" role="alert">
        <span class="text">
          <span aria-hidden="true">⚠</span>
          {{ current.conflict.mine ? i18n.t('csync.conflict.bannerMine') : i18n.t('csync.conflict.bannerOther', { device: current.conflict.deviceName }) }}
        </span>
        <button type="button" [disabled]="busy()" (click)="keep('mine')">{{ i18n.t('csync.conflict.keepMine') }}</button>
        <button type="button" [disabled]="busy()" (click)="keep('other')">{{ i18n.t('csync.conflict.keepOther') }}</button>
        <button type="button" [disabled]="busy()" (click)="comparing.set(true)">{{ i18n.t('csync.conflict.compare') }}</button>
      </div>
      @if (error(); as message) {
        <div class="banner error" role="alert"><span class="text">{{ message }}</span></div>
      }
      @if (comparing()) {
        <app-collection-compare-dialog [root]="current.root" [conflict]="current.conflict" (closed)="comparing.set(false)" />
      }
    }
  `,
  styles: [`
    :host { display: block; flex-shrink: 0; }
    :host-context(.zen) .banner:first-child { margin-top: var(--daftr-zen-titlebar-height); }
    .banner {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 6px 16px;
      font-family: 'Segoe UI', system-ui, sans-serif;
      font-size: 12.5px;
      color: var(--daftr-text);
      background: color-mix(in srgb, #d9a13b 20%, transparent);
      border-bottom: 1px solid var(--daftr-border-subtle);
    }
    .banner.error { background: color-mix(in srgb, var(--daftr-danger) 16%, transparent); }
    .text { flex: 1; min-width: 0; }
    button {
      height: 24px;
      padding: 0 10px;
      background: transparent;
      border: 1px solid var(--daftr-border);
      border-radius: 4px;
      color: var(--daftr-text);
      font: inherit;
      cursor: pointer;
    }
    button:hover:not(:disabled) { border-color: var(--daftr-accent); }
    button:disabled { opacity: 0.5; cursor: default; }
  `],
})
export class CollectionConflictBannerComponent {
  protected readonly i18n = inject(I18nService);
  private readonly collectionSync = inject(CollectionSyncService);

  /** The file open in the editor; `null` for a note. */
  readonly file = input<CollectionFileRef | null>(null);

  protected readonly comparing = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly conflict = computed(() => {
    const file = this.file();
    const conflict = file ? conflictOfFile(this.collectionSync.shares(), file.root, file.path) : null;

    return file && conflict ? { root: file.root, conflict } : null;
  });

  protected async keep(side: ConflictKeep): Promise<void> {
    const current = this.conflict();

    if (!current) {
      return;
    }

    this.busy.set(true);
    this.error.set(null);

    try {
      await this.collectionSync.resolveConflict(current.root, current.conflict.path, side);
    } catch (error) {
      this.error.set(this.i18n.t('csync.conflict.failed', { message: this.collectionSync.errorMessage(error) }));
    } finally {
      this.busy.set(false);
    }
  }
}
