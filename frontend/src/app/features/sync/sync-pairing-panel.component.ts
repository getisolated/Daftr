import { ChangeDetectionStrategy, Component, OnDestroy, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SyncService } from '../../core/services/sync.service';
import { formatPairingCode, needsNewPairingCode, pairingAddresses, svgDataUrl } from '../../core/utils/sync.util';
import { pairingCountdown } from './pairing-countdown';
import { SyncJoinFormComponent } from './sync-join-form.component';

/**
 * "Link a device", always on screen: the QR code and the 6 digits, open from the start and renewed by themselves, with
 * the time left. "Enter a code" replaces them with the entry of the code of another device. The pairing mode opens with
 * this block and closes with it.
 */
@Component({
  selector: 'app-sync-pairing-panel',
  standalone: true,
  imports: [SyncJoinFormComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (joining()) {
      <app-sync-join-form (joined)="joining.set(false)" (cancelled)="joining.set(false)" />
    } @else {
      <h3 class="section">{{ i18n.t('sync.screen.pair') }}</h3>
      <div class="pairing">
        @if (liveTicket(); as current) {
          <img class="qr" [src]="qrUrl()" [alt]="i18n.t('sync.pairing.qrAlt')" width="176" height="176" />
          <div class="code" aria-live="polite">{{ formattedCode(current.code) }}</div>
          <div class="countdown">{{ i18n.t('sync.pairing.expiresIn', { time: countdown() }) }}</div>
          @if (renewed()) {
            <p class="hint">{{ i18n.t('sync.pairing.renewed') }}</p>
          }
          <p class="hint">{{ i18n.t('sync.pairing.instructions') }}</p>
          @if (addresses(); as shown) {
            <p class="hint addresses">{{ shown }}</p>
          }
        } @else if (pairingError(); as message) {
          <div class="placeholder">
            <span class="error" role="alert">{{ message }}</span>
            <button class="btn secondary" type="button" (click)="pairingError.set(null)">{{ i18n.t('sync.pairing.retry') }}</button>
          </div>
        } @else {
          <div class="placeholder">
            <span class="hint">{{ i18n.t('sync.pairing.preparing') }}</span>
          </div>
        }
        <button class="link" type="button" (click)="joining.set(true)">{{ i18n.t('sync.pairing.enterCode') }}</button>
      </div>
    }
  `,
  styleUrls: ['./sync-screen-ui.scss'],
  styles: [`
    :host { display: flex; flex-direction: column; gap: 12px; }
    .pairing { display: flex; flex-direction: column; align-items: center; gap: 4px; text-align: center; }
    /* White background: the QR code stays readable in the dark themes */
    .qr { padding: 10px; background: #fff; border-radius: 6px; }
    .code {
      margin-top: 8px;
      font-family: var(--daftr-font-mono);
      font-size: 30px;
      letter-spacing: 0.12em;
      color: var(--daftr-text-strong);
      user-select: text;
    }
    .countdown { font-size: 11px; color: var(--daftr-text-dimmed); }
    .hint { margin-top: 4px; }
    .addresses { font-family: var(--daftr-font-mono); font-size: 11px; user-select: text; }
    /* Same height as the QR code: the block does not jump when the code arrives */
    .placeholder { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; min-height: 220px; }
    .link { margin-top: 8px; }
  `],
})
export class SyncPairingPanelComponent implements OnDestroy {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);

  /** "Enter a code": the form replaces the code, whose pairing mode is closed meanwhile. */
  protected readonly joining = signal(false);
  private readonly starting = signal(false);
  protected readonly pairingError = signal<string | null>(null);

  private readonly ticket = this.sync.pairing;
  private readonly timer = pairingCountdown(this.ticket, () => !this.joining());
  protected readonly liveTicket = computed(() => (this.timer.remaining() > 0 ? this.ticket() : null));
  protected readonly countdown = this.timer.label;
  protected readonly renewed = this.sync.pairingRenewed;
  protected readonly qrUrl = computed(() => {
    const ticket = this.liveTicket();
    return ticket ? svgDataUrl(ticket.qrSvg) : '';
  });
  /** Addresses of this device, to type on the other one when it does not find this one. */
  protected readonly addresses = computed(() => pairingAddresses(this.liveTicket()?.uri ?? ''));

  constructor() {
    // Always a valid code on screen: a new one at opening, after a linking, and once the previous one expired
    effect(() => {
      const waiting = this.joining() || this.starting() || this.pairingError() !== null;

      if (needsNewPairingCode(this.ticket(), this.timer.remaining(), waiting)) {
        untracked(() => { void this.startPairing(); });
      }
    });

    // The code of this device is not shown while the code of another one is typed
    effect(() => {
      if (this.joining()) {
        untracked(() => { void this.stopPairing(); });
      }
    });
  }

  ngOnDestroy(): void {
    void this.stopPairing();
  }

  protected formattedCode(code: string): string {
    return formatPairingCode(code);
  }

  private async startPairing(): Promise<void> {
    this.starting.set(true);
    this.pairingError.set(null);

    try {
      await this.sync.startPairing();
    } catch (error) {
      this.pairingError.set(this.sync.errorMessage(error));
    } finally {
      this.starting.set(false);
    }
  }

  private async stopPairing(): Promise<void> {
    try {
      await this.sync.stopPairing();
    } catch (error) {
      // The mode expires by itself after a few minutes: nothing to show for a screen being closed
      console.error('[Sync] Pairing not stopped:', error);
    }
  }
}
