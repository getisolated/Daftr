import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { SyncService } from '../../core/services/sync.service';
import { offerBadge } from '../../core/utils/sync-share.util';

/**
 * State of the sync in the sidebar footer (the app has no status bar); a click opens the sync screen. A badge counts the
 * collections received and not set up yet. Hidden while the sync is off (no linked device): the app then looks as it did
 * before the sync.
 */
@Component({
  selector: 'app-sync-indicator',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (shown(); as indicator) {
      <button type="button" class="indicator" [class]="indicator.tone" [title]="tooltip()" (click)="open()">
        <svg class="icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12a9 9 0 0 1-15.5 6.2L3 16" /><path d="M3 21v-5h5" />
          <path d="M3 12a9 9 0 0 1 15.5-6.2L21 8" /><path d="M21 3v5h-5" />
        </svg>
        <span class="label">{{ indicator.label }}</span>
        @if (badge(); as count) {
          <span class="badge" [attr.aria-label]="pendingLabel()">{{ count }}</span>
        }
      </button>
    }
  `,
  styles: [`
    :host { display: block; }
    .indicator {
      display: flex;
      align-items: center;
      gap: 8px;
      width: 100%;
      height: 24px;
      padding: 0 10px;
      border: none;
      border-radius: 4px;
      background: transparent;
      color: var(--daftr-text-dimmed);
      font-family: inherit;
      font-size: 11.5px;
      cursor: pointer;
    }
    .indicator:hover { color: var(--daftr-text-bright); background: rgba(var(--daftr-hover-rgb), 0.08); }
    .label { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
    .icon { flex: none; }
    .badge {
      flex: none;
      margin-left: auto;
      min-width: 16px;
      padding: 0 5px;
      border-radius: 8px;
      background: var(--daftr-accent-strong);
      color: #fff;
      font-size: 10.5px;
      line-height: 16px;
      text-align: center;
    }
    .ok .icon { color: var(--daftr-syntax-comment); }
    .busy .icon { color: var(--daftr-accent); animation: spin 1.2s linear infinite; }
    .error { color: var(--daftr-danger); }
    @keyframes spin { to { transform: rotate(360deg); } }
    @media (prefers-reduced-motion: reduce) { .busy .icon { animation: none; } }
  `],
})
export class SyncIndicatorComponent {
  private readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly collectionSync = inject(CollectionSyncService);

  protected readonly shown = computed(() => {
    const indicator = this.sync.indicator();
    return indicator && indicator.state !== 'disabled' ? indicator : null;
  });

  protected readonly badge = computed(() => offerBadge(this.collectionSync.pendingCount()));
  protected readonly pendingLabel = computed(() => this.i18n.plural('csync.pending', this.collectionSync.pendingCount()));

  /** The state, the cause of an error, the collections to set up, and what a click does. */
  protected readonly tooltip = computed(() => {
    const indicator = this.shown();
    const pending = this.collectionSync.pendingCount() > 0 ? this.pendingLabel() : null;
    const parts = [indicator?.label, indicator?.reason, pending, this.i18n.t('sync.status.open')];

    return parts.filter(Boolean).join('\n');
  });

  protected open(): void {
    this.sync.openScreen();
  }
}
