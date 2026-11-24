import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SyncService } from '../../core/services/sync.service';

/**
 * Problem of the sync in the sidebar footer (the app has no status bar); a click opens the sync screen. Shown only for an
 * error: nothing is shown while the sync is fine, off, syncing or paused, so a problem cannot go unseen
 * without the footer being busier the rest of the time.
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
    .error { color: var(--daftr-danger); }
  `],
})
export class SyncIndicatorComponent {
  private readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);

  protected readonly shown = computed(() => {
    const indicator = this.sync.indicator();
    return indicator && indicator.tone === 'error' ? indicator : null;
  });

  /** The state, the cause of an error, and what a click does. */
  protected readonly tooltip = computed(() => {
    const indicator = this.shown();
    const parts = [indicator?.label, indicator?.reason, this.i18n.t('sync.status.open')];

    return parts.filter(Boolean).join('\n');
  });

  protected open(): void {
    this.sync.openScreen();
  }
}
